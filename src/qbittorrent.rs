//! The parts of the qBittorrent Web API grabarr uses.

use std::collections::BTreeMap;
use std::sync::Mutex;

use reqwest::{RequestBuilder, Response, StatusCode, header};
use serde_json::{Map, Value};

use crate::api::TorrentStatus;
use crate::prowlarr::truncate;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("qBittorrent request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("qBittorrent answered {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("qBittorrent refused the login; check the username and password")]
    Login,
    #[error(
        "qBittorrent requires a login, but there is no password: Prowlarr doesn't reveal it, set qbittorrent.password or qbittorrent.password_file"
    )]
    NoCredentials,
    #[error("qBittorrent didn't add the torrent")]
    AddFailed,
}

/// What to add.
pub enum Source {
    /// A magnet link, or a URL qBittorrent downloads itself.
    Url(String),
    /// The contents of a .torrent file.
    File(Vec<u8>),
}

pub struct Qbittorrent {
    http: reqwest::Client,
    url: String,
    username: Option<String>,
    password: Option<String>,
    /// `name=value` of the session cookie.
    cookie: Mutex<Option<String>>,
}

impl Qbittorrent {
    pub fn new(
        http: reqwest::Client,
        url: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            http,
            url,
            username,
            password,
            cookie: Mutex::new(None),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn cookie(&self) -> Option<String> {
        self.cookie.lock().ok().and_then(|c| c.clone())
    }

    fn set_cookie(&self, value: Option<String>) {
        if let Ok(mut c) = self.cookie.lock() {
            *c = value;
        }
    }

    async fn login(&self) -> Result<(), Error> {
        let (Some(username), Some(password)) = (&self.username, &self.password) else {
            return Err(Error::NoCredentials);
        };
        let response = self
            .http
            .post(format!("{}/api/v2/auth/login", self.url))
            .header(header::REFERER, &self.url)
            .form(&[("username", username), ("password", password)])
            .send()
            .await?;
        let status = response.status();
        let cookie = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|v| v.split(';').next())
            .find(|pair| pair.split('=').next().is_some_and(|n| n.contains("SID")))
            .map(str::to_owned);
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() || body.trim() == "Fails." {
            return Err(Error::Login);
        }
        // Without a cookie, authentication is bypassed for this client.
        self.set_cookie(cookie);
        Ok(())
    }

    /// Sends a request, logging in first when the session is missing or expired.
    async fn send(&self, build: impl Fn() -> RequestBuilder) -> Result<Response, Error> {
        let with_cookie = || {
            let request = build().header(header::REFERER, &self.url);
            match self.cookie() {
                Some(cookie) => request.header(header::COOKIE, cookie),
                None => request,
            }
        };
        let mut response = with_cookie().send().await?;
        if response.status() == StatusCode::FORBIDDEN {
            self.login().await?;
            response = with_cookie().send().await?;
        }
        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(Error::Status {
                status,
                body: truncate(body, 300),
            })
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/api/v2/{path}", self.url)
    }

    pub async fn categories(&self) -> Result<Vec<String>, Error> {
        let url = self.endpoint("torrents/categories");
        let categories: BTreeMap<String, Value> =
            self.send(|| self.http.get(&url)).await?.json().await?;
        Ok(categories.into_keys().collect())
    }

    /// The torrents of a category (or only those of `hashes` in it), each as qBittorrent's map of fields.
    pub async fn torrents(
        &self,
        category: &str,
        hashes: Option<&[String]>,
    ) -> Result<Vec<Map<String, Value>>, Error> {
        let url = self.endpoint("torrents/info");
        let mut query = vec![("category", category.to_owned())];
        if let Some(hashes) = hashes {
            query.push(("hashes", hashes.join("|")));
        }
        Ok(self
            .send(|| self.http.get(&url).query(&query))
            .await?
            .json()
            .await?)
    }

    pub async fn add(&self, source: Source, category: &str) -> Result<(), Error> {
        let url = self.endpoint("torrents/add");
        let response = self
            .send(|| {
                let form = reqwest::multipart::Form::new().text("category", category.to_owned());
                let form = match &source {
                    Source::Url(link) => form.text("urls", link.clone()),
                    Source::File(bytes) => form.part(
                        "torrents",
                        reqwest::multipart::Part::bytes(bytes.clone())
                            .file_name("release.torrent")
                            .mime_str("application/x-bittorrent")
                            .unwrap_or_else(|_| reqwest::multipart::Part::bytes(bytes.clone())),
                    ),
                };
                self.http.post(&url).multipart(form)
            })
            .await?;
        let body = response.text().await.unwrap_or_default();
        if body.trim() == "Fails." {
            return Err(Error::AddFailed);
        }
        Ok(())
    }

    /// Removes torrents and deletes their files.
    pub async fn delete(&self, hashes: &[String]) -> Result<(), Error> {
        let url = self.endpoint("torrents/delete");
        let hashes = hashes.join("|");
        self.send(|| {
            self.http
                .post(&url)
                .form(&[("hashes", hashes.as_str()), ("deleteFiles", "true")])
        })
        .await?;
        Ok(())
    }
}

/// Maps qBittorrent's torrent state (4.x and 5.x names) to a status.
pub fn status(state: &str) -> TorrentStatus {
    match state {
        "error" | "missingFiles" => TorrentStatus::Error,
        "uploading" | "forcedUP" | "stalledUP" => TorrentStatus::Seeding,
        "pausedUP" | "stoppedUP" | "queuedUP" => TorrentStatus::Completed,
        "checkingUP" | "checkingDL" | "checkingResumeData" => TorrentStatus::Checking,
        "downloading" | "forcedDL" | "allocating" => TorrentStatus::Downloading,
        "metaDL" | "forcedMetaDL" => TorrentStatus::Metadata,
        "pausedDL" | "stoppedDL" => TorrentStatus::Paused,
        "queuedDL" => TorrentStatus::Queued,
        "stalledDL" => TorrentStatus::Stalled,
        "moving" => TorrentStatus::Moving,
        _ => TorrentStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states() {
        assert_eq!(status("stoppedDL"), TorrentStatus::Paused);
        assert_eq!(status("pausedUP"), TorrentStatus::Completed);
        assert_eq!(status("stalledDL"), TorrentStatus::Stalled);
        assert_eq!(status("whatever"), TorrentStatus::Unknown);
    }
}
