// On Windows, hide the extra console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use std::sync::Mutex;
use std::time::Duration;

use grasp_core::Grasp;
use tauri::{Emitter, Manager};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    // Open the engine once and share it (behind a mutex) across commands.
    let grasp = Grasp::open().expect("failed to open Grasp database");

    tauri::Builder::default()
        .manage(Mutex::new(grasp))
        .setup(|app| {
            spawn_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::stats,
            commands::list_projects,
            commands::search,
            commands::get_graph,
            commands::get_memory,
            commands::import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Grasp");
}

/// Watch ~/.claude/projects in the background while the app is open. New
/// transcripts are ingested live and the UI is notified via `memories-updated`.
/// This is the passive-capture path: no user or agent action required.
fn spawn_watcher(handle: tauri::AppHandle) {
    let dir = {
        let state = handle.state::<commands::GraspState>();
        let grasp = state.lock().expect("grasp state poisoned");
        grasp.config.claude_projects_dir.clone()
    };

    std::thread::spawn(move || {
        let watcher = match grasp_core::watch::watch(&dir, Duration::from_millis(1000)) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("watcher failed to start on {}: {e}", dir.display());
                return;
            }
        };
        tracing::info!("watching {} for new transcripts", dir.display());
        for changed in watcher.changes {
            let state = handle.state::<commands::GraspState>();
            let added = match state.lock() {
                Ok(grasp) => grasp.ingest_file(&changed).unwrap_or(0),
                Err(_) => continue,
            };
            if added > 0 {
                let _ = handle.emit("memories-updated", added);
            }
        }
    });
}
