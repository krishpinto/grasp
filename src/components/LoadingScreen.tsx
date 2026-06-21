import { BrainLoader } from "./BrainLoader";

interface Props {
  /// When true, plays the fade-out before unmounting.
  leaving: boolean;
}

/// First-impression splash: the animated neural network + the wordmark.
export function LoadingScreen({ leaving }: Props) {
  return (
    <div className={`loading${leaving ? " is-leaving" : ""}`}>
      <BrainLoader size={104} />
      <div className="loading-word">Engram</div>
      <div className="loading-sub">Loading your memory…</div>
    </div>
  );
}
