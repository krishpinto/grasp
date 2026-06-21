interface Props {
  /// When true, plays the fade-out before unmounting.
  leaving: boolean;
}

/// First-impression splash: a small pulsing "brain" of nodes + the wordmark.
/// Shown briefly on launch, then fades out (occasional/first-time, so delight
/// is appropriate here per the animation framework).
export function LoadingScreen({ leaving }: Props) {
  return (
    <div className={`loading${leaving ? " is-leaving" : ""}`}>
      <div className="loading-brain">
        <span className="node core" />
        <span className="node n1" />
        <span className="node n2" />
        <span className="node n3" />
        <span className="node n4" />
        <span className="node n5" />
      </div>
      <div className="loading-word">Engram</div>
      <div className="loading-sub">Loading your memory…</div>
    </div>
  );
}
