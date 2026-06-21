interface Props {
  caption?: string;
  /// Pixel size of the network mark.
  size?: number;
}

// Outer node positions in the 100x100 viewBox (a little "brain" of nodes).
const NODES = [
  { x: 50, y: 14 },
  { x: 84, y: 40 },
  { x: 68, y: 82 },
  { x: 22, y: 76 },
  { x: 14, y: 32 },
];

/// A small animated neural network: connections pulse, nodes breathe, and the
/// whole graph rotates slowly so it reads as something *alive* forming — used as
/// the launch splash and as the graph's loading state.
export function BrainLoader({ caption, size = 96 }: Props) {
  return (
    <div className="brain-loader">
      <svg className="brain-net" viewBox="0 0 100 100" width={size} height={size}>
        <g className="brain-net-rot">
          {NODES.map((n, i) => (
            <line
              key={`l${i}`}
              x1="50"
              y1="50"
              x2={n.x}
              y2={n.y}
              className="bn-line"
              style={{ animationDelay: `${i * 0.18}s` }}
            />
          ))}
          {NODES.map((n, i) => (
            <circle
              key={`n${i}`}
              cx={n.x}
              cy={n.y}
              r="5"
              className="bn-node"
              style={{ animationDelay: `${i * 0.18}s` }}
            />
          ))}
          <circle cx="50" cy="50" r="7" className="bn-core" />
        </g>
      </svg>
      {caption && <div className="loading-sub">{caption}</div>}
    </div>
  );
}
