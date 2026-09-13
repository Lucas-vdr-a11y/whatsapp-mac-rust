/** Floating submenu for the message context menu.
 *
 * The shared `ContextMenu` supports neither custom rows nor submenus, so the
 * quick-reaction row and the delete choices render in a tiny popover that
 * reuses the `.context-menu` visuals. Opens at the originating menu's
 * coordinates, closes on Escape/outside pointer-down, and clamps to the
 * viewport exactly like `ContextMenu`. */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Trash } from "lucide-react";

/** WhatsApp's quick-reaction set. */
const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"];

const VIEWPORT_MARGIN = 8;

interface MessagePopoverProps {
  kind: "react" | "delete";
  x: number;
  y: number;
  /** Own messages may be revoked for everyone; others only for me. */
  canDeleteForEveryone: boolean;
  onClose: () => void;
  onPickEmoji: (emoji: string) => void;
  onDelete: (forEveryone: boolean) => void;
}

export function MessagePopover({
  kind,
  x,
  y,
  canDeleteForEveryone,
  onClose,
  onPickEmoji,
  onDelete,
}: MessagePopoverProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });
  const [measured, setMeasured] = useState(false);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const { width, height } = element.getBoundingClientRect();
    setPosition({
      x: Math.max(
        VIEWPORT_MARGIN,
        Math.min(x, window.innerWidth - width - VIEWPORT_MARGIN),
      ),
      y: Math.max(
        VIEWPORT_MARGIN,
        Math.min(y, window.innerHeight - height - VIEWPORT_MARGIN),
      ),
    });
    setMeasured(true);
  }, [x, y]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("pointerdown", handlePointerDown, true);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("pointerdown", handlePointerDown, true);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  useEffect(() => {
    ref.current?.querySelector<HTMLButtonElement>("button")?.focus({
      preventScroll: true,
    });
  }, []);

  return (
    <div
      ref={ref}
      className="context-menu message-popover"
      role="menu"
      style={{
        left: position.x,
        top: position.y,
        opacity: measured ? 1 : 0,
        pointerEvents: measured ? "auto" : "none",
      }}
    >
      {kind === "react" ? (
        <div className="react-row" role="group" aria-label="React">
          {QUICK_REACTIONS.map((emoji) => (
            <button
              key={emoji}
              type="button"
              className="react-emoji"
              title={`React with ${emoji}`}
              onClick={() => {
                onPickEmoji(emoji);
                onClose();
              }}
            >
              {emoji}
            </button>
          ))}
        </div>
      ) : (
        <>
          {canDeleteForEveryone ? (
            <button
              type="button"
              role="menuitem"
              className="context-menu-item danger"
              onClick={() => {
                onDelete(true);
                onClose();
              }}
            >
              <span className="context-menu-icon">
                <Trash size={18} />
              </span>
              <span className="context-menu-label">Delete for everyone</span>
            </button>
          ) : null}
          <button
            type="button"
            role="menuitem"
            className="context-menu-item danger"
            onClick={() => {
              onDelete(false);
              onClose();
            }}
          >
            <span className="context-menu-icon">
              <Trash size={18} />
            </span>
            <span className="context-menu-label">Delete for me</span>
          </button>
        </>
      )}
    </div>
  );
}
