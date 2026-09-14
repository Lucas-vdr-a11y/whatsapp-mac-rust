/**
 * Reusable positioned context menu.
 *
 * Rendered with fixed positioning at the requested (x, y) coordinates and
 * clamped to the viewport edges. Closes on Escape, outside pointer-down,
 * window blur/resize, or when an item activates.
 */

import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentType,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";

/** Minimal icon contract shared by lucide-react icons. */
export type ContextMenuIcon = ComponentType<{
  size?: number | string;
  strokeWidth?: number;
  className?: string;
}>;

export interface ContextMenuAction {
  kind?: "item";
  id: string;
  label: string;
  icon?: ContextMenuIcon;
  /** Optional keyboard shortcut hint, rendered right-aligned. */
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

export interface ContextMenuSeparator {
  kind: "separator";
  id: string;
}

export type ContextMenuEntry = ContextMenuAction | ContextMenuSeparator;

export interface ContextMenuProps {
  /** Viewport coordinates, typically from a contextmenu event. */
  x: number;
  y: number;
  items: ContextMenuEntry[];
  onClose: () => void;
}

const VIEWPORT_MARGIN = 8;

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });
  const [measured, setMeasured] = useState(false);

  // Measure once per open and keep the menu inside the viewport.
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
  }, [x, y, items]);

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

  useEffect(() => {
    const first = menuRef.current?.querySelector<HTMLButtonElement>(
      "button:not(:disabled)",
    );
    first?.focus({ preventScroll: true });
  }, []);

  const handleNavigation = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const { key } = event;
    if (
      key !== "ArrowDown" &&
      key !== "ArrowUp" &&
      key !== "Home" &&
      key !== "End"
    ) {
      return;
    }

    const buttons = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ) ?? [],
    );
    if (buttons.length === 0) return;

    event.preventDefault();
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next = 0;
    if (key === "ArrowDown") {
      next = current < 0 ? 0 : (current + 1) % buttons.length;
    } else if (key === "ArrowUp") {
      next =
        current < 0
          ? buttons.length - 1
          : (current - 1 + buttons.length) % buttons.length;
    } else if (key === "End") {
      next = buttons.length - 1;
    }
    buttons[next]?.focus({ preventScroll: true });
  };

  return (
    <div
      ref={menuRef}
      className="context-menu"
      role="menu"
      tabIndex={-1}
      style={{
        left: position.x,
        top: position.y,
        opacity: measured ? 1 : 0,
        pointerEvents: measured ? "auto" : "none",
      }}
      onKeyDown={handleNavigation}
    >
      {items.map((entry) => {
        if (entry.kind === "separator") {
          return (
            <div
              key={entry.id}
              className="context-menu-separator"
              role="separator"
            />
          );
        }

        const Icon = entry.icon;
        return (
          <button
            key={entry.id}
            type="button"
            role="menuitem"
            className={`context-menu-item${entry.danger ? " danger" : ""}`}
            disabled={entry.disabled}
            onClick={() => {
              entry.onSelect();
              onClose();
            }}
          >
            <span className="context-menu-icon">
              {Icon ? <Icon size={18} /> : null}
            </span>
            <span className="context-menu-label">{entry.label}</span>
            {entry.shortcut ? (
              <span className="context-menu-shortcut">{entry.shortcut}</span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
