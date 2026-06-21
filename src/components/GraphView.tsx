import { useEffect, useMemo, useRef, useState } from "react";
import ForceGraph3D from "react-force-graph-3d";
import SpriteText from "three-spritetext";
import * as THREE from "three";
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
  deg: number;
  neighbors: GNode[];
  links: GLink[];
}
interface GLink {
  source: number | GNode;
  target: number | GNode;
  kind: string;
}

const FILE_NODE_COLOR = "#9aa0aa";
const DIM_NODE_COLOR = "#3a3a3c";
const LINK_BASE = "rgba(150,152,160,0.13)";
const LINK_HI = "#8b7cff";
const ACCENT = "#8b7cff";
const ACCENT_BRIGHT = "#b5acff";

function baseColor(n: GNode): string {
  return n.nodeType === "file" ? FILE_NODE_COLOR : typeColor(n.type);
}

export function GraphView({ project, onOpen, reloadKey }: Props) {
  const { ref, width, height } = useElementSize<HTMLDivElement>();
  const fgRef = useRef<any>(null);
  const [data, setData] = useState<{ nodes: GNode[]; links: GLink[] }>({
    nodes: [],
    links: [],
  });
  const [loading, setLoading] = useState(true);

  // Hover highlight state (Obsidian-style: light up a node + its neighbours).
  const [hoverNode, setHoverNode] = useState<GNode | null>(null);
  const highlightNodes = useRef(new Set<GNode>());
  const highlightLinks = useRef(new Set<GLink>());
  const [, force] = useState(0);
  const rerender = () => force((n) => n + 1);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    api
      .getGraph(project ?? undefined)
      .then((g) => {
        if (!alive) return;
        // Build nodes and cross-link neighbours/edges for highlight + sizing.
        const byId = new Map<number, GNode>();
        for (const n of g.nodes) {
          byId.set(n.id, {
            id: n.id,
            nodeType: n.node_type,
            label: n.label,
            type: n.chunk_type,
            project: n.project,
            deg: 0,
            neighbors: [],
            links: [],
          });
        }
        const links: GLink[] = [];
        for (const e of g.edges) {
          const a = byId.get(e.source);
          const b = byId.get(e.target);
          if (!a || !b) continue;
          const link: GLink = { source: a.id, target: b.id, kind: e.kind };
          links.push(link);
          a.deg++;
          b.deg++;
          a.neighbors.push(b);
          b.neighbors.push(a);
          a.links.push(link);
          b.links.push(link);
        }
        highlightNodes.current.clear();
        highlightLinks.current.clear();
        setHoverNode(null);
        setData({ nodes: [...byId.values()], links });
        setLoading(false);
      })
      .catch(() => setLoading(false));
    return () => {
      alive = false;
    };
  }, [project, reloadKey]);

  const graphData = useMemo(() => data, [data]);

  function handleHover(node: GNode | null) {
    highlightNodes.current.clear();
    highlightLinks.current.clear();
    if (node) {
      highlightNodes.current.add(node);
      node.neighbors.forEach((n) => highlightNodes.current.add(n));
      node.links.forEach((l) => highlightLinks.current.add(l));
    }
    setHoverNode(node);
    rerender();
  }

  const anyHighlight = highlightNodes.current.size > 0;

  return (
    <div className="graph-wrap" ref={ref}>
      {loading && <div className="placeholder">Building graph…</div>}
      {!loading && data.nodes.length === 0 && (
        <div className="placeholder">No memories to graph yet — import first.</div>
      )}
      {!loading && data.nodes.length > 0 && width > 0 && (
        <>
          <ForceGraph3D
            ref={fgRef}
            width={width}
            height={height}
            graphData={graphData}
            backgroundColor="#1e1e1e"
            showNavInfo={false}
            nodeRelSize={4}
            nodeVal={(n: any) => 1 + Math.sqrt(n.deg)}
            nodeColor={(n: any) => {
              if (anyHighlight) {
                if (n === hoverNode) return ACCENT_BRIGHT;
                if (highlightNodes.current.has(n)) return ACCENT;
                return DIM_NODE_COLOR;
              }
              return baseColor(n);
            }}
            nodeOpacity={0.95}
            nodeThreeObjectExtend
            nodeThreeObject={(n: any) => {
              // Label floats above the node; in 3D it naturally reads when you
              // move the camera in and shrinks away when you pull back.
              const sprite = new SpriteText(n.label) as any;
              sprite.color = "#c8c9cc";
              sprite.textHeight = n.nodeType === "file" ? 3.5 : 2.6;
              sprite.fontFace = "Inter, sans-serif";
              sprite.material.depthWrite = false;
              const group = new THREE.Group();
              sprite.position.set(0, n.nodeType === "file" ? 7 : 6, 0);
              group.add(sprite);
              return group;
            }}
            linkColor={(l: any) =>
              highlightLinks.current.has(l) ? LINK_HI : LINK_BASE
            }
            linkWidth={(l: any) => (highlightLinks.current.has(l) ? 1.4 : 0.4)}
            linkOpacity={0.5}
            linkDirectionalParticles={(l: any) =>
              highlightLinks.current.has(l) ? 2 : 0
            }
            linkDirectionalParticleWidth={1.6}
            linkDirectionalParticleColor={() => ACCENT_BRIGHT}
            onNodeHover={(n: any) => handleHover(n || null)}
            onNodeClick={(n: any) => {
              if (n.nodeType !== "file") onOpen(n.id);
            }}
          />
          <div className="graph-hint">drag to orbit · scroll to zoom · right-drag to pan</div>
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
        <span className="dot" style={{ background: FILE_NODE_COLOR }} />
        File
      </span>
    </div>
  );
}
