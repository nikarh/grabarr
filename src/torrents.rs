//! Watches the torrents of the default category and fans the changes out to subscribers.
//!
//! One poller runs while anyone is subscribed, however many subscribers there are. It asks qBittorrent for the
//! category's torrents (`torrents/info`) rather than using the incremental `sync/maindata`: that is filtered by
//! qBittorrent, stateless, and avoids crashes some qBittorrent versions have in generating incremental updates.

use std::collections::BTreeMap;
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

fn collect(list: &[Map<String, Value>]) -> BTreeMap<String, Torrent> {
    list.iter()
        .filter_map(|t| {
            let hash = t.get("hash").and_then(Value::as_str)?;
            Some((hash.to_owned(), torrent(hash, t)))
        })
        .collect()
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
                inner.torrents = None;
            }
        }

        match fetch(&app).await {
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

async fn fetch(app: &App) -> Result<BTreeMap<String, Torrent>, (ErrorCode, String)> {
    let (qbt, category) = app.target().await.map_err(|e| (e.code, e.message))?;
    let list = qbt
        .torrents(&category, None)
        .await
        .map_err(|e| (ErrorCode::Upstream, e.to_string()))?;
    Ok(collect(&list))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::TorrentStatus;

    fn list(json: &str) -> BTreeMap<String, Torrent> {
        collect(&serde_json::from_str::<Vec<Map<String, Value>>>(json).unwrap())
    }

    #[test]
    fn converts_and_diffs() {
        let first = list(
            r#"[{"hash":"a","name":"A","state":"downloading","progress":0.1,"size":100,"eta":60,"added_on":2},
                {"hash":"b","name":"B","state":"stoppedDL","eta":8640000},
                {"name":"no hash"}]"#,
        );
        assert_eq!(first.len(), 2);
        assert_eq!(first["a"].status, TorrentStatus::Downloading);
        assert_eq!(first["a"].eta, Some(60));
        assert_eq!(first["b"].status, TorrentStatus::Paused);
        assert_eq!(first["b"].eta, None);

        let next = list(
            r#"[{"hash":"a","name":"A","state":"stalledUP","progress":1.0,"size":100,"added_on":2}]"#,
        );
        let Some(TorrentsEvent::Update { upserted, removed }) = diff(&first, &next) else {
            panic!("expected an update");
        };
        assert_eq!(upserted.len(), 1);
        assert_eq!(upserted[0].status, TorrentStatus::Seeding);
        assert_eq!(removed, vec!["b".to_owned()]);
        assert!(diff(&next, &next).is_none());
    }

    #[test]
    fn snapshot_is_newest_first() {
        let TorrentsEvent::Snapshot { torrents } = snapshot(&list(
            r#"[{"hash":"a","added_on":1},{"hash":"b","added_on":5}]"#,
        )) else {
            panic!("expected a snapshot");
        };
        assert_eq!(torrents[0].hash, "b");
    }
}
