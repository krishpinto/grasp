import type { ProjectRow } from "../lib/api";
import { prettyProject } from "../lib/format";

interface Props {
  projects: ProjectRow[];
  importing: boolean;
  onSelect: (slug: string) => void;
  onBack: () => void;
  onImport: () => void;
}

/// Project-wise archive of memories: a card per project, click to open its graph.
export function Archive({ projects, importing, onSelect, onBack, onImport }: Props) {
  return (
    <div className="screen">
      <div className="topbar">
        <button className="back-btn" onClick={onBack}>
          ← Overview
        </button>
        <div className="topbar-title">Your memories</div>
        <div className="topbar-actions">
          <button className="import-btn" onClick={onImport} disabled={importing}>
            {importing ? "Importing…" : "Import"}
          </button>
        </div>
      </div>

      {projects.length === 0 ? (
        <div className="placeholder">No projects yet — hit Import to populate your memory.</div>
      ) : (
        <div className="archive-grid">
          {projects.map((p) => (
            <button className="proj-card" key={p.slug} onClick={() => onSelect(p.slug)}>
              <div className="proj-card-name">{prettyProject(p.slug)}</div>
              <div className="proj-card-meta">{p.chunk_count} memories</div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
