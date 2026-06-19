//! Engram engine — parses Claude Code transcripts, extracts signal, and stores
//! it to SQLite (+ FTS5) and human-readable markdown.
//!
//! This crate is UI-agnostic: the CLI, the Tauri app, and the MCP server all
//! drive it through the same functions.

pub mod config;
pub mod extractor;
pub mod import;
pub mod model;
pub mod parser;
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
}

impl Engram {
    /// Open Engram using the current user's standard directories.
    pub fn open() -> Result<Self> {
        let config = Config::discover()?;
        let conn = store::db::open(&config)?;
        Ok(Self { config, conn })
    }

    /// Import transcripts (defaults to `~/.claude/projects/`).
    pub fn import(&self, path: Option<&std::path::Path>) -> Result<ImportReport> {
        import_all(&self.conn, &self.config, path)
    }

    /// Keyword (BM25) search.
    pub fn search(
        &self,
        query: &str,
        project: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        store::index::search(&self.conn, query, project, limit)
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
        let project = project.unwrap_or("manual").to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let hash = util::hash_text(&format!("{}|{}", chunk_type.as_str(), util::normalize(text)));
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
