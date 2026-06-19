//! Tauri commands — thin wrappers exposing the engine to the frontend.
//!
//! The `Engram` handle owns a SQLite connection (which is `Send` but not
//! `Sync`), so it lives behind a `Mutex` in Tauri's managed state. Each command
//! locks it briefly. Errors are stringified for the JS side.

use std::sync::Mutex;

use engram_core::store::graph::Graph;
use engram_core::store::index::{MemoryDetail, ProjectRow, Stats};
use engram_core::{Engram, ImportReport, SearchHit};
use tauri::{AppHandle, Manager, State};

pub type EngramState = Mutex<Engram>;

fn with<'a>(state: &'a State<EngramState>) -> std::sync::MutexGuard<'a, Engram> {
    state.lock().expect("engram state mutex poisoned")
}

#[tauri::command]
pub fn stats(state: State<EngramState>) -> Result<Stats, String> {
    with(&state).stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_projects(state: State<EngramState>) -> Result<Vec<ProjectRow>, String> {
    with(&state).projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search(
    state: State<EngramState>,
    query: String,
    project: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    with(&state)
        .search(&query, project.as_deref(), limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_graph(state: State<EngramState>, project: Option<String>) -> Result<Graph, String> {
    with(&state)
        .graph(project.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_memory(state: State<EngramState>, id: i64) -> Result<Option<MemoryDetail>, String> {
    with(&state).get_memory(id).map_err(|e| e.to_string())
}

/// Import transcripts. Runs on a blocking background thread so a long import
/// (scanning every project) never freezes the UI; the engine lock is held only
/// inside that thread for the duration of the import.
#[tauri::command]
pub async fn import(app: AppHandle, path: Option<String>) -> Result<ImportReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<EngramState>();
        let engram = state.lock().map_err(|_| "engram state poisoned".to_string())?;
        let path = path.map(std::path::PathBuf::from);
        engram.import(path.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
