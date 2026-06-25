//! Tauri commands — thin wrappers exposing the engine to the frontend.
//!
//! The `Grasp` handle owns a SQLite connection (which is `Send` but not
//! `Sync`), so it lives behind a `Mutex` in Tauri's managed state. Each command
//! locks it briefly. Errors are stringified for the JS side.

use std::sync::Mutex;

use grasp_core::store::graph::Graph;
use grasp_core::store::index::{MemoryDetail, ProjectRow, Stats};
use grasp_core::{Grasp, ImportReport, SearchHit};
use tauri::{AppHandle, Manager, State};

pub type GraspState = Mutex<Grasp>;

fn with<'a>(state: &'a State<GraspState>) -> std::sync::MutexGuard<'a, Grasp> {
    state.lock().expect("grasp state mutex poisoned")
}

#[tauri::command]
pub fn stats(state: State<GraspState>) -> Result<Stats, String> {
    with(&state).stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_projects(state: State<GraspState>) -> Result<Vec<ProjectRow>, String> {
    with(&state).projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search(
    state: State<GraspState>,
    query: String,
    project: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, String> {
    with(&state)
        .search(&query, project.as_deref(), limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_graph(state: State<GraspState>, project: Option<String>) -> Result<Graph, String> {
    with(&state)
        .graph(project.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_memory(state: State<GraspState>, id: i64) -> Result<Option<MemoryDetail>, String> {
    with(&state).get_memory(id).map_err(|e| e.to_string())
}

/// Import transcripts. Runs on a blocking background thread so a long import
/// (scanning every project) never freezes the UI; the engine lock is held only
/// inside that thread for the duration of the import.
#[tauri::command]
pub async fn import(app: AppHandle, path: Option<String>) -> Result<ImportReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<GraspState>();
        let grasp = state.lock().map_err(|_| "grasp state poisoned".to_string())?;
        let path = path.map(std::path::PathBuf::from);
        grasp.import(path.as_deref()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
