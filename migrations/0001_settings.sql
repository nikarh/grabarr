-- Global settings: a single row.
CREATE TABLE settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    -- Prowlarr's id of the qBittorrent download client grabs go to.
    download_client_id INTEGER,
    -- qBittorrent category grabs get, and the downloads screen shows.
    category TEXT
) STRICT;

INSERT INTO settings (id) VALUES (1);

-- Indexers searched when a search names none.
CREATE TABLE default_indexers (
    indexer_id INTEGER PRIMARY KEY NOT NULL
) STRICT;
