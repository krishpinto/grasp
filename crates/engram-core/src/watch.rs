//! Live file watching — the core of Engram's passive capture.
//!
//! Watches `~/.claude/projects/` and surfaces changed `*.jsonl` transcripts so
//! the consumer (the app or `engram watch`) can ingest them incrementally. The
//! agent never participates: if Claude Code writes a transcript, the watcher
//! sees it. Events are debounced so a burst of writes to one file collapses
//! into a single ingest.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Result;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};

/// A running watcher. Drop it to stop watching.
pub struct Watcher {
    // Kept alive so the OS watch stays registered; never read directly.
    _debouncer: Debouncer<RecommendedWatcher>,
    /// Debounced paths of changed `*.jsonl` transcripts.
    pub changes: Receiver<PathBuf>,
}

/// Start watching `dir` recursively. Quiet for `debounce` after the last write
/// before a path is emitted.
pub fn watch(dir: &Path, debounce: Duration) -> Result<Watcher> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut debouncer = new_debouncer(debounce, move |res: DebounceEventResult| {
        if let Ok(events) = res {
            for ev in events {
                if is_jsonl(&ev.path) {
                    // Receiver gone => consumer stopped; ignore send errors.
                    let _ = tx.send(ev.path);
                }
            }
        }
    })?;

    debouncer
        .watcher()
        .watch(dir, RecursiveMode::Recursive)?;

    Ok(Watcher {
        _debouncer: debouncer,
        changes: rx,
    })
}

fn is_jsonl(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("jsonl")
}
