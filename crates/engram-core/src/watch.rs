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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_jsonl_is_watched() {
        assert!(is_jsonl(Path::new("a/b/session.jsonl")));
        assert!(!is_jsonl(Path::new("a/b/notes.txt")));
        assert!(!is_jsonl(Path::new("a/b/Makefile")));
    }

    /// Real watch: a .jsonl write surfaces; a non-jsonl write is filtered out.
    /// Ignored by default (file-watch timing is environment-sensitive); run with:
    ///   cargo test -p engram-core watch_surfaces_jsonl_writes -- --ignored
    #[test]
    #[ignore]
    fn watch_surfaces_jsonl_writes() {
        let dir = std::env::temp_dir().join(format!("engram_watch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = watch(&dir, Duration::from_millis(150)).unwrap();

        std::fs::write(dir.join("ignore_me.txt"), "x").unwrap();
        std::fs::write(dir.join("session.jsonl"), "{}").unwrap();

        let mut got: Vec<PathBuf> = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match w.changes.recv_timeout(Duration::from_millis(300)) {
                Ok(p) => {
                    got.push(p);
                    if got.iter().any(|p| p.file_name().unwrap() == "session.jsonl") {
                        break;
                    }
                }
                Err(_) => {}
            }
        }
        std::fs::remove_dir_all(&dir).ok();

        assert!(
            got.iter().any(|p| p.file_name().unwrap() == "session.jsonl"),
            "the .jsonl write should surface"
        );
        assert!(
            got.iter().all(|p| is_jsonl(p)),
            "non-jsonl writes must be filtered out"
        );
    }
}
