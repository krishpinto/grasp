//! SQLite connection + schema migrations.
//!
//! Phase-1 schema: `chunks` (the memories), `chunks_fts` (FTS5 keyword index
//! kept in sync via triggers), `projects` (registry), and `processed_files`
//! (so re-imports skip unchanged transcripts). The `bundled` rusqlite feature
//! ships a SQLite with FTS5 compiled in.

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::config::Config;

/// Open the Grasp database (creating it + its parent dir if needed) and
/// run idempotent migrations.
pub fn open(config: &Config) -> Result<Connection> {
    config.ensure_dirs()?;
    let path = config.db_path();
    let conn = Connection::open(&path)
        .with_context(|| format!("opening database at {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "foreign_keys", "ON").ok();
    // The watcher daemon and the MCP server are separate writers against one DB;
    // wait on the write lock instead of failing with SQLITE_BUSY (issue #17).
    conn.pragma_update(None, "busy_timeout", 5000).ok();
    migrate(&conn)?;
    Ok(conn)
}

/// Open an in-memory database (used by tests).
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

/// Ordered schema migrations. Append new ones; never edit a shipped entry.
/// The DB's `user_version` pragma records how many have been applied, so an
/// existing database only runs the new ones.
const MIGRATIONS: &[&str] = &[
    // v1 — initial schema.
    r#"
        CREATE TABLE IF NOT EXISTS chunks (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            project     TEXT NOT NULL,
            session_id  TEXT NOT NULL,
            hash        TEXT UNIQUE NOT NULL,
            text        TEXT NOT NULL,
            timestamp   TEXT NOT NULL,
            md_path     TEXT NOT NULL,
            chunk_type  TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_project ON chunks(project);

        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            text,
            project UNINDEXED,
            content='chunks',
            content_rowid='id'
        );

        -- Keep the FTS index in sync with the chunks table.
        CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
            INSERT INTO chunks_fts(rowid, text, project)
            VALUES (new.id, new.text, new.project);
        END;
        CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, text, project)
            VALUES ('delete', old.id, old.text, old.project);
        END;
        CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
            INSERT INTO chunks_fts(chunks_fts, rowid, text, project)
            VALUES ('delete', old.id, old.text, old.project);
            INSERT INTO chunks_fts(rowid, text, project)
            VALUES (new.id, new.text, new.project);
        END;

        CREATE TABLE IF NOT EXISTS projects (
            slug        TEXT PRIMARY KEY,
            path        TEXT NOT NULL,
            last_seen   TEXT NOT NULL,
            chunk_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS processed_files (
            file_path    TEXT PRIMARY KEY,
            last_hash    TEXT NOT NULL,
            processed_at TEXT NOT NULL
        );

        -- One embedding per chunk (f32 vector stored as little-endian bytes).
        CREATE TABLE IF NOT EXISTS embeddings (
            chunk_id INTEGER PRIMARY KEY,
            dim      INTEGER NOT NULL,
            vec      BLOB NOT NULL
        );
        "#,
];

/// Apply any migrations the database hasn't seen yet, tracked via `user_version`.
fn migrate(conn: &Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if current < version {
            conn.execute_batch(sql)
                .with_context(|| format!("applying migration v{version}"))?;
            conn.pragma_update(None, "user_version", version)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_set_user_version_and_are_idempotent() {
        let conn = open_in_memory().unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v as usize, MIGRATIONS.len());
        // Re-running must be a no-op and not error.
        migrate(&conn).unwrap();
        let v2: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v2 as usize, MIGRATIONS.len());
    }
}
