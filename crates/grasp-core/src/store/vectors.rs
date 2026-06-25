//! Embedding storage: f32 vectors as BLOBs, plus brute-force cosine search.
//!
//! At Grasp's scale (thousands of chunks) a linear scan in Rust is plenty
//! fast and avoids a native vector-search extension (sqlite-vec), which would
//! reintroduce the toolchain problems we sidestepped.

use anyhow::Result;
use rusqlite::{params, Connection};

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn insert_embedding(conn: &Connection, chunk_id: i64, vec: &[f32]) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO embeddings (chunk_id, dim, vec) VALUES (?1, ?2, ?3)",
        params![chunk_id, vec.len() as i64, vec_to_blob(vec)],
    )?;
    Ok(())
}

/// Chunks that don't yet have an embedding (id, text) — for backfill.
pub fn chunks_without_embeddings(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.text FROM chunks c
         LEFT JOIN embeddings e ON e.chunk_id = c.id
         WHERE e.chunk_id IS NULL",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn embedding_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))?)
}

/// Load all stored embeddings, optionally restricted to one project.
pub fn load_embeddings(
    conn: &Connection,
    project: Option<&str>,
) -> Result<Vec<(i64, Vec<f32>)>> {
    let map = |r: &rusqlite::Row| -> rusqlite::Result<(i64, Vec<u8>)> {
        Ok((r.get(0)?, r.get(1)?))
    };
    let rows: Vec<(i64, Vec<u8>)> = match project {
        Some(p) => {
            let mut stmt = conn.prepare(
                "SELECT e.chunk_id, e.vec FROM embeddings e
                 JOIN chunks c ON c.id = e.chunk_id WHERE c.project = ?1",
            )?;
            let rows = stmt.query_map([p], map)?.collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        None => {
            let mut stmt = conn.prepare("SELECT chunk_id, vec FROM embeddings")?;
            let rows = stmt.query_map([], map)?.collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
    };
    Ok(rows.into_iter().map(|(id, b)| (id, blob_to_vec(&b))).collect())
}
