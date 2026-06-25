//! Grasp CLI — headless driver for the engine (Stage 1).
//!
//! Subcommands:
//!   grasp import [--path DIR]   ingest transcripts (default: ~/.claude/projects)
//!   grasp search <QUERY> [--project SLUG] [--limit N]
//!   grasp projects              list indexed projects + chunk counts
//!   grasp stats                 totals

mod eval;
mod mcp;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use grasp_core::Grasp;

#[derive(Parser)]
#[command(name = "grasp", version, about = "Local, passive memory for Claude Code")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import transcripts into memory.
    Import {
        /// Directory to import (defaults to ~/.claude/projects).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Search memory by keyword.
    Search {
        /// Words to search for.
        query: Vec<String>,
        /// Restrict to one project slug.
        #[arg(long)]
        project: Option<String>,
        /// Max results.
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// List indexed projects.
    Projects,
    /// Show aggregate stats.
    Stats,
    /// Print the memory graph (nodes + edges) as JSON.
    Graph {
        /// Restrict to one project slug.
        #[arg(long)]
        project: Option<String>,
    },
    /// Watch transcripts and ingest changes live (passive capture).
    Watch {
        /// Directory to watch (defaults to ~/.claude/projects).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Run the MCP server over stdio (for Claude Code).
    Mcp,
    /// Generate embeddings for memories that lack them (enables semantic search).
    Embed,
    /// Re-scrub all stored memories + markdown with the current secret patterns.
    Redact,
    /// Register Grasp with Claude Code (auto-configures the MCP server).
    Setup,
    /// Run the retrieval eval set (BM25-only vs hybrid hit-rate).
    Eval {
        /// JSON file of eval cases (defaults to eval/queries.json).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Top-K cutoff for counting a hit.
        #[arg(long, default_value_t = 5)]
        k: usize,
    },
    /// Forget all memories for one project.
    Forget {
        /// Project slug to forget.
        #[arg(long)]
        project: String,
    },
    /// Wipe all memory (every project).
    Reset {
        /// Confirm the wipe (required).
        #[arg(long)]
        yes: bool,
    },
}

/// Register Grasp as an MCP server via Claude Code's CLI. Tries direct exec,
/// then through the platform shell (the `claude` launcher is a `.cmd` shim on
/// Windows and needs a shell to resolve). Returns true on success.
fn register_mcp(exe: &str) -> bool {
    use std::process::Command;
    let args = [
        "mcp", "add", "grasp", "-s", "user", "--", exe, "mcp",
    ];
    if let Ok(s) = Command::new("claude").args(args).status() {
        if s.success() {
            return true;
        }
    }
    let joined = format!("claude mcp add grasp -s user -- \"{exe}\" mcp");
    let shell = if cfg!(windows) {
        Command::new("cmd").args(["/C", &joined]).status()
    } else {
        Command::new("sh").arg("-c").arg(&joined).status()
    };
    matches!(shell, Ok(s) if s.success())
}

fn main() -> Result<()> {
    // Logs go to stderr: the MCP server uses stdout for the JSON-RPC stream.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let grasp = Grasp::open()?;

    match cli.command {
        Command::Import { path } => {
            let report = grasp.import(path.as_deref())?;
            println!(
                "Imported {} file(s), skipped {} unchanged, added {} new memories.",
                report.files_processed, report.files_skipped, report.chunks_added
            );
        }
        Command::Search {
            query,
            project,
            limit,
        } => {
            let q = query.join(" ");
            if q.trim().is_empty() {
                eprintln!("Provide something to search for, e.g. grasp search auth bug");
                std::process::exit(2);
            }
            let hits = grasp.search(&q, project.as_deref(), limit)?;
            if hits.is_empty() {
                println!("No matches for {q:?}.");
            }
            for hit in hits {
                let when = hit.timestamp.split('T').next().unwrap_or(&hit.timestamp);
                println!(
                    "\n[{}] {} · {}\n{}",
                    when, hit.chunk_type, hit.project, hit.text
                );
            }
        }
        Command::Projects => {
            let projects = grasp.projects()?;
            if projects.is_empty() {
                println!("No projects indexed yet. Run `grasp import` first.");
            }
            for p in projects {
                println!("{:<40} {:>6} memories", p.slug, p.chunk_count);
            }
        }
        Command::Stats => {
            let stats = grasp.stats()?;
            println!("Projects: {}", stats.total_projects);
            println!("Memories: {}", stats.total_chunks);
            println!("Database: {}", grasp.config.db_path().display());
        }
        Command::Graph { project } => {
            let graph = grasp.graph(project.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&graph)?);
        }
        Command::Watch { path } => {
            let dir = path.unwrap_or_else(|| grasp.config.claude_projects_dir.clone());
            // Capture anything already on disk, then watch for changes.
            let report = grasp.import(Some(&dir))?;
            println!(
                "Initial import: {} file(s), +{} memories. Now watching {} … (Ctrl+C to stop)",
                report.files_processed,
                report.chunks_added,
                dir.display()
            );
            let watcher =
                grasp_core::watch::watch(&dir, std::time::Duration::from_millis(800))?;
            for changed in watcher.changes {
                match grasp.ingest_file(&changed) {
                    Ok(0) => {}
                    Ok(n) => println!("+{n} memories from {}", changed.display()),
                    Err(e) => eprintln!("ingest error for {}: {e}", changed.display()),
                }
            }
        }
        Command::Mcp => {
            mcp::run(grasp)?;
        }
        Command::Embed => {
            println!("Generating embeddings (first run downloads ~90MB model)…");
            let n = grasp.embed_backfill()?;
            println!("Embedded {n} memories. Search is now hybrid (keyword + semantic).");
        }
        Command::Eval { path, k } => {
            let path = path.unwrap_or_else(|| PathBuf::from("eval/queries.json"));
            eval::run(&grasp, &path, k)?;
        }
        Command::Redact => {
            let changed = grasp.redact_existing()?;
            println!("Re-scrubbed memories. {changed} chunk(s) had secrets redacted.");
        }
        Command::Setup => {
            let exe = std::env::current_exe()?;
            let exe_str = exe.display().to_string();
            let manual = format!("claude mcp add grasp -s user -- \"{exe_str}\" mcp");
            if register_mcp(&exe_str) {
                println!("✓ Registered Grasp with Claude Code (all projects).");
                println!("  Open a session and ask it about your past work — it'll use Grasp.");
            } else {
                println!("Grasp binary: {exe_str}");
                println!("Couldn't run the `claude` CLI automatically. Run this once:");
                println!("  {manual}");
            }
        }
        Command::Forget { project } => {
            let removed = grasp.forget(&project)?;
            println!("Forgot {removed} memories from {project}.");
        }
        Command::Reset { yes } => {
            if !yes {
                eprintln!("This wipes ALL memory. Re-run with --yes to confirm.");
                std::process::exit(2);
            }
            grasp.reset()?;
            println!("All memory wiped.");
        }
    }
    Ok(())
}
