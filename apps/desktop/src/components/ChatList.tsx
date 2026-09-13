import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { Archive, Ban, Bell, Briefcase, PinOff, Tag, Trash2 } from "lucide-react";
import { invokeCore, isTauri } from "../lib/ipc";
import { formatListTime } from "../lib/time";
import { avatarSrc } from "../lib/avatar";
import { initials } from "../lib/names";
import type { ChatSummary, Jid, Message } from "../lib/types";
import { useAppStore, type ChatFilter } from "../store/app";
import { BusinessProfilePanel } from "./business/BusinessProfilePanel";
import { LabelsMenu } from "./business/LabelsMenu";
import { ContextMenu, type ContextMenuEntry } from "./ContextMenu";
import { ConfirmDialog } from "./settings/ConfirmDialog";
import { blockContact, privacyErrorMessage } from "./settings/privacy";
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

/** Direct chats can be blocked; groups, communities and newsletters cannot. */
function isDirectChat(chat: ChatSummary): boolean {
  return (
    !chat.isGroup &&
    !chat.id.endsWith("@newsletter") &&
    !chat.id.endsWith("@broadcast")
  );
}

/** Placeholder copy for search hits whose message carries no text body. */
const KIND_LABELS: Record<Message["kind"], string> = {
  text: "Message",
  image: "Photo",
  video: "Video",
  audio: "Audio",
  voiceNote: "Voice message",
  document: "Document",
  sticker: "Sticker",
  gif: "GIF",
  location: "Location",
  contact: "Contact",
  poll: "Poll",
  system: "System message",
  unsupported: "Unsupported message",
};

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

  // Business features opened from the context menu. `labelChat` keeps the menu
  // coordinates so the label popover lands where the user clicked.
  const [businessChat, setBusinessChat] = useState<ChatSummary | null>(null);
  const [labelChat, setLabelChat] = useState<{
    chat: ChatSummary;
    x: number;
    y: number;
  } | null>(null);

  // Close both panels when the selection changes (e.g. another chat is
  // picked): the profile shown would otherwise belong to a different chat.
  const lastSelectedRef = useRef(selectedId);
  useEffect(() => {
    if (lastSelectedRef.current === selectedId) return;
    lastSelectedRef.current = selectedId;
    setBusinessChat(null);
    setLabelChat(null);
  }, [selectedId]);

  const [blockTarget, setBlockTarget] = useState<ChatSummary | null>(null);
  const [blockBusy, setBlockBusy] = useState(false);
  const [blockError, setBlockError] = useState<string | null>(null);
  const [notice, setNotice] = useState<{
    kind: "ok" | "error";
    text: string;
  } | null>(null);

  const [hits, setHits] = useState<Message[]>([]);
  const [hitQuery, setHitQuery] = useState<string | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const allChats = useAppStore((state) => state.chats);
  const chatNames = useMemo(() => {
    const names = new Map<Jid, string>();
    for (const chat of allChats) names.set(chat.id, chat.name);
    return names;
  }, [allChats]);

  const trimmedQuery = query.trim();

  // The block confirmation is deliberately transient: nothing changes in the
  // local chat list when a contact is blocked.
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 5000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  // Global message search: debounced ~300ms, at least two characters, newest
  // first, capped at 30 hits.
  useEffect(() => {
    if (trimmedQuery.length < 2) {
      setHits([]);
      setHitQuery(null);
      setSearching(false);
      setSearchError(null);
      return;
    }
    let cancelled = false;
    setSearching(true);
    setSearchError(null);
    const timer = window.setTimeout(() => {
      void searchMessages(trimmedQuery, 30)
        .then((results) => {
          if (cancelled) return;
          setHits(results);
          setHitQuery(trimmedQuery);
          setSearching(false);
        })
        .catch((cause: unknown) => {
          if (cancelled) return;
          setHits([]);
          setHitQuery(trimmedQuery);
          setSearchError(searchErrorMessage(cause));
          setSearching(false);
        });
    }, 300);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trimmedQuery]);

  const openMenu = (event: ReactMouseEvent, chat: ChatSummary) => {
    event.preventDefault();
    onSelect(chat.id);
    setMenu({ x: event.clientX, y: event.clientY, chat });
  };

  const confirmBlock = () => {
    if (!blockTarget) return;
    setBlockBusy(true);
    setBlockError(null);
    void blockContact(blockTarget.id)
      .then(() => {
        setNotice({ kind: "ok", text: `${blockTarget.name} is blocked.` });
        setBlockTarget(null);
      })
      .catch((cause: unknown) => setBlockError(privacyErrorMessage(cause)))
      .finally(() => setBlockBusy(false));
  };

  const blockItem: ContextMenuEntry[] = [];
  if (menu && isDirectChat(menu.chat)) {
    const target = menu.chat;
    blockItem.push({
      id: "block",
      label: "Block contact",
      icon: Ban,
      danger: true,
      onSelect: () => {
        setBlockError(null);
        setBlockTarget(target);
      },
    });
  }

  // Business profiles only exist for user JIDs; groups get labels only.
  const businessItem: ContextMenuEntry[] = [];
  if (menu && isDirectChat(menu.chat)) {
    const target = menu.chat;
    businessItem.push({
      id: "business",
      label: "Business info",
      icon: Briefcase,
      onSelect: () => setBusinessChat(target),
    });
  }

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
        ...businessItem,
        {
          id: "label",
          label: "Label chat…",
          icon: Tag,
          onSelect: () => {
            setLabelChat({ chat: menu.chat, x: menu.x, y: menu.y });
          },
        },
        { kind: "separator", id: "separator" },
        ...blockItem,
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

      {notice ? (
        <p
          className={`chat-list-notice${
            notice.kind === "error" ? " error" : ""
          }`}
        >
          {notice.text}
        </p>
      ) : null}

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

        {trimmedQuery.length >= 2 ? (
          <div className="search-results">
            <p className="search-results-title">Messages</p>
            {searching ? (
              <p className="search-results-note">Searching…</p>
            ) : searchError ? (
              <p className="search-results-error">{searchError}</p>
            ) : hits.length === 0 ? (
              hitQuery === trimmedQuery ? (
                <p className="search-results-note">No messages found</p>
              ) : null
            ) : (
              hits.map((hit) => {
                const name = chatNames.get(hit.chatId) ?? hit.chatId;
                return (
                  <button
                    key={hit.id}
                    type="button"
                    className="search-hit"
                    onClick={() => onSelect(hit.chatId)}
                  >
                    <span className="search-hit-top">
                      <span className="search-hit-chat">{name}</span>
                      <span className="search-hit-time">
                        {formatListTime(hit.timestamp)}
                      </span>
                    </span>
                    <span className="search-hit-text">
                      {hit.text ? (
                        <HighlightedText text={hit.text} query={trimmedQuery} />
                      ) : (
                        KIND_LABELS[hit.kind]
                      )}
                    </span>
                  </button>
                );
              })
            )}
          </div>
        ) : null}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={() => setMenu(null)}
        />
      )}

      {labelChat ? (
        <LabelsMenu
          chatId={labelChat.chat.id}
          chatName={labelChat.chat.name}
          x={labelChat.x}
          y={labelChat.y}
          onClose={() => setLabelChat(null)}
        />
      ) : null}

      {businessChat ? (
        <BusinessProfilePanel
          chat={businessChat}
          onClose={() => setBusinessChat(null)}
        />
      ) : null}

      <ConfirmDialog
        open={blockTarget !== null}
        title={blockTarget ? `Block ${blockTarget.name}?` : "Block contact?"}
        body="Blocked contacts can't call you or send you messages. They also can't see your last seen, profile photo, or about. You can unblock them later in Settings."
        confirmLabel="Block"
        danger
        busy={blockBusy}
        error={blockError}
        onCancel={() => {
          if (blockBusy) return;
          setBlockTarget(null);
          setBlockError(null);
        }}
        onConfirm={confirmBlock}
      />

      {newChatOpen && <NewChatModal onClose={() => setNewChatOpen(false)} />}
    </section>
  );
}

/** Global message search: `search_messages` in Tauri, the store in mock mode. */
async function searchMessages(
  query: string,
  limit: number,
): Promise<Message[]> {
  if (isTauri()) {
    return invokeCore<Message[]>("search_messages", { query, limit });
  }
  const needle = query.toLowerCase();
  return Object.values(useAppStore.getState().messages)
    .flat()
    .filter((message) => message.text?.toLowerCase().includes(needle))
    .sort((a, b) => b.timestamp - a.timestamp)
    .slice(0, limit);
}

/** Quiet, row-level copy for a failed message search. */
function searchErrorMessage(cause: unknown): string {
  const raw =
    cause instanceof Error
      ? cause.message
      : typeof cause === "string"
        ? cause
        : "";
  if (/not connected|not linked|not paired|disconnected/i.test(raw)) {
    return "Search is unavailable while the app isn't connected.";
  }
  if (
    /not implemented|unknown command|command .* not found|unrecognized|not compiled/i.test(
      raw,
    )
  ) {
    return "Message search isn't available in this build yet.";
  }
  return raw || "Search failed. Try again.";
}

/** Renders `text` with the first case-insensitive match of `query` marked. */
function HighlightedText({ text, query }: { text: string; query: string }) {
  const index = text.toLowerCase().indexOf(query.toLowerCase());
  if (index < 0) return <>{text}</>;
  const end = index + query.length;
  return (
    <>
      {text.slice(0, index)}
      <mark>{text.slice(index, end)}</mark>
      {text.slice(end)}
    </>
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
