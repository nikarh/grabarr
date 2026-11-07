# grabarr

A small, mobile-first web UI for one job: search [Prowlarr](https://prowlarr.com), grab what you pick to
qBittorrent, and watch it download.

- **Search** the default indexers, or others for one search. Results load as you scroll (no page numbers, nothing to
  lose by going back). Each result links to its page on the indexer.
- **Grab** one or many results at once. They go to the default download client, with the default category.
- **Downloads** shows every torrent of that category live: status, progress, speed, ETA. Torrents can be deleted from
  here, files included.
- **Settings**: the download client (any qBittorrent client configured in Prowlarr), the category and the default
  indexers. Stored in SQLite, shared by everyone using the instance.

There's no authentication: run it behind a reverse proxy that does it.

The whole thing is one static binary (Rust, with the React frontend embedded) shipped in a `FROM scratch` image of
about 10 MB that runs as `nobody`, on a read-only filesystem, without any capabilities.

## Running it with Docker Compose

```yaml
services:
  grabarr:
    image: ghcr.io/nikarh/grabarr:latest
    restart: unless-stopped
    # Everything below is optional hardening; the image works with all of it.
    read_only: true
    cap_drop: [ALL]
    security_opt: [no-new-privileges:true]
    user: "65534:65534"
    volumes:
      - grabarr-data:/data
      - ./grabarr.toml:/config/config.toml:ro
    secrets:
      - prowlarr_api_key
      - qbittorrent_password
    networks: [media]
    # Or publish a port when there's no proxy on the same network: ports: ["127.0.0.1:8080:8080"]
    deploy:
      resources:
        limits:
          memory: 64M

volumes:
  grabarr-data:

secrets:
  prowlarr_api_key:
    file: ./secrets/prowlarr_api_key
  qbittorrent_password:
    file: ./secrets/qbittorrent_password

networks:
  media:
    external: true
```

`grabarr.toml`, with the secrets read from the files compose mounts under `/run/secrets/`:

```toml
[prowlarr]
url = "http://prowlarr:9696"
api_key_file = "/run/secrets/prowlarr_api_key"

[qbittorrent]
password_file = "/run/secrets/qbittorrent_password"
```

Then open it, choose the download client, the category and the default indexers in Settings, and search.

### Hardening options

| Option | What it does | Needed by grabarr? |
| --- | --- | --- |
| `read_only: true` | The container's filesystem is read-only. | Only `/data` is ever written to (the database, with its `-wal` and `-shm` files). No tmpfs is needed: there are no temporary files. |
| `cap_drop: [ALL]` | Drops every Linux capability. | None are used. It listens on 8080, an unprivileged port. |
| `security_opt: [no-new-privileges:true]` | Nothing in the container can gain privileges (setuid, file capabilities). | There's nothing to gain them with: the image holds only the binary and CA certificates. |
| `user: "uid:gid"` | Which user the process runs as. | Any. The image defaults to `65534:65534` (nobody). `/data` in the image is mode `1777`, and a new named volume copies that, so any uid can create the database there. With a bind mount, make the directory writable by that uid. |
| Memory limit | | It needs a few MB; 64M is plenty. |

The image has no shell, package manager or libc: `docker exec` into it won't work, by design. The healthcheck is
built in (`/grabarr healthcheck`, a plain HTTP request to its own `/healthz`) and declared in the image; to declare it
in compose instead:

```yaml
    healthcheck:
      test: ["CMD", "/grabarr", "healthcheck"]
      interval: 30s
```

### Behind a reverse proxy

The proxy must pass WebSocket upgrades on `/api/ws` (Traefik and Caddy do that by default; nginx needs the usual
`Upgrade`/`Connection` headers). The page may be served under a path prefix, as long as the proxy strips it.

Browsers open WebSockets cross-site with the user's cookies, so a page on another site could otherwise drive grabarr
through a proxy's cookie-based authentication. grabarr accepts a WebSocket only when its `Origin` matches the `Host`
(or `X-Forwarded-Host`) of the request; add other origins to `server.allowed_origins` if needed.

### Reaching qBittorrent

grabarr takes the address and username of the qBittorrent client chosen in Settings from Prowlarr, so it has to be
able to reach qBittorrent under the same name. Prowlarr doesn't reveal passwords, though: give it in
`qbittorrent.password` or `qbittorrent.password_file`. If qBittorrent skips authentication for grabarr's subnet
("Bypass authentication for clients in whitelisted IP subnets"), leave both out: grabarr then never logs in, and
only asks for a password if qBittorrent refuses it.
`qbittorrent.url` and `qbittorrent.username` override what Prowlarr has.

qBittorrent rejects requests whose `Host` header names another port than its own (its *host header validation*), so
reach it on the port it listens on, not through a remapped one.

## Configuration

A TOML file, `/config/config.toml` by default (`--config <path>` or `GRABARR_CONFIG` to change it).
[`config.example.toml`](config.example.toml) lists every setting with its default. Every secret `x` can instead be a
file, `x_file`, whose trimmed contents are the value.

## The API

The UI talks to the server over one WebSocket, `/api/ws`, with a small JSON RPC protocol. Every message type is defined
in [`src/api.rs`](src/api.rs), from which the TypeScript types in `web/src/api/generated/` are generated.

- The client sends a request `{"id": "1", "method": "search", "params": {"query": "debian"}}`, or an array of them.
  The ids are the client's own choice.
- Each request gets its own answer as soon as it's done, `{"type": "result", "id": "1", "result": ...}` or
  `{"type": "error", "id": "1", "error": {"code": "upstream", "message": "..."}}`, so the requests of one array run
  concurrently and answer in any order. The UI batches every call made in the same tick into one frame; grabbing ten
  results is one frame and ten answers.
- `torrents.subscribe` answers `null`, then pushes `{"type": "event", "id": "<its id>", "event": ...}`: a snapshot
  of the category's torrents, then only what changed. `unsubscribe` with that id, or disconnecting, ends it. One
  poller of qBittorrent serves all subscribers and stops when none are left.

| Method | Params | Result |
| --- | --- | --- |
| `settings.get` | | `Settings` |
| `settings.update` | `Settings` | `Settings` |
| `indexers.list` | | `Indexer[]` (torrent indexers) |
| `downloadClients.list` | | `DownloadClient[]` (qBittorrent clients) |
| `categories.list` | `{downloadClientId}` | `string[]` |
| `search` | `{query, indexerIds?, offset?, limit?}` | `{releases, nextOffset}` |
| `grab` | `{guid}` | `null` |
| `torrents.subscribe` | | `null`, then `TorrentsEvent`s |
| `torrents.remove` | `{hashes}` | `null` (only torrents of the category; files are deleted) |
| `unsubscribe` | `{id}` | `null` |

Grabs name a release by the `guid` of a recent search, and the server downloads it through the link Prowlarr gave:
clients can't make the server fetch arbitrary URLs.

## Development

[mise](https://mise.jdx.dev) installs the tools (Rust comes from `rust-toolchain.toml` through rustup) and runs the
tasks:

| Task | |
| --- | --- |
| `mise run dev` | The backend with `dev/config.toml`, and the Vite dev server at http://localhost:5173 |
| `mise run check` | Everything CI checks: formatting, clippy, oxlint, type check, tests, cargo-deny, generated files |
| `mise run fmt` | Format Rust and TypeScript |
| `mise run bindings` | Regenerate the TypeScript API types after changing `src/api.rs` |
| `mise run db:prepare` | Refresh `.sqlx/` after changing a query or a migration |
| `mise run build` | Release binary with the frontend embedded |
| `mise run docker` | Build the image as `grabarr:dev` (`CONTAINER_ENGINE=podman` for podman) |
| `mise run changelog` | Regenerate `CHANGELOG.md` from the commits |

For `mise run dev`, create `dev/config.toml` (the `dev/` directory is ignored by git) from the example, with
`database.path = "dev/grabarr.db"`, `server.listen = "127.0.0.1:8080"`, and
`server.allowed_origins = ["http://localhost:5173"]`.

Commits follow [Conventional Commits](https://www.conventionalcommits.org); the changelog is generated from them by
[git-cliff](https://git-cliff.org).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
