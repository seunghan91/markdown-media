use crate::models::HistoryEntry;
use chrono::Local;
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct HistoryStore {
    db_path: PathBuf,
}

impl HistoryStore {
    pub fn new() -> Result<Self, String> {
        let base_dir = dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(std::env::temp_dir)
            .join("mdm-desktop");

        fs::create_dir_all(&base_dir).map_err(|error| error.to_string())?;
        let db_path = base_dir.join("history.sqlite3");

        let store = Self { db_path };
        store.initialize()?;
        Ok(store)
    }

    fn open(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path).map_err(|error| error.to_string())
    }

    fn initialize(&self) -> Result<(), String> {
        let connection = self.open()?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS history_entries (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  file_name TEXT NOT NULL,
                  file_path TEXT NOT NULL,
                  direction TEXT NOT NULL,
                  output_format TEXT NOT NULL,
                  created_at TEXT NOT NULL,
                  status TEXT NOT NULL
                );
                "#,
            )
            .map_err(|error| error.to_string())
    }

    pub fn record(
        &self,
        input_path: &Path,
        direction: &str,
        output_format: &str,
        status: &str,
    ) -> Result<(), String> {
        let file_name = input_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| input_path.to_string_lossy().to_string());

        let created_at = Local::now().to_rfc3339();
        let connection = self.open()?;

        connection
            .execute(
                r#"
                INSERT INTO history_entries (
                  file_name, file_path, direction, output_format, created_at, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    file_name,
                    input_path.to_string_lossy().to_string(),
                    direction,
                    output_format,
                    created_at,
                    status
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<HistoryEntry>, String> {
        let connection = self.open()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, file_name, file_path, direction, output_format, created_at, status
                FROM history_entries
                ORDER BY id DESC
                LIMIT ?1
                "#,
            )
            .map_err(|error| error.to_string())?;

        let rows = statement
            .query_map([limit as i64], |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    file_name: row.get(1)?,
                    file_path: row.get(2)?,
                    direction: row.get(3)?,
                    output_format: row.get(4)?,
                    created_at: row.get(5)?,
                    status: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }
}
