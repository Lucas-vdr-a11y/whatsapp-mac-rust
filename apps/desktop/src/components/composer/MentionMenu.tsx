/**
 * Mention autocomplete popover for the composer.
 *
 * Purely presentational: the Composer owns the trigger state, the filtered
 * participant list and the keyboard selection; this component renders the
 * options and keeps the active one scrolled into view.
 */

import { useEffect, useRef } from "react";
import { useTranslation } from "../../lib/i18n";
import { initials } from "../../lib/names";
import type { MentionParticipant } from "./mentions";

export interface MentionMenuProps {
  /** Already filtered by the active query. */
  participants: MentionParticipant[];
  /** Keyboard selection index into `participants`. */
  activeIndex: number;
  onHover: (index: number) => void;
  onPick: (participant: MentionParticipant) => void;
}

export function MentionMenu({
  participants,
  activeIndex,
  onHover,
  onPick,
}: MentionMenuProps) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const active = listRef.current?.querySelector<HTMLElement>(
      '[data-active="true"]',
    );
    active?.scrollIntoView({ block: "nearest" });
  }, [activeIndex, participants]);

  return (
    <div
      ref={listRef}
      className="mention-menu"
      role="listbox"
      aria-label={t("mention.suggestionsAria")}
    >
      <div className="mention-menu-header">{t("mention.groupMembers")}</div>
      {participants.length === 0 ? (
        <div className="mention-empty">{t("mention.noMatches")}</div>
      ) : (
        participants.map((participant, index) => {
          const active = index === activeIndex;
          return (
            <button
              key={participant.id}
              type="button"
              role="option"
              aria-selected={active}
              data-active={active}
              className={`mention-item${active ? " active" : ""}`}
              onMouseEnter={() => onHover(index)}
              // Keep the caret in the textarea; the click still lands on this
              // button, so picking works without losing the selection.
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onPick(participant)}
            >
              <span className="mention-avatar">{initials(participant.name)}</span>
              <span className="mention-name">{participant.name}</span>
            </button>
          );
        })
      )}
    </div>
  );
}
