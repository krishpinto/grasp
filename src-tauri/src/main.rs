// On Windows, hide the extra console window in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use std::sync::Mutex;

use engram_core::Engram;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    // Open the engine once and share it (behind a mutex) across commands.
    let engram = Engram::open().expect("failed to open Engram database");

    tauri::Builder::default()
        .manage(Mutex::new(engram))
        .invoke_handler(tauri::generate_handler![
            commands::stats,
            commands::list_projects,
            commands::search,
            commands::get_graph,
            commands::get_memory,
            commands::import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Engram");
}
