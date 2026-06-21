import type { ChunkType } from "./api";

// Soft, clean pastels that read well on the dark graph canvas.
export const TYPE_META: Record<ChunkType, { label: string; color: string }> = {
  decision: { label: "Decision", color: "#b3a4f5" },
  file_write: { label: "File Write", color: "#8ed6ab" },
  error_resolution: { label: "Error Fix", color: "#ef9a9a" },
  summary: { label: "Summary", color: "#9ec1f0" },
  context: { label: "Question", color: "#e9c07a" },
};

export function typeColor(t: ChunkType): string {
  return TYPE_META[t]?.color ?? "#8b93a7";
}

export function shortDate(ts: string): string {
  return (ts || "").split("T")[0] || ts;
}
