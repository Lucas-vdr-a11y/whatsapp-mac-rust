import { useState, type MouseEvent as ReactMouseEvent } from "react";
import { Archive, Bell, PinOff, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { formatListTime } from "../lib/time";
import { avatarSrc } from "../lib/avatar";
import { initials } from "../lib/names";
import type { ChatSummary, Jid } from "../lib/types";
import { useAppStore, type ChatFilter } from "../store/app";
import { ContextMenu, type ContextMenuEntry } from "./ContextMenu";
import { NewChatModal } from "./NewChatModal";
import {
  BellOff,
  CheckCheck,
  EllipsisVertical,
  Pin,
  Plus,
  Search,
} from "./icons";

const filters: { id: ChatFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unread", label: "Unread" },
  { id: "groups", label: "Groups" },
];

interface ChatListProps {
  chats: ChatSummary[];
  selectedId: Jid | null;
  onSelect: (id: Jid) => void;
}

export function ChatList({ chats, selectedId, onSelect }: ChatListProps) {
  const query = useAppStore((state) => state.query);
  const setQuery = useAppStore((state) => state.setQuery);
  const filter = useAppStore((state) => state.filter);
  const setFilter = useAppStore((state) => state.setFilter);
  const togglePinned = useAppStore((state) => state.togglePinned);
  const toggleMuted = useAppStore((state) => state.toggleMuted);
  const archiveChat = useAppStore((state) => state.archiveChat);
  const markRead = useAppStore((state) => state.markRead);
  const deleteChat = useAppStore((state) => state.deleteChat);

  const [newChatOpen, setNewChatOpen] = useState(false);
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    chat: ChatSummary;
  } | null>(null);

  const openMenu = (event: ReactMouseEvent, chat: ChatSummary) => {
    event.preventDefault();
    onSelect(chat.id);
    setMenu({ x: event.clientX, y: event.clientY, chat });
  };

  const menuItems: ContextMenuEntry[] = menu
    ? [
        {
          id: "pin",
          label: menu.chat.pinned ? "Unpin chat" : "Pin chat",
          icon: menu.chat.pinned ? PinOff : Pin,
          onSelect: () => togglePinned(menu.chat.id),
        },
        {
          id: "mute",
          label: menu.chat.muted
            ? "Unmute notifications"
            : "Mute notifications",
          icon: menu.chat.muted ? Bell : BellOff,
          onSelect: () => toggleMuted(menu.chat.id),
        },
        {
          id: "archive",
          label: "Archive chat",
          icon: Archive,
          onSelect: () => archiveChat(menu.chat.id),
        },
        {
          id: "read",
          label: "Mark as read",
          icon: CheckCheck,
          disabled: menu.chat.unreadCount === 0,
          onSelect: () => markRead(menu.chat.id),
        },
        { kind: "separator", id: "separator" },
        {
          id: "delete",
          label: "Delete chat",
          icon: Trash2,
          danger: true,
          onSelect: () => deleteChat(menu.chat.id),
        },
      ]
    : [];

  return (
    <section className="chat-list">
      <header className="chat-list-header" data-tauri-drag-region>
        <h1 className="chat-list-title">Chats</h1>
        <div className="header-actions no-drag">
          <button
            type="button"
            className="icon-button"
            title="New chat"
            onClick={() => setNewChatOpen(true)}
          >
            <Plus size={24} />
          </button>
          <button type="button" className="icon-button" title="Menu">
            <EllipsisVertical size={24} />
          </button>
        </div>
      </header>

      <div className="search-row">
        <label className="search-box">
          <Search size={18} />
          <input
            type="text"
            placeholder="Search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
      </div>

      <div className="filters">
        {filters.map((candidate) => (
          <button
            key={candidate.id}
            type="button"
            className={`filter-pill${filter === candidate.id ? " active" : ""}`}
            onClick={() => setFilter(candidate.id)}
          >
            {candidate.label}
          </button>
        ))}
      </div>

      <div className="chat-items">
        {chats.map((chat) => (
          <ChatListItem
            key={chat.id}
            chat={chat}
            selected={chat.id === selectedId}
            onSelect={() => onSelect(chat.id)}
            onContextMenu={(event) => openMenu(event, chat)}
          />
        ))}
        {chats.length === 0 && (
          <p
            style={{
              padding: "32px 24px",
              color: "var(--text-secondary)",
              textAlign: "center",
            }}
          >
            No chats found
          </p>
        )}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={() => setMenu(null)}
        />
      )}

      {newChatOpen && <NewChatModal onClose={() => setNewChatOpen(false)} />}
    </section>
  );
}

interface ChatListItemProps {
  chat: ChatSummary;
  selected: boolean;
  onSelect: () => void;
  onContextMenu: (event: ReactMouseEvent) => void;
}

function ChatListItem({
  chat,
  selected,
  onSelect,
  onContextMenu,
}: ChatListItemProps) {
  const isOwnLastMessage = chat.lastMessagePreview?.startsWith("You:") ?? false;

  return (
    <div
      className={`chat-item${selected ? " selected" : ""}`}
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") onSelect();
      }}
    >
      <ChatListAvatar chat={chat} />

      <div className="chat-item-body">
        <div className="chat-item-top">
          <span className="chat-item-name">{chat.name}</span>
          <span
            className={`chat-item-time${chat.unreadCount > 0 ? " unread" : ""}`}
          >
            {formatListTime(chat.lastActivityTs)}
          </span>
        </div>
        <div className="chat-item-bottom">
          <span className="chat-item-preview">
            {isOwnLastMessage && (
              <CheckCheck
                size={16}
                style={{ color: "var(--tick-read)", flex: "none" }}
              />
            )}
            <span className="preview-text">
              {chat.lastMessagePreview ?? "No messages yet"}
            </span>
          </span>
          <span className="chat-item-icons">
            {chat.muted && <BellOff size={16} />}
            {chat.pinned && <Pin size={16} />}
            {chat.unreadCount > 0 && (
              <span className="chat-item-badge">{chat.unreadCount}</span>
            )}
          </span>
        </div>
      </div>
    </div>
  );
}

function ChatListAvatar({ chat }: { chat: ChatSummary }) {
  const avatar = useAppStore((state) => state.avatars[chat.id]);
  const loadAvatar = useAppStore((state) => state.loadAvatar);

  useEffect(() => {
    loadAvatar(chat.id);
  }, [chat.id, loadAvatar]);

  return (
    <div className="avatar">
      {avatar ? <img src={avatarSrc(avatar)} alt="" /> : initials(chat.name)}
    </div>
  );
}
