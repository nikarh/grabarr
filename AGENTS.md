When adding dependencies don't edit manifests by hand - use the package managers (`cargo add`, `npm install <pkg>@latest`)
so the latest versions are used.
Commit your changes. Use conventional commits. Split logical changes into individual commits.
Before committing run `mise run check` (formatting, clippy, oxlint, type check, tests, cargo-deny, and that the
generated files are up to date).

Commit messages, code, tests and docs describe bugs and their fixes in general terms: what goes wrong, under which
conditions, and what changed. Never frame them around a particular deployment, and don't carry over its details.

## Layout

- `src/` - the Rust server. `api.rs` holds every type of the WebSocket protocol; `rpc.rs` runs it; `app.rs` is what the
  methods do; `prowlarr.rs` and `qbittorrent.rs` are the upstream clients; `torrents.rs` is the shared qBittorrent
  poller behind `torrents.subscribe`.
- `web/` - the React frontend. `web/src/api/generated/` is generated from `src/api.rs` by `mise run bindings`
  (`cargo test` does it too); never edit it by hand.
- `migrations/` - SQLite migrations. After changing a migration or a `query!`, run `mise run db:prepare` and commit
  `.sqlx/`.

## Rules

- A change to the protocol changes `src/api.rs`, then the bindings, then the frontend, in the same commit.
- A change to the configuration updates `README.md` and `config.example.toml` in the same commit.
- The container image must keep working read-only, as any uid, with no capabilities: no writes outside the database
  directory, no listening on ports below 1024, nothing needing a shell or files from a distribution.
