import type { ChunkType } from "./api";

export const TYPE_META: Record<ChunkType, { label: string; color: string }> = {
  decision: { label: "Decision", color: "#c89bff" },
  file_write: { label: "File Write", color: "#74e0a8" },
  error_resolution: { label: "Error Fix", color: "#ff9b9b" },
  summary: { label: "Summary", color: "#9bc4ff" },
  context: { label: "Question", color: "#ffd98b" },
};

export function typeColor(t: ChunkType): string {
  return TYPE_META[t]?.color ?? "#8b93a7";
}

export function shortDate(ts: string): string {
  return (ts || "").split("T")[0] || ts;
}
