//! The TOML configuration file.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reading {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    server: ServerFile,
    #[serde(default)]
    database: DatabaseFile,
    #[serde(default)]
    log: LogFile,
    prowlarr: ProwlarrFile,
    #[serde(default)]
    qbittorrent: QbittorrentFile,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerFile {
    #[serde(default = "default_listen")]
    listen: SocketAddr,
    #[serde(default)]
    allowed_origins: Vec<String>,
}

impl Default for ServerFile {
    fn default() -> Self {
        Self {
            listen: default_listen(),
            allowed_origins: Vec::new(),
        }
    }
}

fn default_listen() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 8080))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatabaseFile {
    #[serde(default = "default_database")]
    path: PathBuf,
}

impl Default for DatabaseFile {
    fn default() -> Self {
        Self {
            path: default_database(),
        }
    }
}

fn default_database() -> PathBuf {
    PathBuf::from("/data/grabarr.db")
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LogFile {
    #[serde(default)]
    format: LogFormat,
    level: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProwlarrFile {
    url: String,
    api_key: Option<String>,
    api_key_file: Option<PathBuf>,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}

fn default_timeout() -> u64 {
    90
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct QbittorrentFile {
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
    password_file: Option<PathBuf>,
    poll_interval_ms: Option<u64>,
}

/// The validated configuration, with secrets read from their files.
#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub allowed_origins: Vec<String>,
    pub database: PathBuf,
    pub log_format: LogFormat,
    pub log_level: Option<String>,
    pub prowlarr: Prowlarr,
    pub qbittorrent: Qbittorrent,
}

#[derive(Clone)]
pub struct Prowlarr {
    pub url: String,
    pub api_key: String,
    pub timeout: Duration,
}

impl std::fmt::Debug for Prowlarr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prowlarr")
            .field("url", &self.url)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// Overrides for what Prowlarr reports about the qBittorrent download client.
#[derive(Clone)]
pub struct Qbittorrent {
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub poll_interval: Duration,
}

impl std::fmt::Debug for Qbittorrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Qbittorrent")
            .field("url", &self.url)
            .field("username", &self.username)
            .field("poll_interval", &self.poll_interval)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(&text, path)
    }

    fn parse(text: &str, path: &Path) -> Result<Self, Error> {
        let file: File = toml::from_str(text).map_err(|source| Error::Parse {
            path: path.to_owned(),
            source: Box::new(source),
        })?;

        let api_key = secret(
            "prowlarr.api_key",
            file.prowlarr.api_key,
            file.prowlarr.api_key_file,
        )?
        .ok_or_else(|| {
            Error::Invalid("one of prowlarr.api_key and prowlarr.api_key_file is required".into())
        })?;
        let password = secret(
            "qbittorrent.password",
            file.qbittorrent.password,
            file.qbittorrent.password_file,
        )?;

        let url = file.prowlarr.url.trim_end_matches('/').to_owned();
        check_url("prowlarr.url", &url)?;
        let qbt_url = file
            .qbittorrent
            .url
            .map(|u| u.trim_end_matches('/').to_owned());
        if let Some(u) = &qbt_url {
            check_url("qbittorrent.url", u)?;
        }

        Ok(Self {
            listen: file.server.listen,
            allowed_origins: file.server.allowed_origins,
            database: file.database.path,
            log_format: file.log.format,
            log_level: file.log.level,
            prowlarr: Prowlarr {
                url,
                api_key,
                timeout: Duration::from_secs(file.prowlarr.timeout_secs.max(1)),
            },
            qbittorrent: Qbittorrent {
                url: qbt_url,
                username: file.qbittorrent.username,
                password,
                poll_interval: Duration::from_millis(
                    file.qbittorrent.poll_interval_ms.unwrap_or(2000).max(250),
                ),
            },
        })
    }
}

fn check_url(name: &str, url: &str) -> Result<(), Error> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "{name} must start with http:// or https://"
        )))
    }
}

/// A secret given either inline or as a file (a docker secret, for example) whose trimmed contents are the value.
fn secret(
    name: &str,
    inline: Option<String>,
    file: Option<PathBuf>,
) -> Result<Option<String>, Error> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(Error::Invalid(format!(
            "only one of {name} and {name}_file may be set"
        ))),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(path)) => {
            let value = std::fs::read_to_string(&path).map_err(|source| Error::Read {
                path: path.clone(),
                source,
            })?;
            Ok(Some(value.trim().to_owned()))
        }
        (None, None) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, Error> {
        Config::parse(text, Path::new("test.toml"))
    }

    #[test]
    fn minimal() {
        let config = parse(
            r#"
            [prowlarr]
            url = "http://prowlarr:9696/"
            api_key = "abc"
            "#,
        )
        .unwrap();
        assert_eq!(config.prowlarr.url, "http://prowlarr:9696");
        assert_eq!(config.listen.port(), 8080);
        assert_eq!(config.database, PathBuf::from("/data/grabarr.db"));
        assert_eq!(config.qbittorrent.poll_interval, Duration::from_secs(2));
    }

    #[test]
    fn secret_from_file() {
        let dir = std::env::temp_dir().join(format!("grabarr-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = dir.join("key");
        std::fs::write(&key, "  secret\n").unwrap();
        let config = parse(&format!(
            "[prowlarr]\nurl = \"http://p\"\napi_key_file = \"{}\"\n",
            key.display()
        ))
        .unwrap();
        assert_eq!(config.prowlarr.api_key, "secret");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_both_secret_forms() {
        let err = parse("[prowlarr]\nurl = \"http://p\"\napi_key = \"a\"\napi_key_file = \"/x\"\n")
            .unwrap_err();
        assert!(err.to_string().contains("only one of"), "{err}");
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(parse("[prowlarr]\nurl = \"http://p\"\napi_key = \"a\"\nfoo = 1\n").is_err());
    }

    #[test]
    fn example_parses() {
        let example = include_str!("../config.example.toml")
            .replace(
                "api_key_file = \"/run/secrets/prowlarr_api_key\"",
                "api_key = \"k\"",
            )
            .replace("# password = \"...\"", "password = \"p\"");
        let config = parse(&example).unwrap();
        assert_eq!(config.prowlarr.api_key, "k");
        assert_eq!(config.qbittorrent.password.as_deref(), Some("p"));
        assert_eq!(config.qbittorrent.poll_interval, Duration::from_secs(2));
    }

    #[test]
    fn requires_api_key() {
        assert!(parse("[prowlarr]\nurl = \"http://p\"\n").is_err());
    }
}
