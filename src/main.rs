//! grabarr: search Prowlarr, grab releases to qBittorrent, watch the downloads.

mod api;
mod app;
mod config;
mod db;
mod prowlarr;
mod qbittorrent;
mod rpc;
mod torrents;
mod web;

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Request, State, WebSocketUpgrade};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tracing_subscriber::EnvFilter;

use crate::app::App;
use crate::config::{Config, LogFormat};

const HELP: &str = "\
grabarr - search Prowlarr, grab releases to qBittorrent, watch the downloads

Usage:
  grabarr [--config <path>]              Run the server
  grabarr healthcheck [--config <path>]  Exit 0 if the server answers on its listen address

Options:
  -c, --config <path>  The configuration file [env: GRABARR_CONFIG] [default: /config/config.toml]
  -h, --help           Print this help
  -V, --version        Print the version

Logging: RUST_LOG overrides the level from the configuration (e.g. RUST_LOG=grabarr=debug).
";

struct Args {
    healthcheck: bool,
    config: PathBuf,
}

enum Parsed {
    Run(Args),
    Help,
    Version,
}

fn parse_args() -> Result<Parsed, pico_args::Error> {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        return Ok(Parsed::Help);
    }
    if args.contains(["-V", "--version"]) {
        return Ok(Parsed::Version);
    }
    let healthcheck = args.subcommand()?.as_deref() == Some("healthcheck");
    let config = args
        .opt_value_from_str(["-c", "--config"])?
        .or_else(|| std::env::var_os("GRABARR_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/config/config.toml"));
    let rest = args.finish();
    if !rest.is_empty() {
        return Err(pico_args::Error::ArgumentParsingFailed {
            cause: format!("unexpected arguments: {rest:?}"),
        });
    }
    Ok(Parsed::Run(Args {
        healthcheck,
        config,
    }))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Parsed::Run(args)) => args,
        Ok(Parsed::Help) => return write_out(HELP),
        Ok(Parsed::Version) => {
            return write_out(concat!("grabarr ", env!("CARGO_PKG_VERSION"), "\n"));
        }
        Err(e) => return fail(&format!("{e}\n\n{HELP}")),
    };
    let config = match Config::load(&args.config) {
        Ok(config) => config,
        Err(e) => return fail(&e.to_string()),
    };
    if args.healthcheck {
        return healthcheck(config.listen);
    }

    init_logging(&config);
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return fail(&format!("starting the runtime: {e}")),
    };
    match runtime.block_on(run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn write_out(text: &str) -> ExitCode {
    let _ = std::io::stdout().write_all(text.as_bytes());
    ExitCode::SUCCESS
}

fn fail(message: &str) -> ExitCode {
    let _ = writeln!(std::io::stderr(), "grabarr: {message}");
    ExitCode::FAILURE
}

fn init_logging(config: &Config) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.log_level.as_deref().unwrap_or("info")));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match config.log_format {
        LogFormat::Text => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

/// A plain HTTP/1.0 request to `/healthz`: scratch images have no curl.
fn healthcheck(listen: SocketAddr) -> ExitCode {
    let mut addr = listen;
    if addr.ip().is_unspecified() {
        addr.set_ip(if addr.is_ipv4() {
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    let check = || -> std::io::Result<bool> {
        let timeout = Duration::from_secs(3);
        let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        stream.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
        let mut response = String::new();
        stream.take(1024).read_to_string(&mut response)?;
        Ok(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"))
    };
    match check() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => fail("unhealthy"),
        Err(e) => fail(&format!("{addr}: {e}")),
    }
}

#[derive(Debug, thiserror::Error)]
enum RunError {
    #[error("opening the database {0}: {1}")]
    Db(PathBuf, sqlx::Error),
    #[error("creating the HTTP client: {0}")]
    Http(#[from] reqwest::Error),
    #[error("listening on {0}: {1}")]
    Listen(SocketAddr, std::io::Error),
    #[error("serving: {0}")]
    Serve(std::io::Error),
}

#[derive(Clone)]
struct AppState {
    app: Arc<App>,
    allowed_origins: Arc<[String]>,
}

async fn run(config: Config) -> Result<(), RunError> {
    // reqwest is built without a default crypto provider: ring builds much faster than aws-lc.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let db = db::Db::open(&config.database)
        .await
        .map_err(|e| RunError::Db(config.database.clone(), e))?;
    let listen = config.listen;
    let state = AppState {
        allowed_origins: config.allowed_origins.clone().into(),
        app: App::new(config, db)?,
    };

    let router = Router::new()
        .route("/api/ws", get(websocket))
        .route("/healthz", get(|| async { "ok" }))
        .fallback(web::serve)
        .layer(middleware::from_fn(security_headers))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|e| RunError::Listen(listen, e))?;
    tracing::info!(%listen, version = env!("CARGO_PKG_VERSION"), "Listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(RunError::Serve)
}

async fn shutdown() {
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        () = terminate => {},
    }
    tracing::info!("Shutting down");
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; connect-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
             base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

async fn websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !origin_allowed(&headers, &state.allowed_origins) {
        tracing::warn!(origin = ?headers.get(header::ORIGIN), "Refused a WebSocket from another origin");
        return StatusCode::FORBIDDEN.into_response();
    }
    upgrade
        .max_message_size(1024 * 1024)
        .on_upgrade(move |socket| rpc::serve(socket, state.app))
}

/// Browsers open `WebSocket`s cross-site with the user's cookies, which a proxy in front may authenticate by: only
/// pages of this origin (or explicitly allowed ones) may connect.
fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|o| o.to_str().ok()) else {
        // Not a browser.
        return true;
    };
    if allowed.iter().any(|a| a.trim_end_matches('/') == origin) {
        return true;
    }
    let Some(origin_host) = origin.split_once("://").map(|(_, host)| host) else {
        return false;
    };
    [header::HOST.as_str(), "x-forwarded-host"]
        .iter()
        .filter_map(|name| headers.get(*name).and_then(|h| h.to_str().ok()))
        .flat_map(|h| h.split(','))
        .any(|host| host.trim().eq_ignore_ascii_case(origin_host))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    axum::http::HeaderName::from_static(k),
                    HeaderValue::from_static(v),
                )
            })
            .collect()
    }

    #[test]
    fn origins() {
        let none: &[String] = &[];
        assert!(origin_allowed(&headers(&[("host", "a.example")]), none));
        assert!(origin_allowed(
            &headers(&[("host", "a.example"), ("origin", "https://a.example")]),
            none
        ));
        assert!(origin_allowed(
            &headers(&[
                ("host", "grabarr:8080"),
                ("x-forwarded-host", "a.example"),
                ("origin", "https://a.example")
            ]),
            none
        ));
        assert!(!origin_allowed(
            &headers(&[("host", "a.example"), ("origin", "https://evil.example")]),
            none
        ));
        assert!(origin_allowed(
            &headers(&[("host", "a.example"), ("origin", "http://localhost:5173")]),
            &["http://localhost:5173/".to_owned()]
        ));
    }
}
