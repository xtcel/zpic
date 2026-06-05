//! SQLite-backed upload history implementation.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use rusqlite::{params, Connection, OptionalExtension};

use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::UploadOutput;

/// One row in the `uploads` table.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub source_path: Option<String>,
    pub uploader: String,
    pub key: String,
    pub url: String,
    pub markdown: String,
    pub mime: Option<String>,
    pub size: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub status: String,
}

/// Filter for `HistoryStore::list`.
#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub uploader: Option<String>,
    pub limit: Option<usize>,
}

/// SQLite-backed history store.
pub struct HistoryStore {
    conn: Connection,
}

impl HistoryStore {
    /// Open (and create) the history store at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(|e| ZpicError::History(format!("open: {e}")))?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    /// Open the default history store (XDG / platform-appropriate).
    pub fn open_default() -> Result<Self> {
        let path = default_history_path();
        Self::open(path)
    }

    /// Open an in-memory store. Useful for tests.
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().map_err(|e| ZpicError::History(format!("memory: {e}")))?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn
            .execute_batch(SCHEMA)
            .map_err(|e| ZpicError::History(format!("migrate: {e}")))?;
        Ok(())
    }

    /// Persist a successful upload.
    pub fn record(&self, out: &UploadOutput, source_path: Option<&str>) -> Result<String> {
        let id = uuid_v4();
        let now = Utc::now();
        self.conn
            .execute(
                "INSERT INTO uploads (id, created_at, source_path, uploader, key, url, markdown, mime, size, width, height, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    id,
                    now.to_rfc3339(),
                    source_path,
                    out.uploader,
                    out.key,
                    out.url,
                    out.markdown,
                    out.mime,
                    out.size as i64,
                    out.width.map(|w| w as i64),
                    out.height.map(|h| h as i64),
                    "ok",
                ],
            )
            .map_err(|e| ZpicError::History(format!("insert: {e}")))?;
        Ok(id)
    }

    /// List entries, most recent first.
    pub fn list(&self, filter: ListFilter) -> Result<Vec<HistoryEntry>> {
        let mut sql = String::from(
            "SELECT id, created_at, source_path, uploader, key, url, markdown, mime, size, width, height, status \
             FROM uploads",
        );
        let mut clauses: Vec<String> = Vec::new();
        let mut bindings: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(u) = &filter.uploader {
            clauses.push("uploader = ?".to_string());
            bindings.push(Box::new(u.clone()));
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| ZpicError::History(format!("prepare: {e}")))?;
        let params_refs: Vec<&dyn rusqlite::ToSql> = bindings.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), row_to_entry)
            .map_err(|e| ZpicError::History(format!("query: {e}")))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| ZpicError::History(format!("row: {e}")))?);
        }
        Ok(out)
    }

    /// Look up a single entry by id.
    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Result<Option<HistoryEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, created_at, source_path, uploader, key, url, markdown, mime, size, width, height, status \
                 FROM uploads WHERE id = ?1",
            )
            .map_err(|e| ZpicError::History(format!("prepare: {e}")))?;
        let row = stmt
            .query_row(params![id], row_to_entry)
            .optional()
            .map_err(|e| ZpicError::History(format!("query: {e}")))?;
        Ok(row)
    }

    /// Delete an entry by id. Returns `true` if a row was removed.
    #[allow(dead_code)]
    pub fn delete(&self, id: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM uploads WHERE id = ?1", params![id])
            .map_err(|e| ZpicError::History(format!("delete: {e}")))?;
        Ok(n > 0)
    }
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let created_at: String = row.get(1)?;
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Ok(HistoryEntry {
        id: row.get(0)?,
        created_at,
        source_path: row.get(2)?,
        uploader: row.get(3)?,
        key: row.get(4)?,
        url: row.get(5)?,
        markdown: row.get(6)?,
        mime: row.get(7)?,
        size: row.get(8)?,
        width: row.get(9)?,
        height: row.get(10)?,
        status: row.get(11)?,
    })
}

/// Default XDG-aware path for the history database.
pub fn default_history_path() -> PathBuf {
    if let Some(p) = ProjectDirs::from("io", "zpic", "zpic") {
        return p.data_dir().join("history.db");
    }
    PathBuf::from(".zpic-history.db")
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS uploads (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    source_path TEXT,
    uploader TEXT NOT NULL,
    key TEXT NOT NULL,
    url TEXT NOT NULL,
    markdown TEXT NOT NULL,
    mime TEXT,
    size INTEGER,
    width INTEGER,
    height INTEGER,
    status TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_uploads_uploader ON uploads(uploader);
CREATE INDEX IF NOT EXISTS idx_uploads_created_at ON uploads(created_at DESC);
"#;

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Tiny self-contained UUIDv4 (no external crate dep) so the history
    // store works even when uuid is not in scope.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = nanos as u128;
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&(n as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&((n >> 64) as u64).to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut out = String::with_capacity(36);
    for (i, b) in bytes.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            out.push('-');
        }
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_output() -> UploadOutput {
        UploadOutput {
            source: "cover.png".into(),
            url: "https://cdn.example.com/cover.png".into(),
            key: "images/cover.png".into(),
            markdown: "![cover](https://cdn.example.com/cover.png)".into(),
            mime: "image/png".into(),
            size: 1024,
            width: Some(800),
            height: Some(600),
            uploader: "local".into(),
        }
    }

    #[test]
    fn record_and_list() {
        let store = HistoryStore::open_memory().unwrap();
        let id = store.record(&fake_output(), Some("cover.png")).unwrap();
        let all = store.list(ListFilter::default()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].uploader, "local");
    }

    #[test]
    fn filter_by_uploader() {
        let store = HistoryStore::open_memory().unwrap();
        store.record(&fake_output(), None).unwrap();
        let mut o = fake_output();
        o.uploader = "github".into();
        store.record(&o, None).unwrap();
        let gh = store
            .list(ListFilter {
                uploader: Some("github".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(gh.len(), 1);
        assert_eq!(gh[0].uploader, "github");
    }

    #[test]
    fn delete_removes_entry() {
        let store = HistoryStore::open_memory().unwrap();
        let id = store.record(&fake_output(), None).unwrap();
        assert!(store.delete(&id).unwrap());
        assert!(store.list(ListFilter::default()).unwrap().is_empty());
    }

    #[test]
    fn uuid_is_well_formed() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }
}
