//! Cross-platform path resolution.
//!
//! CLAUDE.md hardcodes Linux paths (`~/.local/share/engram`); we use the
//! `directories` crate so the same code works on Windows/mac/Linux.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};

/// Resolved locations Engram reads from / writes to.
#[derive(Debug, Clone)]
pub struct Config {
    /// `~/.claude/projects/` — where Claude Code writes transcripts.
    pub claude_projects_dir: PathBuf,
    /// Engram's own data dir (db + markdown live under here).
    pub data_dir: PathBuf,
}

impl Config {
    /// Build a config from the current user's standard directories.
    pub fn discover() -> Result<Self> {
        let base = BaseDirs::new().context("could not resolve home directory")?;
        let claude_projects_dir = base.home_dir().join(".claude").join("projects");

        let data_dir = ProjectDirs::from("dev", "engram", "engram")
            .map(|p| p.data_dir().to_path_buf())
            // Fallback: ~/.engram if platform dirs are unavailable.
            .unwrap_or_else(|| base.home_dir().join(".engram"));

        Ok(Self {
            claude_projects_dir,
            data_dir,
        })
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("engram.db")
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
