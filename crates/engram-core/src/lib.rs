//! Engram engine — parses Claude Code transcripts, extracts signal, and stores
//! it to SQLite (+ FTS5) and human-readable markdown.
//!
//! This crate is UI-agnostic: the CLI, the Tauri app, and the MCP server all
//! drive it through the same functions.

pub mod config;
pub mod embed;
pub mod extractor;
pub mod import;
pub mod model;
pub mod parser;
pub mod redact;
pub mod store;
pub mod util;
pub mod watch;

pub use config::Config;
pub use import::{import_all, import_file, ImportReport};
pub use model::{Chunk, ChunkType, Entry, SearchHit};

use anyhow::Result;
use rusqlite::Connection;

/// Convenience handle bundling the resolved config with an open DB connection.
pub struct Engram {
    pub config: Config,
    pub conn: Connection,
    /// Lazily-loaded embedding model (only loaded when embeddings are used).
    embedder: std::cell::OnceCell<embed::Embedder>,
}

impl Engram {
    /// Open Engram using the current user's standard directories.
    pub fn open() -> Result<Self> {
        let config = Config::discover()?;
        let conn = store::db::open(&config)?;
        Ok(Self {
            config,
            conn,
            embedder: std::cell::OnceCell::new(),
        })
    }

    /// Import transcripts (defaults to `~/.claude/projects/`).
    pub fn import(&self, path: Option<&std::path::Path>) -> Result<ImportReport> {
        import_all(&self.conn, &self.config, path)
    }

    /// Load (downloading on first use) the embedding model, cached for reuse.
    fn embedder(&self) -> Result<&embed::Embedder> {
        if self.embedder.get().is_none() {
            let e = embed::Embedder::load()?;
            let _ = self.embedder.set(e);
        }
        Ok(self.embedder.get().expect("embedder just set"))
    }

    /// Backfill embeddings for every chunk that lacks one. Returns the count
    /// embedded. Triggers a one-time model download (~90MB) on first run.
    pub fn embed_backfill(&self) -> Result<usize> {
        let missing = store::vectors::chunks_without_embeddings(&self.conn)?;
        if missing.is_empty() {
            return Ok(0);
        }
        let embedder = self.embedder()?;
        let mut done = 0;
        for batch in missing.chunks(32) {
            let texts: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();
            let vecs = embedder.embed(&texts)?;
            for ((id, _), v) in batch.iter().zip(vecs) {
                store::vectors::insert_embedding(&self.conn, *id, &v)?;
                done += 1;
            }
        }
        Ok(done)
    }

    /// Search memory. Uses hybrid BM25 + semantic (RRF) when embeddings exist,
    /// otherwise falls back to keyword-only BM25.
    pub fn search(
        &self,
        query: &str,
        project: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        if store::vectors::embedding_count(&self.conn)? == 0 {
            return store::index::search(&self.conn, query, project, limit);
        }
        self.hybrid_search(query, project, limit)
    }

    /// BM25 + cosine fused with Reciprocal Rank Fusion (k=60).
    fn hybrid_search(
        &self,
        query: &str,
        project: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        const K: f64 = 60.0;
        let candidates = (limit * 5).max(50);

        // Keyword ranking.
        let bm25 = store::index::search(&self.conn, query, project, candidates)?;
        let bm25_ids: Vec<i64> = bm25.iter().map(|h| h.id).collect();

        // Semantic ranking (brute-force cosine over stored vectors).
        let qvec = self.embedder()?.embed_one(query)?;
        let mut sims: Vec<(i64, f32)> = store::vectors::load_embeddings(&self.conn, project)?
            .into_iter()
            .map(|(id, v)| (id, embed::cosine(&qvec, &v)))
            .collect();
        sims.sort_by(|a, b| b.1.total_cmp(&a.1));
        let sem_ids: Vec<i64> = sims.into_iter().take(candidates).map(|(id, _)| id).collect();

        // RRF fuse.
        let mut scores: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
        for (rank, id) in bm25_ids.iter().enumerate() {
            *scores.entry(*id).or_default() += 1.0 / (K + (rank + 1) as f64);
        }
        for (rank, id) in sem_ids.iter().enumerate() {
            *scores.entry(*id).or_default() += 1.0 / (K + (rank + 1) as f64);
        }
        let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

        let mut hits = Vec::new();
        for (id, score) in ranked.into_iter().take(limit) {
            if let Some(mut h) = store::index::chunk_as_hit(&self.conn, id)? {
                h.score = score;
                hits.push(h);
            }
        }
        Ok(hits)
    }

    pub fn projects(&self) -> Result<Vec<store::index::ProjectRow>> {
        store::index::list_projects(&self.conn)
    }

    pub fn stats(&self) -> Result<store::index::Stats> {
        store::index::stats(&self.conn)
    }

    /// Build the node/edge graph for the visual view.
    pub fn graph(&self, project: Option<&str>) -> Result<store::graph::Graph> {
        store::graph::build_graph(&self.conn, project)
    }

    /// Forget all memories for one project (DB rows, FTS via triggers, the
    /// processed-file records so it can be re-imported, and its markdown).
    /// Returns the number of memories removed.
    pub fn forget(&self, project: &str) -> Result<usize> {
        let removed = self
            .conn
            .execute("DELETE FROM chunks WHERE project = ?1", [project])?;
        self.conn
            .execute("DELETE FROM projects WHERE slug = ?1", [project])?;
        self.conn.execute(
            "DELETE FROM processed_files WHERE file_path LIKE ?1",
            [format!("%{project}%")],
        )?;
        let dir = self.config.memory_project_dir(project);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).ok();
        }
        Ok(removed)
    }

    /// Wipe all memory (every project). Schema is preserved.
    pub fn reset(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM chunks; DELETE FROM projects; DELETE FROM processed_files;",
        )?;
        let mem = self.config.memory_dir();
        if mem.exists() {
            std::fs::remove_dir_all(&mem).ok();
        }
        std::fs::create_dir_all(self.config.memory_dir()).ok();
        Ok(())
    }

    /// Fetch one memory by id (for the note viewer).
    pub fn get_memory(&self, id: i64) -> Result<Option<store::index::MemoryDetail>> {
        store::index::get_chunk(&self.conn, id)
    }

    /// Manually save a note to memory (the MCP `save_context` tool). Returns
    /// true if stored, false if a duplicate. `type_` is one of
    /// decision/context/note (note maps to context).
    pub fn save_context(
        &self,
        text: &str,
        project: Option<&str>,
        type_: Option<&str>,
    ) -> Result<bool> {
        let chunk_type = match type_ {
            Some("decision") => ChunkType::Decision,
            Some("summary") => ChunkType::Summary,
            _ => ChunkType::Context, // "note" / "context" / unknown
        };
        let project = config::normalize_slug(project.unwrap_or("manual"));
        let timestamp = chrono::Utc::now().to_rfc3339();
        // Scrub secrets from manually-saved notes too (issue #1).
        let text = redact::scrub(text.trim());
        let hash = util::hash_text(&format!("{}|{}", chunk_type.as_str(), util::normalize(&text)));
        let chunk = Chunk {
            project: project.clone(),
            session_id: "manual".to_string(),
            hash,
            text: text.trim().to_string(),
            timestamp: timestamp.clone(),
            chunk_type,
        };
        let md_path = store::markdown::append_chunk(&self.config, &chunk)?;
        let added = store::index::insert_chunk(&self.conn, &chunk, &md_path.to_string_lossy())?;
        store::index::upsert_project(&self.conn, &project, &project, &timestamp)?;
        Ok(added)
    }

    /// Ingest a single transcript file (used by the live watcher). The project
    /// slug is derived from the file's parent directory. Returns chunks added.
    pub fn ingest_file(&self, file: &std::path::Path) -> Result<usize> {
        let slug = file
            .parent()
            .map(config::slug_from_project_dir)
            .unwrap_or_else(|| "unknown".to_string());
        let mut report = ImportReport::default();
        let added = import::import_file(&self.conn, &self.config, &slug, file, &mut report)?;
        if added.is_some() {
            // Keep the project registry / chunk counts current.
            let now = chrono::Utc::now().to_rfc3339();
            if let Some(dir) = file.parent() {
                store::index::upsert_project(&self.conn, &slug, &dir.to_string_lossy(), &now)?;
            }
        }
        Ok(added.unwrap_or(0))
    }
}
