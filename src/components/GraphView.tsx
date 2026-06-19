import { useEffect, useMemo, useState } from "react";
import ForceGraph2D from "react-force-graph-2d";
import { api } from "../lib/api";
import { TYPE_META, typeColor } from "../lib/format";
import { useElementSize } from "../lib/useElementSize";

interface Props {
  project: string | null;
  onOpen: (id: number) => void;
}

interface GNode {
  id: number;
  label: string;
  type: keyof typeof TYPE_META;
  project: string;
}
interface GLink {
  source: number;
  target: number;
}

export function GraphView({ project, onOpen }: Props) {
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
            label: n.label,
            type: n.chunk_type,
            project: n.project,
          }))
        );
        setLinks(g.edges.map((e) => ({ source: e.source, target: e.target })));
        setLoading(false);
      })
      .catch(() => setLoading(false));
    return () => {
      alive = false;
    };
  }, [project]);

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
            backgroundColor="#0e1016"
            nodeRelSize={4}
            nodeColor={(n: any) => typeColor(n.type)}
            nodeLabel={(n: any) =>
              `${TYPE_META[n.type as keyof typeof TYPE_META].label}: ${n.label}`
            }
            linkColor={() => "rgba(120,140,180,0.18)"}
            linkWidth={1}
            onNodeClick={(n: any) => onOpen(n.id)}
            nodeCanvasObjectMode={() => "after"}
            nodeCanvasObject={(n: any, ctx, scale) => {
              // glow
              ctx.beginPath();
              ctx.arc(n.x, n.y, 5, 0, 2 * Math.PI);
              ctx.fillStyle = typeColor(n.type);
              ctx.shadowColor = typeColor(n.type);
              ctx.shadowBlur = 8;
              ctx.fill();
              ctx.shadowBlur = 0;
              // label only when zoomed in enough, to avoid clutter
              if (scale > 2.2) {
                ctx.font = `${10 / scale}px sans-serif`;
                ctx.fillStyle = "rgba(230,232,238,0.85)";
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
    </div>
  );
}
