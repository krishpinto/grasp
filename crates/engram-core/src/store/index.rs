//! Writes and queries against SQLite: insert chunks, BM25 keyword search,
//! project registry, and processed-file tracking.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{Chunk, SearchHit};

/// Insert a chunk. Returns `true` if newly inserted, `false` if its hash
/// already existed (dedup). `md_path` is the markdown file the chunk lives in.
pub fn insert_chunk(conn: &Connection, chunk: &Chunk, md_path: &str) -> Result<bool> {
    let changed = conn.execute(
        "INSERT OR IGNORE INTO chunks
            (project, session_id, hash, text, timestamp, md_path, chunk_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            chunk.project,
            chunk.session_id,
            chunk.hash,
            chunk.text,
            chunk.timestamp,
            md_path,
            chunk.chunk_type.as_str(),
        ],
    )?;
    Ok(changed > 0)
}

/// Keyword (BM25) search over the FTS index. `project` optionally filters.
pub fn search(
    conn: &Connection,
    query: &str,
    project: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let match_expr = build_match_expr(query);
    if match_expr.is_empty() {
        return Ok(Vec::new());
    }

    // bm25() returns a score where *lower is more relevant*.
    let base = "SELECT c.id, c.project, c.session_id, c.text, c.timestamp, \
                       c.chunk_type, c.md_path, bm25(chunks_fts) AS score \
                FROM chunks_fts \
                JOIN chunks c ON c.id = chunks_fts.rowid \
                WHERE chunks_fts MATCH ?1";

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<SearchHit> {
        Ok(SearchHit {
            id: row.get(0)?,
            project: row.get(1)?,
            session_id: row.get(2)?,
            text: row.get(3)?,
            timestamp: row.get(4)?,
            chunk_type: row.get(5)?,
            md_path: row.get(6)?,
            score: row.get(7)?,
        })
    };

    let hits = match project {
        Some(p) => {
            let sql = format!("{base} AND c.project = ?2 ORDER BY score LIMIT ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![match_expr, p, limit as i64], map_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        }
        None => {
            let sql = format!("{base} ORDER BY score LIMIT ?2");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![match_expr, limit as i64], map_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        }
    };
    Ok(hits)
}

/// Turn a free-text query into a safe FTS5 MATCH expression: each whitespace
/// token becomes a quoted term, AND-ed together. Quoting neutralizes FTS
/// operator characters so user input can't break the query.
fn build_match_expr(query: &str) -> String {
    query
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Record/refresh a project in the registry and refresh its chunk count.
pub fn upsert_project(conn: &Connection, slug: &str, path: &str, now: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO projects (slug, path, last_seen, chunk_count)
         VALUES (?1, ?2, ?3, 0)
         ON CONFLICT(slug) DO UPDATE SET path = excluded.path, last_seen = excluded.last_seen",
        params![slug, path, now],
    )?;
    conn.execute(
        "UPDATE projects SET chunk_count =
            (SELECT COUNT(*) FROM chunks WHERE project = ?1) WHERE slug = ?1",
        params![slug],
    )?;
    Ok(())
}

/// A project row for `list_projects`.
#[derive(Debug, Clone)]
pub struct ProjectRow {
    pub slug: String,
    pub path: String,
    pub last_seen: String,
    pub chunk_count: i64,
}

pub fn list_projects(conn: &Connection) -> Result<Vec<ProjectRow>> {
    let mut stmt = conn.prepare(
        "SELECT slug, path, last_seen, chunk_count FROM projects ORDER BY last_seen DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ProjectRow {
            slug: row.get(0)?,
            path: row.get(1)?,
            last_seen: row.get(2)?,
            chunk_count: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Has this file already been processed at this content hash? (skip re-import)
pub fn file_already_processed(conn: &Connection, file_path: &str, hash: &str) -> Result<bool> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT last_hash FROM processed_files WHERE file_path = ?1",
            params![file_path],
            |row| row.get(0),
        )
        .optional()?;
    Ok(existing.as_deref() == Some(hash))
}

pub fn mark_file_processed(
    conn: &Connection,
    file_path: &str,
    hash: &str,
    now: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO processed_files (file_path, last_hash, processed_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(file_path) DO UPDATE SET last_hash = excluded.last_hash,
                                              processed_at = excluded.processed_at",
        params![file_path, hash, now],
    )?;
    Ok(())
}

/// Aggregate stats for the `stats` command.
#[derive(Debug, Clone)]
pub struct Stats {
    pub total_chunks: i64,
    pub total_projects: i64,
}

pub fn stats(conn: &Connection) -> Result<Stats> {
    let total_chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    let total_projects: i64 =
        conn.query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))?;
    Ok(Stats {
        total_chunks,
        total_projects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chunk, ChunkType};
    use crate::store::db;

    fn chunk(text: &str, hash: &str) -> Chunk {
        Chunk {
            project: "proj".into(),
            session_id: "s".into(),
            hash: hash.into(),
            text: text.into(),
            timestamp: "2026-01-01T10:00:00Z".into(),
            chunk_type: ChunkType::Decision,
        }
    }

    #[test]
    fn insert_dedups_by_hash() {
        let conn = db::open_in_memory().unwrap();
        assert!(insert_chunk(&conn, &chunk("hello world", "h1"), "x.md").unwrap());
        assert!(!insert_chunk(&conn, &chunk("hello world", "h1"), "x.md").unwrap());
    }

    #[test]
    fn search_finds_inserted_chunk() {
        let conn = db::open_in_memory().unwrap();
        insert_chunk(
            &conn,
            &chunk("switched to GKE for the operator", "h1"),
            "x.md",
        )
        .unwrap();
        let hits = search(&conn, "GKE operator", None, 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].text.contains("GKE"));
    }

    #[test]
    fn search_query_with_special_chars_does_not_error() {
        let conn = db::open_in_memory().unwrap();
        insert_chunk(&conn, &chunk("path/to/file.rs changed", "h1"), "x.md").unwrap();
        // Characters like '/' and '.' are FTS operators; must not break.
        let hits = search(&conn, "path/to/file.rs", None, 5).unwrap();
        assert!(!hits.is_empty());
    }
}
