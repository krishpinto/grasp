//! Retroactive import: walk transcript files, extract memories, persist them.
//!
//! This is the Stage-1 ingest path (`grasp import`). The live file watcher
//! (Stage 6) will reuse `import_file` for incremental updates.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

use crate::config::{slug_from_project_dir, Config};
use crate::model::Entry;
use crate::store::{index, markdown};
use crate::util::hash_text;
use crate::{extractor, parser};

/// Summary of an import run.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ImportReport {
    pub files_processed: usize,
    pub files_skipped: usize,
    pub chunks_added: usize,
}

/// Import every project under `root` (defaults to the configured Claude
/// projects directory). A "project" is one immediate subdirectory; its `*.jsonl`
/// files are the transcripts.
pub fn import_all(conn: &Connection, config: &Config, root: Option<&Path>) -> Result<ImportReport> {
    let root = root.unwrap_or(&config.claude_projects_dir);
    let mut report = ImportReport::default();

    if !root.exists() {
        anyhow::bail!("transcript path does not exist: {}", root.display());
    }

    // If `root` itself contains *.jsonl files, treat it as a single project dir.
    let project_dirs: Vec<std::path::PathBuf> = if has_jsonl(root)? {
        vec![root.to_path_buf()]
    } else {
        std::fs::read_dir(root)
            .with_context(|| format!("reading {}", root.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect()
    };

    // Wrap the entire import in a single transaction. Without this each chunk
    // insert auto-commits (an fsync apiece), making a full import of many
    // projects crawl to the point of looking hung.
    let tx = conn.unchecked_transaction()?;
    {
        let c: &Connection = &tx; // deref-coerces Transaction -> Connection
        for dir in project_dirs {
            import_project_dir(c, config, &dir, &mut report)?;
        }
    }
    tx.commit()?;
    Ok(report)
}

fn has_jsonl(dir: &Path) -> Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn import_project_dir(
    conn: &Connection,
    config: &Config,
    dir: &Path,
    report: &mut ImportReport,
) -> Result<()> {
    let slug = slug_from_project_dir(dir);
    let now = Utc::now().to_rfc3339();

    let mut entries = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    entries.sort();

    let mut touched = false;
    for file in entries {
        let added = import_file(conn, config, &slug, &file, report)?;
        touched = touched || added.is_some();
    }

    if touched {
        index::upsert_project(conn, &slug, &dir.to_string_lossy(), &now)?;
    }
    Ok(())
}

/// Import a single transcript file. Returns `Some(chunks_added)` if processed,
/// `None` if skipped (unchanged since last import). Public so the watcher reuses it.
pub fn import_file(
    conn: &Connection,
    config: &Config,
    slug: &str,
    file: &Path,
    report: &mut ImportReport,
) -> Result<Option<usize>> {
    let raw = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("skipping unreadable file {}: {e}", file.display());
            return Ok(None);
        }
    };

    let file_key = file.to_string_lossy().to_string();
    let content_hash = hash_text(&raw);

    if index::file_already_processed(conn, &file_key, &content_hash)? {
        report.files_skipped += 1;
        return Ok(None);
    }

    // Parse every line into normalized entries (unknown lines -> Entry::Other).
    let mut parsed: Vec<Entry> = Vec::new();
    for line in raw.lines() {
        if let Some(entry) = parser::parse_line(line) {
            parsed.push(entry);
        }
    }

    let default_ts = Utc::now().to_rfc3339();
    let chunks = extractor::extract_session(&parsed, slug, &default_ts);

    let mut added = 0usize;
    for chunk in &chunks {
        // Markdown is the source of truth; write it first, then index in SQLite.
        let md_path = markdown::append_chunk(config, chunk)?;
        if index::insert_chunk(conn, chunk, &md_path.to_string_lossy())? {
            added += 1;
        }
    }

    let now = Utc::now().to_rfc3339();
    index::mark_file_processed(conn, &file_key, &content_hash, &now)?;

    report.files_processed += 1;
    report.chunks_added += added;
    Ok(Some(added))
}
