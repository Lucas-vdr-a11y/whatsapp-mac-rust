import type { ReactNode } from "react";
import { Star } from "lucide-react";
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
  label: string;
  icon: ReactNode;
}

// Order mirrors the official macOS app: Chats, Calls, Status, Channels,
// Starred. Communities live inside the chat list in the official client, so
// they are reachable but not a rail item here either.
const primaryItems: RailItem[] = [
  { id: "chats", label: "Chats", icon: <MessageCircle size={24} /> },
  { id: "calls", label: "Calls", icon: <Phone size={24} /> },
  { id: "status", label: "Status", icon: <CircleDashed size={24} /> },
  { id: "channels", label: "Channels", icon: <RadioTower size={24} /> },
  { id: "starred", label: "Starred messages", icon: <Star size={24} /> },
];

const secondaryItems: RailItem[] = [
  { id: "settings", label: "Settings", icon: <Settings size={24} /> },
  { id: "profile", label: "Profile", icon: <UserRound size={24} /> },
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
  const renderItem = (item: RailItem) => (
    <button
      key={item.id}
      type="button"
      title={item.label}
      aria-label={item.label}
      aria-current={active === item.id ? "page" : undefined}
      className={`rail-button${active === item.id ? " active" : ""}`}
      onClick={() => onSelect(item.id)}
    >
      {item.icon}
      {item.id === "chats" && unreadCount > 0 && (
        <span className="badge">{unreadCount > 99 ? "99+" : unreadCount}</span>
      )}
    </button>
  );

  return (
    <nav className="rail">
      <div className="chrome-inset" data-tauri-drag-region />
      <div className="rail-nav">{primaryItems.map(renderItem)}</div>
      <div className="rail-spacer" data-tauri-drag-region />
      <div className="rail-nav">{secondaryItems.map(renderItem)}</div>
    </nav>
  );
}
