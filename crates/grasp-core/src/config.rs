//! Cross-platform path resolution.
//!
//! CLAUDE.md hardcodes Linux paths (`~/.local/share/grasp`); we use the
//! `directories` crate so the same code works on Windows/mac/Linux.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};

/// Resolved locations Grasp reads from / writes to.
#[derive(Debug, Clone)]
pub struct Config {
    /// `~/.claude/projects/` — where Claude Code writes transcripts.
    pub claude_projects_dir: PathBuf,
    /// Grasp's own data dir (db + markdown live under here).
    pub data_dir: PathBuf,
}

impl Config {
    /// Build a config from the current user's standard directories.
    pub fn discover() -> Result<Self> {
        let base = BaseDirs::new().context("could not resolve home directory")?;
        let claude_projects_dir = base.home_dir().join(".claude").join("projects");

        let data_dir = ProjectDirs::from("dev", "grasp", "grasp")
            .map(|p| p.data_dir().to_path_buf())
            // Fallback: ~/.grasp if platform dirs are unavailable.
            .unwrap_or_else(|| base.home_dir().join(".grasp"));

        // One-time migration from the project's former name ("engram"): if we
        // have no data yet but a previous install does, adopt it so existing
        // memories carry across the rename. Best-effort — failures are ignored.
        if !data_dir.exists() {
            migrate_from_engram(&base, &data_dir);
        }

        Ok(Self {
            claude_projects_dir,
            data_dir,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("grasp.db")
    }

    pub fn memory_dir(&self) -> PathBuf {
        self.data_dir.join("memory")
    }

    pub fn memory_project_dir(&self, slug: &str) -> PathBuf {
        self.memory_dir().join(slug)
    }

    /// Ensure data + memory directories exist.
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.memory_dir())
            .with_context(|| format!("creating data dir {}", self.data_dir.display()))?;
        Ok(())
    }
}

/// Adopt a previous "engram"-named install's data, if present. Moves the old
/// data directory into place and renames the `engram.db` files to `grasp.db`.
/// Best-effort: any error leaves the new (empty) install untouched.
fn migrate_from_engram(base: &BaseDirs, new_data_dir: &Path) {
    let old_dir = ProjectDirs::from("dev", "engram", "engram")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| base.home_dir().join(".engram"));
    if !old_dir.exists() {
        return;
    }
    if let Some(parent) = new_data_dir.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Prefer an atomic move. If that fails because the old data is locked — e.g.
    // a former "engram" MCP server is still running and holds the db open, which
    // on Windows blocks moving the directory — fall back to a recursive copy so
    // the migration still works without asking the user to stop anything.
    let moved = std::fs::rename(&old_dir, new_data_dir).is_ok();
    if !moved && copy_dir_recursive(&old_dir, new_data_dir).is_err() {
        return; // give up; the new install starts empty (a re-import rebuilds it)
    }
    // Rename the database files (engram.db, -wal, -shm) to the new name.
    for suffix in ["", "-wal", "-shm"] {
        let from = new_data_dir.join(format!("engram.db{suffix}"));
        let to = new_data_dir.join(format!("grasp.db{suffix}"));
        if from.exists() {
            let _ = std::fs::rename(from, to);
        }
    }
}

/// Recursively copy `src` into `dst` (creating `dst`). Best-effort fallback for
/// the migration when an atomic rename isn't possible.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = match entry.path().strip_prefix(src) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// The project slug is the transcript directory name (e.g.
/// `c--projects-letterstack`), normalized so one project can't split across
/// casings.
pub fn slug_from_project_dir(dir: &Path) -> String {
    let raw = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());
    normalize_slug(&raw)
}

/// Normalize a project slug so the same project can't appear under two casings
/// (issue #8). Claude Code lowercases the Windows drive letter (`c--projects-x`),
/// but stray imports produced `C--projects-x`; lowercasing the leading drive
/// letter collapses those duplicates without disturbing the rest of the name.
pub fn normalize_slug(slug: &str) -> String {
    let mut chars: Vec<char> = slug.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = first.to_ascii_lowercase();
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_drive_letter_case() {
        assert_eq!(normalize_slug("C--projects-letterstack"), "c--projects-letterstack");
        assert_eq!(normalize_slug("c--projects-Engram"), "c--projects-Engram");
        assert_eq!(normalize_slug(""), "");
    }
}
