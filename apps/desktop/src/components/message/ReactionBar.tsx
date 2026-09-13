/** Reaction chips shown under a bubble.
 *
 * Counts come from `store.reactions`; clicking a chip toggles our own
 * reaction (the chip we already picked clears it). */

interface ReactionBarProps {
  reactions: Record<string, number>;
  mine: string | null;
  onToggle: (emoji: string) => void;
}

export function ReactionBar({ reactions, mine, onToggle }: ReactionBarProps) {
  return (
    <div className="reaction-bar">
      {Object.entries(reactions).map(([emoji, count]) => (
        <button
          key={emoji}
          type="button"
          className={`reaction-chip${mine === emoji ? " mine" : ""}`}
          title={mine === emoji ? "Remove reaction" : `React with ${emoji}`}
          onClick={() => onToggle(emoji)}
        >
          <span className="reaction-emoji">{emoji}</span>
          {count > 1 ? <span className="reaction-count">{count}</span> : null}
        </button>
      ))}
    </div>
  );
}
