//! Types of the WebSocket RPC protocol, shared with the frontend.
//!
//! `cargo test` exports them as TypeScript into `web/src/api/generated/` (see `mise run bindings`).
//!
//! # Protocol
//!
//! The client sends text frames that are either a single [`Request`] or a JSON array of them. Every request carries
//! a client-chosen `id`, and the requests of one frame run concurrently: each gets its own response, a
//! [`ServerMessage::Result`] or [`ServerMessage::Error`] with the same `id`, as soon as it completes.
//!
//! A `*.subscribe` call answers with a `null` result and then pushes [`ServerMessage::Event`]s tagged with the
//! subscribing request's `id` until the client sends `unsubscribe` with that id, or disconnects.

use serde::{Deserialize, Serialize};

#[cfg(test)]
use ts_rs::TS;

/// One call from the client: a method and its parameters.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
pub struct Request {
    pub id: String,
    #[serde(flatten)]
    pub call: Call,
}

/// Every method the server implements, with its parameters.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(tag = "method", content = "params")]
pub enum Call {
    /// Result: [`Settings`].
    #[serde(rename = "settings.get")]
    SettingsGet,
    /// Result: [`Settings`], as saved.
    #[serde(rename = "settings.update")]
    SettingsUpdate(Settings),
    /// Result: `Indexer[]`, the torrent indexers configured in Prowlarr.
    #[serde(rename = "indexers.list")]
    IndexersList,
    /// Result: `DownloadClient[]`, the qBittorrent download clients configured in Prowlarr.
    #[serde(rename = "downloadClients.list")]
    DownloadClientsList,
    /// Result: `string[]`, the categories of a download client.
    #[serde(rename = "downloadCategories.list")]
    DownloadCategoriesList(CategoriesParams),
    /// Result: `SearchCategory[]`, Prowlarr's search categories.
    #[serde(rename = "searchCategories.list")]
    SearchCategoriesList,
    /// Result: [`SearchPage`].
    #[serde(rename = "search")]
    Search(SearchParams),
    /// Result: `null`, once the release was added to the default download client.
    #[serde(rename = "grab")]
    Grab(GrabParams),
    /// Result: `null`, then [`TorrentsEvent`]s.
    #[serde(rename = "torrents.subscribe")]
    TorrentsSubscribe,
    /// Result: `null`. Removes torrents of the default category together with their files.
    #[serde(rename = "torrents.remove")]
    TorrentsRemove(RemoveParams),
    /// Result: `null`. Ends the subscription started by the request with this id.
    #[serde(rename = "unsubscribe")]
    Unsubscribe(UnsubscribeParams),
}

/// A message from the server.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Result {
        id: String,
        #[cfg_attr(test, ts(type = "unknown"))]
        result: serde_json::Value,
    },
    Error {
        /// `null` when the frame couldn't be parsed far enough to know the request's id.
        id: Option<String>,
        error: RpcError,
    },
    Event {
        id: String,
        #[cfg_attr(test, ts(type = "unknown"))]
        event: serde_json::Value,
    },
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
pub struct RpcError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The frame or the parameters are malformed.
    BadRequest,
    /// Something has to be chosen in the settings first.
    NotConfigured,
    /// The release is no longer known; search again.
    NotFound,
    /// Prowlarr or qBittorrent failed or couldn't be reached.
    Upstream,
    Internal,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Prowlarr's id of the qBittorrent download client grabs go to.
    #[cfg_attr(test, ts(type = "number | null"))]
    pub download_client_id: Option<i64>,
    /// The category grabs get, and the downloads screen shows.
    pub category: Option<String>,
    /// Indexers searched when a search names none; empty means all of them.
    #[cfg_attr(test, ts(type = "Array<number>"))]
    pub indexer_ids: Vec<i64>,
    /// Prowlarr categories searched unless a search names others; empty means all of them.
    #[serde(default)]
    #[cfg_attr(test, ts(type = "Array<number>"))]
    pub search_categories: Vec<i64>,
}

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Indexer {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: i64,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct DownloadClient {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: i64,
    pub name: String,
    pub enabled: bool,
}

/// A Prowlarr (Newznab) search category, e.g. 2000 Movies, with its subcategories (2040 Movies/HD, ...).
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SearchCategory {
    #[cfg_attr(test, ts(type = "number"))]
    pub id: i64,
    pub name: String,
    pub subcategories: Vec<SearchCategory>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(rename_all = "camelCase")]
pub struct CategoriesParams {
    #[cfg_attr(test, ts(type = "number"))]
    pub download_client_id: i64,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    pub query: String,
    /// Indexers to search instead of the default ones.
    #[serde(default)]
    #[cfg_attr(test, ts(optional, type = "Array<number>"))]
    pub indexer_ids: Option<Vec<i64>>,
    /// Prowlarr categories to search instead of the default ones; a parent category includes its subcategories.
    #[serde(default)]
    #[cfg_attr(test, ts(optional, type = "Array<number>"))]
    pub categories: Option<Vec<i64>>,
    #[serde(default)]
    #[cfg_attr(test, ts(optional))]
    pub offset: Option<u32>,
    #[serde(default)]
    #[cfg_attr(test, ts(optional))]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub releases: Vec<Release>,
    /// The offset of the next page, or `null` when this was the last one.
    pub next_offset: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub guid: String,
    pub title: String,
    #[cfg_attr(test, ts(type = "number"))]
    pub size: u64,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    pub indexer: String,
    #[cfg_attr(test, ts(type = "number"))]
    pub indexer_id: i64,
    /// RFC 3339.
    pub publish_date: Option<String>,
    /// The release's page on the indexer's site.
    pub info_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(rename_all = "camelCase")]
pub struct GrabParams {
    /// A release's `guid` from a recent search.
    pub guid: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(rename_all = "camelCase")]
pub struct RemoveParams {
    pub hashes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "rpc.ts"))]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeParams {
    pub id: String,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub enum TorrentStatus {
    Downloading,
    /// Downloading, but no peers to download from.
    Stalled,
    /// Fetching metadata of a magnet link.
    Metadata,
    Queued,
    Checking,
    /// Complete and seeding.
    Seeding,
    /// Complete, not seeding.
    Completed,
    /// Stopped before completion.
    Paused,
    Moving,
    Error,
    Unknown,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(rename_all = "camelCase")]
pub struct Torrent {
    pub hash: String,
    pub name: String,
    pub status: TorrentStatus,
    /// 0 to 1.
    pub progress: f64,
    #[cfg_attr(test, ts(type = "number"))]
    pub size: u64,
    /// Bytes per second.
    #[cfg_attr(test, ts(type = "number"))]
    pub download_speed: u64,
    /// Bytes per second.
    #[cfg_attr(test, ts(type = "number"))]
    pub upload_speed: u64,
    /// Seconds until complete, when known.
    #[cfg_attr(test, ts(type = "number | null"))]
    pub eta: Option<u64>,
    /// Unix seconds.
    #[cfg_attr(test, ts(type = "number"))]
    pub added_on: i64,
}

/// Pushed to `torrents.subscribe` subscribers.
#[derive(Debug, Serialize, Clone)]
#[cfg_attr(test, derive(TS), ts(export, export_to = "model.ts"))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TorrentsEvent {
    /// All torrents of the category; replaces whatever the client had.
    Snapshot { torrents: Vec<Torrent> },
    /// Changes since the previous event.
    Update {
        upserted: Vec<Torrent>,
        removed: Vec<String>,
    },
    /// The torrents can't be fetched right now; retried until it works, then a snapshot follows.
    Error { code: ErrorCode, message: String },
}
