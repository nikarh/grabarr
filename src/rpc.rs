//! The WebSocket endpoint: parses request frames, runs the calls concurrently and streams the responses back.
//! See [`crate::api`] for the protocol.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc};
use tokio::task::{AbortHandle, JoinSet};

use crate::api::{Call, ErrorCode, Request, RpcError, ServerMessage, TorrentsEvent};
use crate::app::{App, Error};

/// Requests in one frame, at most.
const MAX_BATCH: usize = 200;
const PING_INTERVAL: Duration = Duration::from_secs(30);

type Subscriptions = Arc<Mutex<HashMap<String, AbortHandle>>>;

pub async fn serve(socket: WebSocket, app: Arc<App>) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(256);

    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(PING_INTERVAL);
        ping.reset();
        loop {
            let message = tokio::select! {
                message = rx.recv() => match message {
                    Some(message) => match serde_json::to_string(&message) {
                        Ok(text) => Message::Text(Utf8Bytes::from(text)),
                        Err(e) => {
                            tracing::error!(error = %e, "Serializing a response failed");
                            continue;
                        }
                    },
                    None => break,
                },
                _ = ping.tick() => Message::Ping(axum::body::Bytes::new()),
            };
            if sink.send(message).await.is_err() {
                break;
            }
        }
    });

    let subscriptions: Subscriptions = Arc::default();
    let mut calls = JoinSet::new();

    while let Some(Ok(message)) = stream.next().await {
        let text = match message {
            Message::Text(text) => text,
            Message::Close(_) => break,
            _ => continue,
        };
        for (id, call) in parse(&text) {
            let call = match call {
                Ok(call) => call,
                Err(error) => {
                    if tx.send(ServerMessage::Error { id, error }).await.is_err() {
                        break;
                    }
                    continue;
                }
            };
            let Some(id) = id else { continue };
            let app = Arc::clone(&app);
            let tx = tx.clone();
            let subscriptions = Arc::clone(&subscriptions);
            calls.spawn(async move {
                let message = match run(&app, &id, call, &tx, &subscriptions).await {
                    // Already answered.
                    Ok(None) => return,
                    Ok(Some(result)) => ServerMessage::Result { id, result },
                    Err(e) => ServerMessage::Error {
                        id: Some(id),
                        error: RpcError {
                            code: e.code,
                            message: e.message,
                        },
                    },
                };
                let _ = tx.send(message).await;
            });
        }
        // Reap finished calls so the set doesn't grow over a long connection.
        while calls.try_join_next().is_some() {}
    }

    calls.abort_all();
    if let Ok(subscriptions) = subscriptions.lock() {
        for handle in subscriptions.values() {
            handle.abort();
        }
    }
    drop(tx);
    let _ = writer.await;
}

/// Splits a frame into its calls. An unparsable call yields an error, with the id when there is one.
fn parse(text: &str) -> Vec<(Option<String>, Result<Call, RpcError>)> {
    let bad = |message: String| RpcError {
        code: ErrorCode::BadRequest,
        message,
    };
    let values = match serde_json::from_str::<Value>(text) {
        Ok(Value::Array(values)) => values,
        Ok(value @ Value::Object(_)) => vec![value],
        Ok(_) => {
            return vec![(
                None,
                Err(bad("expected a request or an array of them".into())),
            )];
        }
        Err(e) => return vec![(None, Err(bad(format!("invalid JSON: {e}"))))],
    };
    if values.len() > MAX_BATCH {
        return vec![(
            None,
            Err(bad(format!("at most {MAX_BATCH} requests per frame"))),
        )];
    }
    values
        .into_iter()
        .map(|value| {
            let id = value.get("id").and_then(Value::as_str).map(str::to_owned);
            match (&id, serde_json::from_value::<Request>(value)) {
                (Some(_), Ok(request)) => (Some(request.id), Ok(request.call)),
                (None, _) => (None, Err(bad("a request needs a string id".into()))),
                (Some(_), Err(e)) => (id, Err(bad(e.to_string()))),
            }
        })
        .collect()
}

fn json(value: impl Serialize) -> Result<Option<Value>, Error> {
    serde_json::to_value(value)
        .map(Some)
        .map_err(|e| Error::new(ErrorCode::Internal, e.to_string()))
}

async fn run(
    app: &Arc<App>,
    id: &str,
    call: Call,
    tx: &mpsc::Sender<ServerMessage>,
    subscriptions: &Subscriptions,
) -> Result<Option<Value>, Error> {
    match call {
        Call::SettingsGet => json(app.settings().await?),
        Call::SettingsUpdate(settings) => json(app.update_settings(settings).await?),
        Call::IndexersList => json(app.indexers().await?),
        Call::DownloadClientsList => json(app.download_clients().await?),
        Call::DownloadCategoriesList(p) => json(app.categories(p.download_client_id).await?),
        Call::SearchCategoriesList => json(app.search_categories().await?),
        Call::Search(p) => json(app.search(p).await?),
        Call::Grab(p) => json(app.grab(&p.guid).await?),
        Call::TorrentsRemove(p) => json(app.remove(p.hashes).await?),
        Call::TorrentsSubscribe => {
            subscribe_torrents(app, id, tx, subscriptions).await?;
            Ok(None)
        }
        Call::Unsubscribe(p) => {
            let handle = subscriptions.lock().ok().and_then(|mut s| s.remove(&p.id));
            if let Some(handle) = handle {
                handle.abort();
            }
            Ok(Some(Value::Null))
        }
    }
}

/// Answers the call, then starts pushing events, so the client sees the result first.
async fn subscribe_torrents(
    app: &Arc<App>,
    id: &str,
    tx: &mpsc::Sender<ServerMessage>,
    subscriptions: &Subscriptions,
) -> Result<(), Error> {
    if subscriptions.lock().is_ok_and(|s| s.contains_key(id)) {
        return Err(Error::new(
            ErrorCode::BadRequest,
            "a subscription with this id exists",
        ));
    }
    let _ = tx
        .send(ServerMessage::Result {
            id: id.to_owned(),
            result: Value::Null,
        })
        .await;
    let mut subscription = app.torrents.subscribe(app);
    let tx = tx.clone();
    let id = id.to_owned();
    let task = tokio::spawn({
        let id = id.clone();
        async move {
            let send = |event: &TorrentsEvent| {
                let message = serde_json::to_value(event).map(|event| ServerMessage::Event {
                    id: id.clone(),
                    event,
                });
                let tx = tx.clone();
                async move {
                    match message {
                        Ok(message) => tx.send(message).await.is_ok(),
                        Err(_) => true,
                    }
                }
            };
            if let Some(initial) = subscription.initial.take()
                && !send(&initial).await
            {
                return;
            }
            loop {
                let event = match subscription.events.recv().await {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => match subscription.snapshot() {
                        Some(snapshot) => Arc::new(snapshot),
                        None => continue,
                    },
                    Err(broadcast::error::RecvError::Closed) => return,
                };
                if !send(&event).await {
                    return;
                }
            }
        }
    });
    if let Ok(mut subscriptions) = subscriptions.lock() {
        subscriptions.insert(id, task.abort_handle());
    } else {
        task.abort();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_batches() {
        let parsed = parse(
            r#"[{"id":"1","method":"settings.get"},
                {"id":"2","method":"search","params":{"query":"debian","offset":100}},
                {"id":"3","method":"nope"},
                {"method":"settings.get"}]"#,
        );
        assert_eq!(parsed.len(), 4);
        assert!(matches!(parsed[0], (Some(_), Ok(Call::SettingsGet))));
        let (_, Ok(Call::Search(p))) = &parsed[1] else {
            panic!("expected a search");
        };
        assert_eq!(p.query, "debian");
        assert_eq!(p.offset, Some(100));
        assert_eq!(p.indexer_ids, None);
        assert!(matches!(&parsed[2], (Some(id), Err(_)) if id == "3"));
        assert!(matches!(parsed[3], (None, Err(_))));
    }

    #[test]
    fn parses_single_request() {
        let parsed = parse(r#"{"id":"x","method":"unsubscribe","params":{"id":"s"}}"#);
        assert!(matches!(&parsed[..], [(Some(_), Ok(Call::Unsubscribe(p)))] if p.id == "s"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(&parse("42")[..], [(None, Err(_))]));
        assert!(matches!(&parse("{")[..], [(None, Err(_))]));
    }
}
