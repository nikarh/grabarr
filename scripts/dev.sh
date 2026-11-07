#!/usr/bin/env bash
# Runs the backend (restarted by cargo-watch-free polling: rerun `mise run dev` after Rust changes) and the Vite dev
# server, which proxies /api to the backend.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

config=${GRABARR_CONFIG:-dev/config.toml}
if [[ ! -f $config ]]; then
  echo "Create $config first (see config.example.toml); use database.path = \"dev/grabarr.db\"." >&2
  exit 1
fi

trap 'kill 0' EXIT
cargo run -- --config "$config" &
(cd web && npm run dev) &
wait -n
