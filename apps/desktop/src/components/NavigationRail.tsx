import type { ReactNode } from "react";
import { Star } from "lucide-react";
import { useTranslation } from "../lib/i18n";
import { useAppStore } from "../store/app";
import {
  CircleDashed,
  MessageCircle,
  Phone,
  RadioTower,
  Settings,
  UserRound,
} from "./icons";

export type RailSection =
  | "chats"
  | "calls"
  | "status"
  | "channels"
  | "starred"
  | "communities"
  | "settings"
  | "profile";

interface RailItem {
  id: RailSection;
  labelKey: string;
  icon: ReactNode;
}

// Order mirrors the official macOS app: Chats, Calls, Status, Channels,
// Starred. Communities live inside the chat list in the official client, so
// they are reachable but not a rail item here either.
const primaryItems: RailItem[] = [
  { id: "chats", labelKey: "nav.chats", icon: <MessageCircle size={24} /> },
  { id: "calls", labelKey: "nav.calls", icon: <Phone size={24} /> },
  { id: "status", labelKey: "nav.status", icon: <CircleDashed size={24} /> },
  { id: "channels", labelKey: "nav.channels", icon: <RadioTower size={24} /> },
  { id: "starred", labelKey: "nav.starred", icon: <Star size={24} /> },
];

const secondaryItems: RailItem[] = [
  { id: "settings", labelKey: "nav.settings", icon: <Settings size={24} /> },
  { id: "profile", labelKey: "nav.profile", icon: <UserRound size={24} /> },
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

  // Channels badge parity: unread from newsletter chats. Muted channels are
  // excluded, matching how the Chats badge counts in App.tsx.
  const channelUnreadCount = useAppStore((state) =>
    state.chats.reduce(
      (sum, chat) =>
        sum +
        (chat.id.endsWith("@newsletter") && !chat.muted ? chat.unreadCount : 0),
      0,
    ),
  );

  const renderItem = (item: RailItem) => {
    const badgeCount =
      item.id === "chats"
        ? unreadCount
        : item.id === "channels"
          ? channelUnreadCount
          : 0;
    return (
      <button
        key={item.id}
        type="button"
        title={t(item.labelKey)}
        aria-label={t(item.labelKey)}
        aria-current={active === item.id ? "page" : undefined}
        className={`rail-button${active === item.id ? " active" : ""}`}
        onClick={() => onSelect(item.id)}
      >
        {item.icon}
        {badgeCount > 0 && (
          <span className="badge">
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
      <div className="rail-spacer" data-tauri-drag-region />
      <div className="rail-nav">{secondaryItems.map(renderItem)}</div>
    </nav>
  );
}
