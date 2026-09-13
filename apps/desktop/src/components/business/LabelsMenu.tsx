/**
 * "Label chat…" popover.
 *
 * Opened from the ChatList context menu at the menu's coordinates. Lists the
 * account's labels with a checkmark for the ones associated with the chat and
 * toggles associations optimistically (reverted on failure).
 *
 * Limitation: the core exposes no per-chat label read, so associations come
 * from the last `labels_list` snapshot. The footer says so, and edits made
 * here are local until the next sync/refetch.
 */

import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { Check, LoaderCircle, RefreshCw } from "lucide-react";
import { useTranslation } from "../../lib/i18n";
import type { Jid } from "../../lib/types";
import { useLabels } from "./useLabels";
import type { Label } from "./types";

interface LabelsMenuProps {
  chatId: Jid;
  chatName: string;
  /** Viewport coordinates, typically from the context menu. */
  x: number;
  y: number;
  onClose: () => void;
}

const VIEWPORT_MARGIN = 8;

/**
 * WhatsApp label colour indices, as seen in WhatsApp Web's label palette.
 * Indices outside the palette fall back to the accent colour.
 */
const LABEL_COLORS = [
  "#F2C94C",
  "#FFA97A",
  "#F7C4D0",
  "#A5C9F2",
  "#B6E3B0",
  "#7FD2C1",
  "#D8C4F2",
  "#F2B6E3",
  "#9ED0C3",
  "#D9C695",
  "#F0A6A6",
  "#9BC7F0",
  "#C5E0A5",
  "#B8B3F0",
  "#F0C48A",
  "#A8D8EA",
  "#E3A6C9",
  "#BFD3A0",
  "#A6B6F0",
  "#E0C3A8",
];

function labelColor(color: number | null): string {
  if (color === null || color < 0) return "var(--accent)";
  return LABEL_COLORS[color % LABEL_COLORS.length];
}

export function LabelsMenu({
  chatId,
  chatName,
  x,
  y,
  onClose,
}: LabelsMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });
  const [measured, setMeasured] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const { labels, loading, error, reload, pending, toggle } = useLabels();

  // Measure once per open and keep the popover inside the viewport.
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
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
  }, [x, y, labels.length]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("pointerdown", handlePointerDown, true);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("pointerdown", handlePointerDown, true);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  const handleToggle = (label: Label) => {
    setActionError(null);
    void toggle(chatId, label.id).catch((cause: unknown) => {
      setActionError(
        cause instanceof Error
          ? cause.message
          : t("labels.updateError"),
      );
    });
  };

  const handleNavigation = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const { key } = event;
    if (key !== "ArrowDown" && key !== "ArrowUp") return;
    const buttons = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ) ?? [],
    );
    if (buttons.length === 0) return;
    event.preventDefault();
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      key === "ArrowDown"
        ? current < 0
          ? 0
          : (current + 1) % buttons.length
        : current < 0
          ? buttons.length - 1
          : (current - 1 + buttons.length) % buttons.length;
    buttons[next]?.focus({ preventScroll: true });
  };

  return (
    <div
      ref={menuRef}
      className="labels-menu"
      role="dialog"
      aria-label={t("labels.aria", { name: chatName })}
      style={{
        left: position.x,
        top: position.y,
        opacity: measured ? 1 : 0,
        pointerEvents: measured ? "auto" : "none",
      }}
      onKeyDown={handleNavigation}
    >
      <header className="labels-menu-header">
        <span className="labels-menu-title">{t("labels.title")}</span>
        <span className="labels-menu-chat">{chatName}</span>
      </header>

      <div className="labels-menu-list">
        {loading ? (
          <p className="labels-menu-note">
            <LoaderCircle size={16} className="business-spin" />
            {t("labels.loading")}
          </p>
        ) : error ? (
          <div className="labels-menu-error">
            <p>{error}</p>
            <button
              type="button"
              className="labels-menu-retry"
              onClick={reload}
            >
              <RefreshCw size={14} />
              {t("common.tryAgain")}
            </button>
          </div>
        ) : labels.length === 0 ? (
          <p className="labels-menu-note">{t("labels.empty")}</p>
        ) : (
          labels.map((label) => {
            const associated = label.chatIds.includes(chatId);
            const busy = pending.has(label.id);
            return (
              <button
                key={label.id}
                type="button"
                role="menuitemcheckbox"
                aria-checked={associated}
                className="labels-menu-item"
                disabled={busy}
                onClick={() => handleToggle(label)}
              >
                <span
                  className="label-dot"
                  style={{ background: labelColor(label.color) }}
                  aria-hidden="true"
                />
                <span className="labels-menu-name">
                  {label.name ?? label.id}
                </span>
                <span className="labels-menu-check" aria-hidden="true">
                  {busy ? (
                    <LoaderCircle size={15} className="business-spin" />
                  ) : associated ? (
                    <Check size={15} strokeWidth={3} />
                  ) : null}
                </span>
              </button>
            );
          })
        )}
      </div>

      {actionError ? (
        <p className="labels-menu-action-error" role="alert">
          {actionError}
        </p>
      ) : null}

      <p className="labels-menu-footer">{t("labels.footer")}</p>
    </div>
  );
}
