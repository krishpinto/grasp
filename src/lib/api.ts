// Typed wrappers around the Tauri commands exposed by src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";

export type ChunkType =
  | "decision"
  | "file_write"
  | "error_resolution"
  | "summary"
  | "context";

export interface SearchHit {
  id: number;
  project: string;
  session_id: string;
  text: string;
  timestamp: string;
  chunk_type: ChunkType;
  md_path: string;
  score: number;
}

export interface ProjectRow {
  slug: string;
  path: string;
  last_seen: string;
  chunk_count: number;
}

export interface Stats {
  total_chunks: number;
  total_projects: number;
}

export interface MemoryDetail {
  id: number;
  project: string;
  session_id: string;
  text: string;
  timestamp: string;
  chunk_type: ChunkType;
  md_path: string;
}

export interface GraphNode {
  id: number;
  /// "memory" or "file".
  node_type: string;
  label: string;
  chunk_type: ChunkType;
  project: string;
  session_id: string;
  timestamp: string;
}

export interface GraphEdge {
  source: number;
  target: number;
  kind: string;
}

export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface ImportReport {
  files_processed: number;
  files_skipped: number;
  chunks_added: number;
}

export const api = {
  stats: () => invoke<Stats>("stats"),
  listProjects: () => invoke<ProjectRow[]>("list_projects"),
  search: (query: string, project?: string, limit = 30) =>
    invoke<SearchHit[]>("search", { query, project, limit }),
  getGraph: (project?: string) => invoke<Graph>("get_graph", { project }),
  getMemory: (id: number) => invoke<MemoryDetail | null>("get_memory", { id }),
  import: (path?: string) => invoke<ImportReport>("import", { path }),
};
