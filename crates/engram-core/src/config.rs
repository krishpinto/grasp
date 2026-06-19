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

/// The project slug is simply the transcript directory name
/// (e.g. `c--projects-letterstack`).
pub fn slug_from_project_dir(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string())
}
