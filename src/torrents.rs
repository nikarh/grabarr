//! Watches the torrents of the default category and fans the changes out to subscribers.
//!
//! One poller of qBittorrent's `sync/maindata` runs while anyone is subscribed, however many subscribers there are.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, Weak};

use serde_json::{Map, Value};
use tokio::sync::{Notify, broadcast};

use crate::api::{ErrorCode, Torrent, TorrentsEvent};
use crate::app::App;
use crate::qbittorrent;

pub struct Watcher {
    inner: Mutex<Inner>,
    events: broadcast::Sender<Arc<TorrentsEvent>>,
    /// Wakes the poller early: on a settings change or a removal.
    wake: Notify,
}

#[derive(Default)]
struct Inner {
    subscribers: usize,
    running: bool,
    /// Bumped on a settings change, so the poller starts over from a full update.
    generation: u64,
    /// The torrents of the category, once known.
    torrents: Option<BTreeMap<String, Torrent>>,
}

/// A subscription; unsubscribes when dropped.
pub struct Subscription {
    watcher: Arc<Watcher>,
    pub initial: Option<TorrentsEvent>,
    pub events: broadcast::Receiver<Arc<TorrentsEvent>>,
}

impl Subscription {
    /// The current state, after the receiver fell behind.
    pub fn snapshot(&self) -> Option<TorrentsEvent> {
        let inner = self.watcher.inner.lock().ok()?;
        inner.torrents.as_ref().map(snapshot)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.watcher.inner.lock() {
            inner.subscribers = inner.subscribers.saturating_sub(1);
        }
        self.watcher.wake.notify_one();
    }
}

fn snapshot(torrents: &BTreeMap<String, Torrent>) -> TorrentsEvent {
    let mut list: Vec<_> = torrents.values().cloned().collect();
    list.sort_by(|a, b| {
        b.added_on
            .cmp(&a.added_on)
            .then_with(|| a.name.cmp(&b.name))
    });
    TorrentsEvent::Snapshot { torrents: list }
}

impl Watcher {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner::default()),
            events: broadcast::channel(64).0,
            wake: Notify::new(),
        })
    }

    pub fn subscribe(self: &Arc<Self>, app: &Arc<App>) -> Subscription {
        let mut start = false;
        let (initial, events) = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inner.subscribers += 1;
            if !inner.running {
                inner.running = true;
                start = true;
            }
            (
                inner.torrents.as_ref().map(snapshot),
                self.events.subscribe(),
            )
        };
        if start {
            tokio::spawn(poll(Arc::downgrade(app), Arc::clone(self)));
        }
        Subscription {
            watcher: Arc::clone(self),
            initial,
            events,
        }
    }

    /// Starts over with a full update, e.g. after the category changed.
    pub fn reset(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.generation += 1;
        }
        self.wake.notify_one();
    }

    /// Polls sooner than the interval.
    pub fn refresh(&self) {
        self.wake.notify_one();
    }

    fn publish(&self, event: TorrentsEvent) {
        // No receivers is fine.
        let _ = self.events.send(Arc::new(event));
    }
}

/// The poller's view of qBittorrent: all torrents' raw fields, merged from partial updates.
#[derive(Default)]
struct Sync {
    rid: u64,
    raw: HashMap<String, Map<String, Value>>,
}

impl Sync {
    /// Applies an update and returns the torrents of `category`.
    fn apply(&mut self, data: qbittorrent::MainData, category: &str) -> BTreeMap<String, Torrent> {
        if data.full_update {
            self.raw.clear();
        }
        self.rid = data.rid;
        for hash in data.torrents_removed {
            self.raw.remove(&hash);
        }
        for (hash, fields) in data.torrents {
            self.raw.entry(hash).or_default().extend(fields);
        }
        self.raw
            .iter()
            .filter(|(_, t)| t.get("category").and_then(Value::as_str) == Some(category))
            .map(|(hash, t)| (hash.clone(), torrent(hash, t)))
            .collect()
    }
}

/// qBittorrent's "infinity".
const ETA_UNKNOWN: u64 = 8_640_000;

fn torrent(hash: &str, t: &Map<String, Value>) -> Torrent {
    let num = |name: &str| t.get(name).and_then(Value::as_u64).unwrap_or(0);
    let eta = num("eta");
    Torrent {
        hash: hash.to_owned(),
        name: t
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        status: qbittorrent::status(t.get("state").and_then(Value::as_str).unwrap_or_default()),
        progress: t
            .get("progress")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0),
        size: num("size"),
        download_speed: num("dlspeed"),
        upload_speed: num("upspeed"),
        eta: (eta > 0 && eta < ETA_UNKNOWN).then_some(eta),
        added_on: t.get("added_on").and_then(Value::as_i64).unwrap_or(0),
    }
}

fn diff(old: &BTreeMap<String, Torrent>, new: &BTreeMap<String, Torrent>) -> Option<TorrentsEvent> {
    let upserted: Vec<_> = new
        .iter()
        .filter(|(hash, t)| old.get(*hash) != Some(t))
        .map(|(_, t)| t.clone())
        .collect();
    let removed: Vec<_> = old
        .keys()
        .filter(|hash| !new.contains_key(*hash))
        .cloned()
        .collect();
    (!upserted.is_empty() || !removed.is_empty())
        .then_some(TorrentsEvent::Update { upserted, removed })
}

async fn poll(app: Weak<App>, watcher: Arc<Watcher>) {
    let mut sync = Sync::default();
    let mut generation = None;
    let mut failing = false;

    loop {
        let Some(app) = app.upgrade() else { return };
        {
            let mut inner = watcher
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if inner.subscribers == 0 {
                inner.running = false;
                inner.torrents = None;
                return;
            }
            if generation != Some(inner.generation) {
                generation = Some(inner.generation);
                sync = Sync::default();
                inner.torrents = None;
            }
        }

        match fetch(&app, &mut sync).await {
            Ok(torrents) => {
                failing = false;
                let mut inner = watcher
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                // The settings changed while fetching: this result is for the old category.
                if generation == Some(inner.generation) {
                    let event = match &inner.torrents {
                        None => Some(snapshot(&torrents)),
                        Some(old) => diff(old, &torrents),
                    };
                    inner.torrents = Some(torrents);
                    if let Some(event) = event {
                        watcher.publish(event);
                    }
                }
            }
            Err((code, message)) => {
                if !failing {
                    tracing::warn!(%message, "Fetching torrents failed");
                }
                failing = true;
                sync = Sync::default();
                if let Ok(mut inner) = watcher.inner.lock() {
                    inner.torrents = None;
                }
                watcher.publish(TorrentsEvent::Error { code, message });
            }
        }

        let interval = app.poll_interval();
        drop(app);
        let _ = tokio::time::timeout(interval, watcher.wake.notified()).await;
    }
}

async fn fetch(
    app: &App,
    sync: &mut Sync,
) -> Result<BTreeMap<String, Torrent>, (ErrorCode, String)> {
    let (qbt, category) = app.target().await.map_err(|e| (e.code, e.message))?;
    let data = qbt
        .main_data(sync.rid)
        .await
        .map_err(|e| (ErrorCode::Upstream, e.to_string()))?;
    Ok(sync.apply(data, &category))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::TorrentStatus;

    fn data(json: &str) -> qbittorrent::MainData {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn merges_partial_updates() {
        let mut sync = Sync::default();
        let full = sync.apply(
            data(
                r#"{"rid":1,"full_update":true,"torrents":{
                    "a":{"name":"A","category":"c","state":"downloading","progress":0.1,"size":100,"eta":60,"added_on":2},
                    "b":{"name":"B","category":"other","state":"downloading"}}}"#,
            ),
            "c",
        );
        assert_eq!(full.len(), 1);
        assert_eq!(full["a"].status, TorrentStatus::Downloading);
        assert_eq!(full["a"].eta, Some(60));

        let next = sync.apply(
            data(
                r#"{"rid":2,"torrents":{"a":{"progress":1.0,"state":"stalledUP","eta":8640000}}}"#,
            ),
            "c",
        );
        assert_eq!(sync.rid, 2);
        assert_eq!(next["a"].name, "A");
        assert_eq!(next["a"].status, TorrentStatus::Seeding);
        assert_eq!(next["a"].eta, None);

        let Some(TorrentsEvent::Update { upserted, removed }) = diff(&full, &next) else {
            panic!("expected an update");
        };
        assert_eq!(upserted.len(), 1);
        assert_eq!(removed, Vec::<String>::new());
        assert!(diff(&next, &next).is_none());

        // Moving a torrent to another category removes it.
        let moved = sync.apply(data(r#"{"rid":3,"torrents":{"a":{"category":"x"}}}"#), "c");
        let Some(TorrentsEvent::Update { removed, .. }) = diff(&next, &moved) else {
            panic!("expected an update");
        };
        assert_eq!(removed, vec!["a".to_owned()]);

        let gone = sync.apply(data(r#"{"rid":4,"torrents_removed":["a","b"]}"#), "x");
        assert!(gone.is_empty());
        assert!(sync.raw.is_empty());
    }
}
