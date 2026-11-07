//! The parts of the Prowlarr API grabarr uses.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::api;
use crate::config;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Prowlarr request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Prowlarr answered {status}: {body}")]
    Status {
        status: reqwest::StatusCode,
        body: String,
    },
}

#[derive(Clone)]
pub struct Prowlarr {
    http: reqwest::Client,
    url: String,
    api_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Indexer {
    id: i64,
    name: String,
    enable: bool,
    protocol: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadClient {
    id: i64,
    name: String,
    enable: bool,
    implementation: String,
    #[serde(default)]
    fields: Vec<Field>,
}

#[derive(Debug, Deserialize)]
struct Field {
    name: String,
    #[serde(default)]
    value: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Category {
    id: i64,
    name: String,
    #[serde(default)]
    sub_categories: Vec<Category>,
}

impl From<Category> for api::SearchCategory {
    fn from(c: Category) -> Self {
        Self {
            id: c.id,
            name: c.name,
            subcategories: c.sub_categories.into_iter().map(Into::into).collect(),
        }
    }
}

/// A search result as Prowlarr returns it.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub guid: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub size: u64,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    #[serde(default)]
    pub indexer: String,
    pub indexer_id: i64,
    pub publish_date: Option<String>,
    pub info_url: Option<String>,
    pub download_url: Option<String>,
    pub magnet_url: Option<String>,
    #[serde(default)]
    pub protocol: String,
}

impl From<&Release> for api::Release {
    fn from(r: &Release) -> Self {
        Self {
            guid: r.guid.clone(),
            title: r.title.clone(),
            size: r.size,
            seeders: r.seeders,
            leechers: r.leechers,
            indexer: r.indexer.clone(),
            indexer_id: r.indexer_id,
            publish_date: r.publish_date.clone(),
            info_url: r.info_url.clone().filter(|u| is_web_url(u)),
        }
    }
}

/// Only links the browser can open as a page; anything else (`javascript:`, ...) is dropped.
fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// How to reach a qBittorrent download client, as configured in Prowlarr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QbittorrentSettings {
    pub url: String,
    pub username: Option<String>,
    /// `None` when Prowlarr has none, or doesn't reveal it.
    pub password: Option<String>,
}

impl Prowlarr {
    pub fn new(http: reqwest::Client, config: &config::Prowlarr) -> Self {
        Self {
            http,
            url: config.url.clone(),
            api_key: config.api_key.clone(),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, Error> {
        let response = self
            .http
            .get(format!("{}/api/v1/{path}", self.url))
            .header("X-Api-Key", &self.api_key)
            .query(query)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Status {
                status,
                body: truncate(error_message(body), 300),
            });
        }
        Ok(response.json().await?)
    }

    /// Torrent indexers.
    pub async fn indexers(&self) -> Result<Vec<api::Indexer>, Error> {
        let indexers: Vec<Indexer> = self.get("indexer", &[]).await?;
        let mut indexers: Vec<_> = indexers
            .into_iter()
            .filter(|i| i.protocol == "torrent")
            .map(|i| api::Indexer {
                id: i.id,
                name: i.name,
                enabled: i.enable,
            })
            .collect();
        indexers.sort_by_cached_key(|i| i.name.to_lowercase());
        Ok(indexers)
    }

    async fn qbittorrent_clients(&self) -> Result<Vec<DownloadClient>, Error> {
        let clients: Vec<DownloadClient> = self.get("downloadclient", &[]).await?;
        Ok(clients
            .into_iter()
            .filter(|c| c.implementation.eq_ignore_ascii_case("QBittorrent"))
            .collect())
    }

    /// The qBittorrent download clients.
    pub async fn download_clients(&self) -> Result<Vec<api::DownloadClient>, Error> {
        let mut clients: Vec<_> = self
            .qbittorrent_clients()
            .await?
            .into_iter()
            .map(|c| api::DownloadClient {
                id: c.id,
                name: c.name,
                enabled: c.enable,
            })
            .collect();
        clients.sort_by_cached_key(|c| c.name.to_lowercase());
        Ok(clients)
    }

    /// Connection settings of a qBittorrent download client, `None` if there is no such client.
    pub async fn qbittorrent(&self, id: i64) -> Result<Option<QbittorrentSettings>, Error> {
        let clients = self.qbittorrent_clients().await?;
        Ok(clients
            .into_iter()
            .find(|c| c.id == id)
            .map(|c| qbittorrent_settings(&c.fields)))
    }

    /// The search categories. Prowlarr also lists a legacy "Other" with id 0, a duplicate of 8000: left out.
    pub async fn search_categories(&self) -> Result<Vec<api::SearchCategory>, Error> {
        let categories: Vec<Category> = self.get("indexer/categories", &[]).await?;
        Ok(categories
            .into_iter()
            .filter(|c| c.id > 0)
            .map(Into::into)
            .collect())
    }

    pub async fn search(
        &self,
        query: &str,
        indexer_ids: &[i64],
        categories: &[i64],
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Release>, Error> {
        let mut params = vec![
            ("query", query.to_owned()),
            ("type", "search".to_owned()),
            ("offset", offset.to_string()),
            ("limit", limit.to_string()),
        ];
        params.extend(indexer_ids.iter().map(|id| ("indexerIds", id.to_string())));
        params.extend(categories.iter().map(|id| ("categories", id.to_string())));
        self.get("search", &params).await
    }
}

fn qbittorrent_settings(fields: &[Field]) -> QbittorrentSettings {
    let map: HashMap<&str, &Value> = fields.iter().map(|f| (f.name.as_str(), &f.value)).collect();
    let text = |name: &str| {
        map.get(name)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let host = text("host").unwrap_or_else(|| "localhost".to_owned());
    let port = map.get("port").and_then(|v| v.as_u64()).unwrap_or(8080);
    let tls = map.get("useSsl").and_then(|v| v.as_bool()).unwrap_or(false);
    let base = text("urlBase").map_or_else(String::new, |b| format!("/{}", b.trim_matches('/')));
    let scheme = if tls { "https" } else { "http" };
    // Prowlarr masks secrets in its API responses.
    let password = text("password").filter(|p| p.chars().any(|c| c != '*'));
    QbittorrentSettings {
        url: format!("{scheme}://{host}:{port}{base}"),
        username: text("username"),
        password,
    }
}

/// Prowlarr answers errors with `{"message": ..., "description": <stack trace>}`: only the message is of use.
fn error_message(body: String) -> String {
    #[derive(Deserialize)]
    struct Body {
        message: String,
    }
    match serde_json::from_str::<Body>(&body) {
        Ok(b) if !b.message.trim().is_empty() => b.message,
        _ => body,
    }
}

pub fn truncate(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(json: &str) -> Vec<Field> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn qbittorrent_from_fields() {
        let s = qbittorrent_settings(&fields(
            r#"[{"name":"host","value":"qbt"},{"name":"port","value":8081},{"name":"useSsl","value":true},
                {"name":"urlBase","value":"/qb/"},{"name":"username","value":"admin"},
                {"name":"password","value":"********"},{"name":"category","value":"prowlarr"}]"#,
        ));
        assert_eq!(
            s,
            QbittorrentSettings {
                url: "https://qbt:8081/qb".into(),
                username: Some("admin".into()),
                password: None,
            }
        );
    }

    #[test]
    fn qbittorrent_defaults() {
        let s = qbittorrent_settings(&fields(
            r#"[{"name":"host","value":"qbt"},{"name":"password","value":"pw"},{"name":"urlBase"}]"#,
        ));
        assert_eq!(s.url, "http://qbt:8080");
        assert_eq!(s.password.as_deref(), Some("pw"));
    }

    #[test]
    fn drops_non_web_info_urls() {
        let mut release: Release =
            serde_json::from_str(r#"{"guid":"g","indexerId":1,"infoUrl":"javascript:alert(1)"}"#)
                .unwrap();
        assert_eq!(api::Release::from(&release).info_url, None);
        release.info_url = Some("https://example.org/t/1".into());
        assert!(api::Release::from(&release).info_url.is_some());
    }

    #[test]
    fn error_messages() {
        assert_eq!(
            error_message(
                r#"{"message":"Search failed due to all selected indexers being unavailable","description":"NzbDrone..."}"#
                    .into()
            ),
            "Search failed due to all selected indexers being unavailable"
        );
        assert_eq!(error_message("Unauthorized".into()), "Unauthorized");
    }

    #[test]
    fn truncates_on_char_boundary() {
        assert_eq!(truncate("äää".into(), 3), "ä…");
        assert_eq!(truncate("abc".into(), 3), "abc");
    }
}
