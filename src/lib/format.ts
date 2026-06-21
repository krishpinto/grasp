import type { ChunkType } from "./api";

// Vivid enough to read on the light graph canvas and as legend dots.
export const TYPE_META: Record<ChunkType, { label: string; color: string }> = {
  decision: { label: "Decision", color: "#7c3aed" },
  file_write: { label: "File Write", color: "#059669" },
  error_resolution: { label: "Error Fix", color: "#dc2626" },
  summary: { label: "Summary", color: "#2563eb" },
  context: { label: "Question", color: "#d97706" },
};

export function typeColor(t: ChunkType): string {
  return TYPE_META[t]?.color ?? "#8b93a7";
}

export function shortDate(ts: string): string {
  return (ts || "").split("T")[0] || ts;
}
