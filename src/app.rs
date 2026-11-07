//! What the RPC methods do.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::{StatusCode, header};

use crate::api::{self, ErrorCode, SearchPage, SearchParams, Settings};
use crate::config::Config;
use crate::db::Db;
use crate::prowlarr::{self, Prowlarr};
use crate::qbittorrent::{self, Qbittorrent, Source};
use crate::torrents::Watcher;

/// A failed call, as reported to the client.
#[derive(Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<prowlarr::Error> for Error {
    fn from(e: prowlarr::Error) -> Self {
        Self::new(ErrorCode::Upstream, e.to_string())
    }
}

impl From<qbittorrent::Error> for Error {
    fn from(e: qbittorrent::Error) -> Self {
        Self::new(ErrorCode::Upstream, e.to_string())
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "Database error");
        Self::new(ErrorCode::Internal, "database error")
    }
}

const DEFAULT_PAGE: u32 = 100;
const MAX_PAGE: u32 = 500;
/// Releases are grabbed by guid from what recent searches returned, rather than from URLs the client sends.
const RELEASE_TTL: Duration = Duration::from_hours(6);
const MAX_RELEASES: usize = 50_000;
const MAX_TORRENT_FILE: usize = 10 * 1024 * 1024;

pub struct App {
    config: Config,
    db: Db,
    prowlarr: Prowlarr,
    /// For fetching .torrent files from Prowlarr: doesn't follow redirects, as they can lead to magnet links.
    downloads: reqwest::Client,
    http: reqwest::Client,
    qbittorrent: tokio::sync::Mutex<Option<(i64, Arc<Qbittorrent>)>>,
    releases: Mutex<HashMap<String, (Instant, prowlarr::Release)>>,
    pub torrents: Arc<Watcher>,
}

impl App {
    pub fn new(config: Config, db: Db) -> Result<Arc<Self>, reqwest::Error> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("grabarr/", env!("GRABARR_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(config.prowlarr.timeout)
            .build()?;
        let downloads = reqwest::Client::builder()
            .user_agent(concat!("grabarr/", env!("GRABARR_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(config.prowlarr.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Arc::new(Self {
            prowlarr: Prowlarr::new(http.clone(), &config.prowlarr),
            config,
            db,
            downloads,
            http,
            qbittorrent: tokio::sync::Mutex::new(None),
            releases: Mutex::new(HashMap::new()),
            torrents: Watcher::new(),
        }))
    }

    pub fn poll_interval(&self) -> Duration {
        self.config.qbittorrent.poll_interval
    }

    pub async fn settings(&self) -> Result<Settings, Error> {
        Ok(self.db.settings().await?)
    }

    pub async fn update_settings(&self, mut settings: Settings) -> Result<Settings, Error> {
        settings.category = settings
            .category
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty());
        settings.indexer_ids.sort_unstable();
        settings.indexer_ids.dedup();
        let old = self.db.settings().await?;
        self.db.save_settings(&settings).await?;
        if old.download_client_id != settings.download_client_id {
            *self.qbittorrent.lock().await = None;
        }
        if old.download_client_id != settings.download_client_id
            || old.category != settings.category
        {
            self.torrents.reset();
        }
        Ok(settings)
    }

    pub async fn indexers(&self) -> Result<Vec<api::Indexer>, Error> {
        Ok(self.prowlarr.indexers().await?)
    }

    pub async fn download_clients(&self) -> Result<Vec<api::DownloadClient>, Error> {
        Ok(self.prowlarr.download_clients().await?)
    }

    pub async fn categories(&self, download_client_id: i64) -> Result<Vec<String>, Error> {
        Ok(self
            .qbittorrent(download_client_id)
            .await?
            .categories()
            .await?)
    }

    /// A client for a qBittorrent download client in Prowlarr, with the overrides from the config applied.
    async fn qbittorrent(&self, id: i64) -> Result<Arc<Qbittorrent>, Error> {
        let mut cached = self.qbittorrent.lock().await;
        if let Some((cached_id, client)) = cached.as_ref()
            && *cached_id == id
        {
            return Ok(Arc::clone(client));
        }
        let settings = self.prowlarr.qbittorrent(id).await?.ok_or_else(|| {
            Error::new(
                ErrorCode::NotConfigured,
                format!("Prowlarr has no qBittorrent download client with id {id}"),
            )
        })?;
        let o = &self.config.qbittorrent;
        let client = Arc::new(Qbittorrent::new(
            self.http.clone(),
            o.url.clone().unwrap_or(settings.url),
            o.username.clone().or(settings.username),
            o.password.clone().or(settings.password),
        ));
        tracing::info!(id, url = client.url(), "Using qBittorrent");
        if id_matches_settings(&self.db, id).await {
            *cached = Some((id, Arc::clone(&client)));
        }
        Ok(client)
    }

    /// The default download client and category.
    pub async fn target(&self) -> Result<(Arc<Qbittorrent>, String), Error> {
        let settings = self.db.settings().await?;
        let (Some(id), Some(category)) = (settings.download_client_id, settings.category) else {
            return Err(Error::new(
                ErrorCode::NotConfigured,
                "choose a download client and a category in the settings first",
            ));
        };
        Ok((self.qbittorrent(id).await?, category))
    }

    pub async fn search(&self, params: SearchParams) -> Result<SearchPage, Error> {
        let query = params.query.trim();
        if query.is_empty() {
            return Err(Error::new(ErrorCode::BadRequest, "the query is empty"));
        }
        let indexer_ids = match params.indexer_ids {
            Some(ids) => ids,
            None => self.db.settings().await?.indexer_ids,
        };
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
        let releases = self
            .prowlarr
            .search(query, &indexer_ids, offset, limit)
            .await?;
        let next_offset = next_offset(offset, &releases);
        let releases: Vec<_> = releases
            .into_iter()
            .filter(|r| r.protocol.is_empty() || r.protocol == "torrent")
            .collect();
        let page = releases.iter().map(api::Release::from).collect();
        self.remember(releases);
        Ok(SearchPage {
            releases: page,
            next_offset,
        })
    }

    fn remember(&self, releases: Vec<prowlarr::Release>) {
        let Ok(mut cache) = self.releases.lock() else {
            return;
        };
        let now = Instant::now();
        if cache.len() + releases.len() > MAX_RELEASES {
            cache.retain(|_, (at, _)| now.duration_since(*at) < RELEASE_TTL);
        }
        if cache.len() + releases.len() > MAX_RELEASES {
            // Still too many: forget the older half.
            let mut ages: Vec<_> = cache.values().map(|(at, _)| *at).collect();
            ages.sort_unstable();
            if let Some(cutoff) = ages.get(ages.len() / 2).copied() {
                cache.retain(|_, (at, _)| *at > cutoff);
            }
        }
        for release in releases {
            cache.insert(release.guid.clone(), (now, release));
        }
    }

    fn release(&self, guid: &str) -> Option<prowlarr::Release> {
        let cache = self.releases.lock().ok()?;
        cache
            .get(guid)
            .filter(|(at, _)| at.elapsed() < RELEASE_TTL)
            .map(|(_, r)| r.clone())
    }

    pub async fn grab(&self, guid: &str) -> Result<(), Error> {
        let release = self.release(guid).ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                "this release is no longer known; search again",
            )
        })?;
        let (qbt, category) = self.target().await?;
        // Prowlarr's magnetUrl may itself be a link through Prowlarr that redirects to the magnet.
        let source = match (&release.magnet_url, &release.download_url) {
            (Some(magnet), _) if magnet.starts_with("magnet:") => Source::Url(magnet.clone()),
            (_, Some(url)) | (Some(url), None) => self.fetch_torrent(url).await?,
            (None, None) => {
                return Err(Error::new(
                    ErrorCode::Upstream,
                    "the release has no download link",
                ));
            }
        };
        qbt.add(source, &category).await?;
        tracing::info!(title = release.title, indexer = release.indexer, "Grabbed");
        self.torrents.refresh();
        Ok(())
    }

    /// Downloads a .torrent through Prowlarr, which may instead redirect to a magnet link.
    async fn fetch_torrent(&self, url: &str) -> Result<Source, Error> {
        let upstream = |m: String| Error::new(ErrorCode::Upstream, m);
        let mut url = url.to_owned();
        for _ in 0..5 {
            let mut request = self.downloads.get(&url);
            if url.starts_with(&self.config.prowlarr.url) {
                request = request.header("X-Api-Key", &self.config.prowlarr.api_key);
            }
            let mut response = request
                .send()
                .await
                .map_err(|e| upstream(format!("downloading the torrent failed: {e}")))?;
            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|l| l.to_str().ok())
                    .ok_or_else(|| upstream("a redirect without a location".into()))?;
                if location.starts_with("magnet:") {
                    return Ok(Source::Url(location.to_owned()));
                }
                url = response
                    .url()
                    .join(location)
                    .map_err(|e| upstream(format!("a bad redirect: {e}")))?
                    .into();
                continue;
            }
            if status != StatusCode::OK {
                return Err(upstream(format!(
                    "downloading the torrent failed with {status}"
                )));
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| upstream(format!("downloading the torrent failed: {e}")))?
            {
                bytes.extend_from_slice(&chunk);
                if bytes.len() > MAX_TORRENT_FILE {
                    return Err(upstream("the torrent file is too large".into()));
                }
            }
            if bytes.starts_with(b"magnet:") {
                return Ok(Source::Url(
                    String::from_utf8_lossy(&bytes).trim().to_owned(),
                ));
            }
            return Ok(Source::File(bytes));
        }
        Err(upstream("too many redirects".into()))
    }

    /// Removes torrents of the default category, with their files.
    pub async fn remove(&self, hashes: Vec<String>) -> Result<(), Error> {
        if hashes.is_empty() {
            return Ok(());
        }
        let (qbt, category) = self.target().await?;
        // Only torrents of the category may be removed: check against qBittorrent itself.
        let found = qbt.torrents(&category, Some(&hashes)).await?;
        let found: std::collections::HashSet<&str> = found
            .iter()
            .filter_map(|t| t.get("hash").and_then(|h| h.as_str()))
            .collect();
        if let Some(hash) = hashes.iter().find(|h| !found.contains(h.as_str())) {
            return Err(Error::new(
                ErrorCode::BadRequest,
                format!("torrent {hash} is not in the category {category}"),
            ));
        }
        qbt.delete(&hashes).await?;
        tracing::info!(count = hashes.len(), "Removed torrents");
        self.torrents.refresh();
        Ok(())
    }
}

/// Where the next page starts. Prowlarr hands the offset to every indexer, each answers with a page of its own size
/// (some ignore the limit), and indexers that can't page answer anything past the first page with nothing. So the
/// next page starts after the longest page any indexer gave, and an empty page is the end.
fn next_offset(offset: u32, releases: &[prowlarr::Release]) -> Option<u32> {
    let mut per_indexer: HashMap<i64, u32> = HashMap::new();
    for r in releases {
        *per_indexer.entry(r.indexer_id).or_default() += 1;
    }
    per_indexer
        .into_values()
        .max()
        .map(|longest| offset.saturating_add(longest))
}

/// A client is cached only while it's the default one.
async fn id_matches_settings(db: &Db, id: i64) -> bool {
    db.settings()
        .await
        .is_ok_and(|s| s.download_client_id == Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn releases(indexers: &[(i64, usize)]) -> Vec<prowlarr::Release> {
        indexers
            .iter()
            .flat_map(|&(id, n)| {
                (0..n).map(move |i| {
                    serde_json::from_value(serde_json::json!({
                        "guid": format!("{id}-{i}"),
                        "indexerId": id,
                    }))
                    .unwrap()
                })
            })
            .collect()
    }

    #[test]
    fn next_page_follows_the_longest_indexer_page() {
        assert_eq!(next_offset(0, &releases(&[(1, 100), (2, 25)])), Some(100));
        assert_eq!(next_offset(100, &releases(&[(2, 50)])), Some(150));
        assert_eq!(next_offset(200, &[]), None);
    }
}
