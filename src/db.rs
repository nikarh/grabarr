//! The SQLite database: global settings.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

use crate::api::Settings;

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn open(path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str("sqlite://")?
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            // Scratch images have no temporary directory.
            .pragma("temp_store", "MEMORY");
        Self::connect(options).await
    }

    #[cfg(test)]
    pub async fn memory() -> Result<Self, sqlx::Error> {
        Self::connect(SqliteConnectOptions::from_str("sqlite::memory:")?).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            // In memory, every connection would be a database of its own.
            .max_connections(if options.get_filename() == Path::new(":memory:") {
                1
            } else {
                4
            })
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn settings(&self) -> Result<Settings, sqlx::Error> {
        let row = sqlx::query!("SELECT download_client_id, category FROM settings WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;
        let indexer_ids =
            sqlx::query_scalar!("SELECT indexer_id FROM default_indexers ORDER BY indexer_id")
                .fetch_all(&self.pool)
                .await?;
        Ok(Settings {
            download_client_id: row.download_client_id,
            category: row.category,
            indexer_ids,
        })
    }

    pub async fn save_settings(&self, settings: &Settings) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "UPDATE settings SET download_client_id = ?, category = ? WHERE id = 1",
            settings.download_client_id,
            settings.category,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("DELETE FROM default_indexers")
            .execute(&mut *tx)
            .await?;
        for id in &settings.indexer_ids {
            sqlx::query!(
                "INSERT OR IGNORE INTO default_indexers (indexer_id) VALUES (?)",
                id
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn settings_round_trip() {
        let db = Db::memory().await.unwrap();
        assert_eq!(db.settings().await.unwrap(), Settings::default());

        let settings = Settings {
            download_client_id: Some(3),
            category: Some("linux".into()),
            indexer_ids: vec![1, 5],
        };
        db.save_settings(&settings).await.unwrap();
        assert_eq!(db.settings().await.unwrap(), settings);

        db.save_settings(&Settings::default()).await.unwrap();
        assert_eq!(db.settings().await.unwrap(), Settings::default());
    }
}
