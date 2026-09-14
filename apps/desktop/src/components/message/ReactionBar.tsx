/** Reaction chips shown under a bubble.
 *
 * Counts come from `store.reactions`; clicking a chip toggles our own
 * reaction (the chip we already picked clears it). */

import { useTranslation } from "../../lib/i18n";

interface ReactionBarProps {
  reactions: Record<string, number>;
  mine: string | null;
  onToggle: (emoji: string) => void;
}

export function ReactionBar({ reactions, mine, onToggle }: ReactionBarProps) {
  const { t } = useTranslation();

  return (
    <div className="reaction-bar">
      {Object.entries(reactions).map(([emoji, count]) => (
        <button
          key={emoji}
          type="button"
          className={`reaction-chip${mine === emoji ? " mine" : ""}`}
          title={
            mine === emoji
              ? t("message.removeReaction")
              : t("message.reactWith", { emoji })
          }
          onClick={() => onToggle(emoji)}
        >
          <span className="reaction-emoji">{emoji}</span>
          {count > 1 ? <span className="reaction-count">{count}</span> : null}
        </button>
      ))}
    </div>
  );
}
