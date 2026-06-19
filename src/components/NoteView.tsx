import type { MemoryDetail } from "../lib/api";
import { TYPE_META, shortDate } from "../lib/format";

interface Props {
  memory: MemoryDetail | null;
  onClose: () => void;
}

export function NoteView({ memory, onClose }: Props) {
  if (!memory) return null;
  const meta = TYPE_META[memory.chunk_type];
  return (
    <div className="note-overlay" onClick={onClose}>
      <div className="note" onClick={(e) => e.stopPropagation()}>
        <div className="note-head">
          <span className="tag" style={{ color: meta.color, borderColor: meta.color }}>
            {meta.label}
          </span>
          <span className="note-meta">
            {shortDate(memory.timestamp)} · {memory.project}
          </span>
          <button className="close" onClick={onClose}>
            ✕
          </button>
        </div>
        <pre className="note-body">{memory.text}</pre>
        <div className="note-foot" title={memory.md_path}>
          {memory.md_path}
        </div>
      </div>
    </div>
  );
}
