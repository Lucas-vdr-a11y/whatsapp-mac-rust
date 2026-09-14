import type { ReactNode } from "react";
import { useTranslation } from "../lib/i18n";
import { useAppStore } from "../store/app";
import {
  ArchiveGlyph,
  CallsGlyph,
  ChatsGlyph,
  MediaGlyph,
  SettingsGlyph,
  StarGlyph,
  StatusGlyph,
} from "./icons";

export type RailSection =
  | "chats"
  | "calls"
  | "status"
  | "archived"
  | "starred"
  | "media"
  | "channels"
  | "communities"
  | "settings"
  | "profile";

interface RailItem {
  id: RailSection;
  labelKey: string;
  icon: ReactNode;
}

// Order mirrors the official macOS app: Chats, Calls, Updates, then Archived
// and Starred behind a divider, with Media and Settings pinned to the bottom.
// Channels, communities and the profile live inside the chat list/settings.
const primaryItems: RailItem[] = [
  { id: "chats", labelKey: "nav.chats", icon: <ChatsGlyph size={24} /> },
  { id: "calls", labelKey: "nav.calls", icon: <CallsGlyph size={24} /> },
  { id: "status", labelKey: "nav.status", icon: <StatusGlyph size={24} /> },
];

const archivedItems: RailItem[] = [
  { id: "archived", labelKey: "nav.archived", icon: <ArchiveGlyph size={24} /> },
  { id: "starred", labelKey: "nav.starred", icon: <StarGlyph size={24} /> },
];

const secondaryItems: RailItem[] = [
  { id: "media", labelKey: "nav.media", icon: <MediaGlyph size={24} /> },
  { id: "settings", labelKey: "nav.settings", icon: <SettingsGlyph size={24} /> },
];

interface NavigationRailProps {
  active: RailSection;
  onSelect: (section: RailSection) => void;
  unreadCount: number;
}

export function NavigationRail({
  active,
  onSelect,
  unreadCount,
}: NavigationRailProps) {
  const { t } = useTranslation();

  // The archived badge counts unread archived chats only; muted chats do not
  // contribute, mirroring the chats badge.
  const archivedUnreadCount = useAppStore((state) =>
    state.chats.reduce(
      (sum, chat) =>
        sum +
        (chat.isArchived && !chat.muted ? chat.unreadCount : 0),
      0,
    ),
  );

  const renderItem = (item: RailItem) => {
    const badgeCount =
      item.id === "chats"
        ? unreadCount
        : item.id === "archived"
          ? archivedUnreadCount
          : 0;
    const textBadge = item.id === "archived";
    return (
      <button
        key={item.id}
        type="button"
        title={t(item.labelKey)}
        aria-label={t(item.labelKey)}
        aria-current={active === item.id ? "page" : undefined}
        className={`rail-button${active === item.id ? " active" : ""}${
          textBadge ? " rail-button--text-badge" : ""
        }`}
        onClick={() => onSelect(item.id)}
      >
        {item.icon}
        {badgeCount > 0 && (
          <span className={textBadge ? "count" : "badge"}>
            {badgeCount > 99 ? "99+" : badgeCount}
          </span>
        )}
      </button>
    );
  };

  return (
    <nav className="rail">
      <div className="chrome-inset" data-tauri-drag-region />
      <div className="rail-nav">{primaryItems.map(renderItem)}</div>
      <div className="rail-divider" />
      <div className="rail-nav">{archivedItems.map(renderItem)}</div>
      <div className="rail-spacer" data-tauri-drag-region />
      <div className="rail-nav rail-nav--bottom">
        {secondaryItems.map(renderItem)}
      </div>
    </nav>
  );
}
