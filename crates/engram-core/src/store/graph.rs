//! Build a node/edge graph of memories for the visual "brain" view.
//!
//! Node kinds:
//!   - "memory": a chunk (decision/file_write/error/…), id = chunk id (positive).
//!   - "file":   a file touched by file_write memories, id = negative synthetic.
//!
//! Edge kinds:
//!   - "session":  chunks chained chronologically within a session (structure).
//!   - "file":     a file_write memory → the file node it touched (structure).
//!   - "semantic": two memories whose embeddings are similar (meaning) — links
//!                 related decisions across sessions, turning the "strip" into a web.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

use crate::embed::cosine;
use crate::store::vectors;

/// How similar two memories must be (cosine) to draw a semantic edge.
const SEMANTIC_THRESHOLD: f32 = 0.55;
/// Max semantic neighbours kept per memory (keeps the graph a web, not a hairball).
const SEMANTIC_TOP_K: usize = 3;

/// A graph node — either a memory or a file.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: i64,
    /// "memory" or "file".
    pub node_type: String,
    /// Short label for display (first line / file name, truncated).
    pub label: String,
    pub chunk_type: String,
    pub project: String,
    pub session_id: String,
    pub timestamp: String,
}

/// A link between two nodes.
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    /// Why these are linked: "session", "file", or "semantic".
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

    // Collect memory rows (keep raw text so we can mine file paths for file edges).
    struct Row {
        node: GraphNode,
        text: String,
    }
    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<Row> {
        let text: String = row.get(1)?;
        Ok(Row {
            node: GraphNode {
                id: row.get(0)?,
                node_type: "memory".to_string(),
                label: short_label(&text),
                chunk_type: row.get(2)?,
                project: row.get(3)?,
                session_id: row.get(4)?,
                timestamp: row.get(5)?,
            },
            text,
        })
    };

    let rows: Vec<Row> = match project {
        Some(p) => {
            let sql = format!("{base} WHERE project = ?1{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([p], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        None => {
            let sql = format!("{base}{order}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], map_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
    };

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Session chains: consecutive memories within one session (rows are ordered).
    for pair in rows.windows(2) {
        let (a, b) = (&pair[0].node, &pair[1].node);
        if a.session_id == b.session_id {
            edges.push(GraphEdge {
                source: a.id,
                target: b.id,
                kind: "session".to_string(),
            });
        }
    }

    // File nodes + edges: link each file_write memory to the file it touched, so
    // memories cluster around the files they're about.
    let mut file_ids: HashMap<String, i64> = HashMap::new();
    let mut next_file_id: i64 = -1;
    let mut file_nodes: Vec<GraphNode> = Vec::new();
    for r in &rows {
        if r.node.chunk_type != "file_write" {
            continue;
        }
        let Some(path) = file_path_in(&r.text) else {
            continue;
        };
        let fid = *file_ids.entry(path.clone()).or_insert_with(|| {
            let id = next_file_id;
            next_file_id -= 1;
            file_nodes.push(GraphNode {
                id,
                node_type: "file".to_string(),
                label: file_label(&path),
                chunk_type: "file".to_string(),
                project: r.node.project.clone(),
                session_id: String::new(),
                timestamp: r.node.timestamp.clone(),
            });
            id
        });
        edges.push(GraphEdge {
            source: r.node.id,
            target: fid,
            kind: "file".to_string(),
        });
    }

    // Semantic edges: connect memories whose embeddings are similar, even across
    // sessions. Keep each node's top-K neighbours above the threshold.
    let embeddings = vectors::load_embeddings(conn, project)?;
    add_semantic_edges(&embeddings, &mut edges);

    nodes.extend(rows.into_iter().map(|r| r.node));
    nodes.extend(file_nodes);

    Ok(Graph { nodes, edges })
}

/// Add up to `SEMANTIC_TOP_K` similarity edges per memory (undirected, deduped).
fn add_semantic_edges(embeddings: &[(i64, Vec<f32>)], edges: &mut Vec<GraphEdge>) {
    let mut seen: HashSet<(i64, i64)> = HashSet::new();
    for (i, (id_a, vec_a)) in embeddings.iter().enumerate() {
        // Score this node against all others.
        let mut sims: Vec<(i64, f32)> = Vec::new();
        for (j, (id_b, vec_b)) in embeddings.iter().enumerate() {
            if i == j {
                continue;
            }
            let sim = cosine(vec_a, vec_b);
            if sim >= SEMANTIC_THRESHOLD {
                sims.push((*id_b, sim));
            }
        }
        sims.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (id_b, _) in sims.into_iter().take(SEMANTIC_TOP_K) {
            let key = if *id_a < id_b {
                (*id_a, id_b)
            } else {
                (id_b, *id_a)
            };
            if seen.insert(key) {
                edges.push(GraphEdge {
                    source: key.0,
                    target: key.1,
                    kind: "semantic".to_string(),
                });
            }
        }
    }
}

/// Extract the first backticked file path from a file_write memory body
/// (format: "Write `path` — note").
fn file_path_in(text: &str) -> Option<String> {
    let start = text.find('`')? + 1;
    let rest = &text[start..];
    let end = rest.find('`')?;
    let path = rest[..end].trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

/// Display label for a file node: the file name (last path segment).
fn file_label(path: &str) -> String {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path);
    crate::util::truncate(name, 40)
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

    #[test]
    fn file_write_creates_file_node_and_edge() {
        let conn = db::open_in_memory().unwrap();
        let mut c = chunk("f1", "A", "2026-01-01T10:00:00Z");
        c.chunk_type = ChunkType::FileWrite;
        c.text = "Write `src/main.rs` — set up the entry point".into();
        index::insert_chunk(&conn, &c, "x.md").unwrap();

        let g = build_graph(&conn, None).unwrap();
        assert!(g
            .nodes
            .iter()
            .any(|n| n.node_type == "file" && n.label == "main.rs"));
        assert!(g.edges.iter().any(|e| e.kind == "file"));
    }
}
