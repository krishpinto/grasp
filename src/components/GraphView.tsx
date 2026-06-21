import { useEffect, useMemo, useState } from "react";
import ForceGraph2D from "react-force-graph-2d";
import { api } from "../lib/api";
import { TYPE_META, typeColor } from "../lib/format";
import { useElementSize } from "../lib/useElementSize";

interface Props {
  project: string | null;
  onOpen: (id: number) => void;
  /// Bumped by the parent when memories change, to force a reload.
  reloadKey?: number;
}

interface GNode {
  id: number;
  nodeType: string; // "memory" | "file"
  label: string;
  type: keyof typeof TYPE_META;
  project: string;
}
interface GLink {
  source: number;
  target: number;
  kind: string; // "session" | "file" | "semantic"
}

const FILE_COLOR = "#475569";
const EDGE_COLOR: Record<string, string> = {
  session: "rgba(100,116,139,0.20)",
  file: "rgba(71,85,105,0.30)",
  semantic: "rgba(124,58,237,0.30)",
};

function nodeColor(n: { nodeType: string; type: keyof typeof TYPE_META }): string {
  return n.nodeType === "file" ? FILE_COLOR : typeColor(n.type);
}

export function GraphView({ project, onOpen, reloadKey }: Props) {
  const { ref, width, height } = useElementSize<HTMLDivElement>();
  const [nodes, setNodes] = useState<GNode[]>([]);
  const [links, setLinks] = useState<GLink[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    api
      .getGraph(project ?? undefined)
      .then((g) => {
        if (!alive) return;
        setNodes(
          g.nodes.map((n) => ({
            id: n.id,
            nodeType: n.node_type,
            label: n.label,
            type: n.chunk_type,
            project: n.project,
          }))
        );
        setLinks(
          g.edges.map((e) => ({ source: e.source, target: e.target, kind: e.kind }))
        );
        setLoading(false);
      })
      .catch(() => setLoading(false));
    return () => {
      alive = false;
    };
  }, [project, reloadKey]);

  const data = useMemo(() => ({ nodes, links }), [nodes, links]);

  return (
    <div className="graph-wrap" ref={ref}>
      {loading && <div className="placeholder">Building graph…</div>}
      {!loading && nodes.length === 0 && (
        <div className="placeholder">No memories to graph yet — import first.</div>
      )}
      {!loading && nodes.length > 0 && width > 0 && (
        <>
          <ForceGraph2D
            width={width}
            height={height}
            graphData={data}
            backgroundColor="#f6f7f9"
            nodeRelSize={4}
            nodeColor={(n: any) => nodeColor(n)}
            nodeLabel={(n: any) =>
              n.nodeType === "file"
                ? `File: ${n.label}`
                : `${TYPE_META[n.type as keyof typeof TYPE_META].label}: ${n.label}`
            }
            linkColor={(l: any) => EDGE_COLOR[l.kind] ?? "rgba(120,140,180,0.18)"}
            linkWidth={(l: any) => (l.kind === "semantic" ? 1.2 : 1)}
            // Only memories open in the note view; file nodes are just anchors.
            onNodeClick={(n: any) => {
              if (n.nodeType !== "file") onOpen(n.id);
            }}
            nodeCanvasObjectMode={() => "after"}
            nodeCanvasObject={(n: any, ctx, scale) => {
              const color = nodeColor(n);
              const isFile = n.nodeType === "file";
              const r = isFile ? 6 : 5;
              if (isFile) {
                // files are hubs: a square marker, so they read differently
                ctx.fillStyle = color;
                ctx.shadowColor = color;
                ctx.shadowBlur = 6;
                ctx.fillRect(n.x - r / 2, n.y - r / 2, r, r);
                ctx.shadowBlur = 0;
              } else {
                ctx.beginPath();
                ctx.arc(n.x, n.y, r, 0, 2 * Math.PI);
                ctx.fillStyle = color;
                ctx.shadowColor = color;
                ctx.shadowBlur = 8;
                ctx.fill();
                ctx.shadowBlur = 0;
              }
              // File labels appear sooner (fewer of them, useful as anchors).
              const labelAt = isFile ? 1.4 : 2.2;
              if (scale > labelAt) {
                ctx.font = `${10 / scale}px Inter, sans-serif`;
                ctx.fillStyle = "rgba(39,39,42,0.82)";
                ctx.fillText(n.label, n.x + 7 / scale, n.y + 3 / scale);
              }
            }}
          />
          <Legend />
        </>
      )}
    </div>
  );
}

function Legend() {
  return (
    <div className="legend">
      {(Object.keys(TYPE_META) as (keyof typeof TYPE_META)[]).map((t) => (
        <span key={t} className="legend-item">
          <span className="dot" style={{ background: TYPE_META[t].color }} />
          {TYPE_META[t].label}
        </span>
      ))}
      <span className="legend-item">
        <span className="dot" style={{ background: FILE_COLOR, borderRadius: 2 }} />
        File
      </span>
    </div>
  );
}
