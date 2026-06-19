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

    /// Fetch one memory by id (for the note viewer).
    pub fn get_memory(&self, id: i64) -> Result<Option<store::index::MemoryDetail>> {
        store::index::get_chunk(&self.conn, id)
    }
}
