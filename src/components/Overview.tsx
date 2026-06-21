import { GraphView } from "./GraphView";
import type { Stats } from "../lib/api";

interface Props {
  stats: Stats | null;
  reloadKey?: number;
  importing: boolean;
  showSpotlight: boolean;
  onOpenMemory: (id: number) => void;
  onEnterArchive: () => void;
  onDismissSpotlight: () => void;
  onImport: () => void;
}

/// The landing screen: the whole-system "brain" (all projects) as a full-bleed
/// 3D graph, with a glowing call-to-action and a first-time guided spotlight.
export function Overview({
  stats,
  reloadKey,
  importing,
  showSpotlight,
  onOpenMemory,
  onEnterArchive,
  onDismissSpotlight,
  onImport,
}: Props) {
  return (
    <div className="overview">
      <GraphView project={null} onOpen={onOpenMemory} reloadKey={reloadKey} />

      <div className="overview-overlay">
        <div className="hero-stats">
          <div className="brand">
            <span className="brand-mark" />
            Engram
          </div>
          {stats && (
            <div className="dim">
              {stats.total_chunks} memories · {stats.total_projects} projects
            </div>
          )}
        </div>
        <button className="import-btn overlay-btn" onClick={onImport} disabled={importing}>
          {importing ? "Importing…" : "Import"}
        </button>
      </div>

      {!showSpotlight && (
        <button className="cta cta-float" onClick={onEnterArchive}>
          Browse your memories →
        </button>
      )}

      {showSpotlight && (
        <div className="spotlight" onClick={onDismissSpotlight}>
          <div className="spotlight-card" onClick={(e) => e.stopPropagation()}>
            <div className="spotlight-tip">
              This is your whole memory — every decision, file, and fix, connected.
            </div>
            <button className="cta" onClick={onEnterArchive}>
              Check out your memories →
            </button>
            <button className="spotlight-skip" onClick={onDismissSpotlight}>
              Maybe later
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
