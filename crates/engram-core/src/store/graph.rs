//! Build a node/edge graph of memories for the visual "brain" view.
//!
//! v1 edges are *structural*: chunks from the same session are chained in
//! chronological order, so the graph reads as "what happened, in order, per
//! session", clustered by project. Stage 4 will add *semantic* edges (links
//! between chunks that are about the same thing) once embeddings exist.

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

/// One memory, as a graph node.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: i64,
    /// Short label for display (first line, truncated).
    pub label: String,
    pub chunk_type: String,
    pub project: String,
    pub session_id: String,
    pub timestamp: String,
}

/// A link between two memories.
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    /// Why these are linked: "session" (chronological) — later: "semantic".
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Build the graph, optionally restricted to one project.
pub fn build_graph(conn: &Connection, project: Option<&str>) -> Result<Graph> {
    let base = "SELECT id, text, chunk_type, project, session_id, timestamp \
                FROM chunks";
    let order = " ORDER BY session_id, timestamp, id";

    let mut nodes: Vec<GraphNode> = Vec::new();
    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<GraphNode> {
        let text: String = row.get(1)?;
        Ok(GraphNode {
            id: row.get(0)?,
            label: short_label(&text),
            chunk_type: row.get(2)?,
            project: row.get(3)?,
            session_id: row.get(4)?,
            timestamp: row.get(5)?,
        })
    };

    match project {
        Some(p) => {
            let sql = format!("{base} WHERE project = ?1{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([p], map_row)?;
            for n in rows {
                nodes.push(n?);
            }
        }
        None => {
            let sql = format!("{base}{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], map_row)?;
            for n in rows {
                nodes.push(n?);
            }
        }
    }

    // Chain consecutive nodes within the same session (rows are already ordered
    // by session, then time).
    let mut edges = Vec::new();
    for pair in nodes.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.session_id == b.session_id {
            edges.push(GraphEdge {
                source: a.id,
                target: b.id,
                kind: "session".to_string(),
            });
        }
    }

    Ok(Graph { nodes, edges })
}

/// First line of the text, trimmed and truncated for a compact node label.
fn short_label(text: &str) -> String {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    crate::util::truncate(first.trim(), 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Chunk, ChunkType};
    use crate::store::{db, index};

    fn chunk(hash: &str, session: &str, ts: &str) -> Chunk {
        Chunk {
            project: "proj".into(),
            session_id: session.into(),
            hash: hash.into(),
            text: format!("memory {hash}\nsecond line"),
            timestamp: ts.into(),
            chunk_type: ChunkType::Decision,
        }
    }

    #[test]
    fn chains_within_session_only() {
        let conn = db::open_in_memory().unwrap();
        // session A: two chunks (one edge). session B: one chunk (no edge).
        index::insert_chunk(&conn, &chunk("a1", "A", "2026-01-01T10:00:00Z"), "x.md").unwrap();
        index::insert_chunk(&conn, &chunk("a2", "A", "2026-01-01T10:05:00Z"), "x.md").unwrap();
        index::insert_chunk(&conn, &chunk("b1", "B", "2026-01-01T11:00:00Z"), "x.md").unwrap();

        let g = build_graph(&conn, None).unwrap();
        assert_eq!(g.nodes.len(), 3);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].kind, "session");
        // label is the first non-empty line, truncated.
        assert!(g.nodes[0].label.starts_with("memory"));
    }
}
