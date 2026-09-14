import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import {
  Archive,
  ExternalLink,
  Ban,
  Bell,
  Briefcase,
  PinOff,
  Tag,
  Trash2,
} from "lucide-react";
import { invokeCore, isTauri } from "../lib/ipc";
import { t, useTranslation } from "../lib/i18n";
import { formatListTime } from "../lib/time";
import { avatarSrc } from "../lib/avatar";
import { initials } from "../lib/names";
import type { ChatSummary, Jid, Message } from "../lib/types";
import { useAppStore } from "../store/app";
import { BusinessProfilePanel } from "./business/BusinessProfilePanel";
import { LabelsMenu } from "./business/LabelsMenu";
import { ContextMenu, type ContextMenuEntry } from "./ContextMenu";
import { ConfirmDialog } from "./settings/ConfirmDialog";
import { blockContact, privacyErrorMessage } from "./settings/privacy";
import { NewChatModal } from "./NewChatModal";
import { BellOff, CheckCheck, ComposeGlyph, Pin, Search } from "./icons";

/** Direct chats can be blocked; groups, communities and newsletters cannot. */
function isDirectChat(chat: ChatSummary): boolean {
  return (
    !chat.isGroup &&
    !chat.id.endsWith("@newsletter") &&
    !chat.id.endsWith("@broadcast")
  );
}

/** Translation keys for search hits whose message carries no text body. */
const KIND_LABEL_KEYS: Record<Message["kind"], string> = {
  text: "media.message",
  image: "media.photo",
  video: "media.video",
  audio: "media.audio",
  voiceNote: "media.voice",
  document: "media.document",
  sticker: "media.sticker",
  gif: "media.gif",
  location: "media.location",
  contact: "media.contact",
  poll: "media.poll",
  system: "media.system",
  unsupported: "media.unsupported",
};

interface ChatListProps {
  chats: ChatSummary[];
  selectedId: Jid | null;
  onSelect: (id: Jid) => void;
  /** When true the list shows archived chats (opened from the rail). */
  archivedView?: boolean;
}


/** Localized chat-list preview: media kinds render as labels, text as-is.
 *
 * Group previews are not prefixed with the sender name: the core's
 * `ChatSummary` has no last-message sender field yet. Add the prefix here once
 * `list_chats` carries one. */
function localizedPreview(
  chat: ChatSummary,
  translate: (key: string) => string,
): string {
  const kind = chat.lastMessageKind;
  const labels: Partial<Record<string, string>> = {
    image: "media.photo",
    video: "media.video",
    voiceNote: "media.voice",
    audio: "media.audio",
    document: "media.document",
    sticker: "media.sticker",
    gif: "media.gif",
    location: "media.location",
    contact: "media.contact",
    poll: "media.poll",
    unsupported: "media.message",
  };
  const key = kind ? labels[kind] : undefined;
  if (key) return translate(key);
  return chat.lastMessagePreview ?? translate("chats.noMessagesYet");
}

export function ChatList({
  chats,
  selectedId,
  onSelect,
  archivedView = false,
}: ChatListProps) {
  const { t } = useTranslation();
  const query = useAppStore((state) => state.query);
  const setQuery = useAppStore((state) => state.setQuery);
  const filter = useAppStore((state) => state.filter);
  const setFilter = useAppStore((state) => state.setFilter);
  const togglePinned = useAppStore((state) => state.togglePinned);
  const toggleMuted = useAppStore((state) => state.toggleMuted);
  const archiveChat = useAppStore((state) => state.archiveChat);
  const unarchiveChat = useAppStore((state) => state.unarchiveChat);
  const markRead = useAppStore((state) => state.markRead);
  const deleteChat = useAppStore((state) => state.deleteChat);

  const showArchived = archivedView;

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

  // Archived chats stay out of the main list. Pinned first, then newest,
  // mirroring the store's ordering; the query filters archived names only.
  const archivedAll = useMemo(
    () => allChats.filter((chat) => chat.isArchived),
    [allChats],
  );
  const archivedChats = useMemo(() => {
    const needle = trimmedQuery.toLowerCase();
    return archivedAll
      .filter((chat) => !needle || chat.name.toLowerCase().includes(needle))
      .sort((a, b) => {
        if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
        return b.lastActivityTs - a.lastActivityTs;
      });
  }, [archivedAll, trimmedQuery]);

  const listedChats = showArchived ? archivedChats : chats;

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
        setNotice({
          kind: "ok",
          text: t("chats.blockedNotice", { name: blockTarget.name }),
        });
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
      label: t("chats.menu.block"),
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
      label: t("chats.menu.businessInfo"),
      icon: Briefcase,
      onSelect: () => setBusinessChat(target),
    });
  }

  const menuItems: ContextMenuEntry[] = menu
    ? [
        {
          id: "pin",
          label: menu.chat.pinned
            ? t("chats.menu.unpin")
            : t("chats.menu.pin"),
          icon: menu.chat.pinned ? PinOff : Pin,
          onSelect: () => togglePinned(menu.chat.id),
        },
        {
          id: "mute",
          label: menu.chat.muted
            ? t("chats.menu.unmute")
            : t("chats.menu.mute"),
          icon: menu.chat.muted ? Bell : BellOff,
          onSelect: () => toggleMuted(menu.chat.id),
        },
        {
          id: "archive",
          label: menu.chat.isArchived
            ? t("chats.menu.unarchive")
            : t("chats.menu.archive"),
          icon: Archive,
          onSelect: () =>
            menu.chat.isArchived
              ? unarchiveChat(menu.chat.id)
              : archiveChat(menu.chat.id),
        },
        {
          id: "read",
          label: t("chats.menu.markRead"),
          icon: CheckCheck,
          disabled: menu.chat.unreadCount === 0,
          onSelect: () => markRead(menu.chat.id),
        },
        {
          id: "new-window",
          label: t("chats.menu.openInNewWindow"),
          icon: ExternalLink,
          onSelect: () => {
            void invokeCore("open_chat_window", { chatId: menu.chat.id });
          },
        },
        ...businessItem,
        {
          id: "label",
          label: t("chats.menu.label"),
          icon: Tag,
          onSelect: () => {
            setLabelChat({ chat: menu.chat, x: menu.x, y: menu.y });
          },
        },
        { kind: "separator", id: "separator" },
        ...blockItem,
        {
          id: "delete",
          label: t("chats.menu.delete"),
          icon: Trash2,
          danger: true,
          onSelect: () => deleteChat(menu.chat.id),
        },
      ]
    : [];

  return (
    <section className="chat-list">
      <header className="chat-list-header" data-tauri-drag-region>
        <h1 className="chat-list-title" data-tauri-drag-region>
          {showArchived ? t("chats.archived") : t("chats.title")}
        </h1>
        {!showArchived && (
          <div className="header-actions no-drag">
            <button
              type="button"
              className="icon-button"
              title={t("chats.newChat")}
              onClick={() => setNewChatOpen(true)}
            >
              <ComposeGlyph size={19} />
            </button>
          </div>
        )}
      </header>

      <div className="search-row">
        <label className="search-box">
          <Search size={17} />
          <input
            type="text"
            placeholder={t("chats.searchPlaceholder")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
      </div>

      {!showArchived && (
        <div className="filters no-drag" role="tablist">
          {(
            [
              { id: "all", label: t("chats.filterAll") },
              { id: "unread", label: t("chats.filterUnread") },
              { id: "favorites", label: t("chats.filterFavorites") },
              { id: "groups", label: t("chats.filterGroups") },
            ] as const
          ).map((entry) => (
            <button
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={filter === entry.id}
              className={`filter-pill${filter === entry.id ? " active" : ""}`}
              onClick={() => setFilter(entry.id)}
            >
              {entry.label}
            </button>
          ))}
        </div>
      )}

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
        {listedChats.map((chat) => (
          <ChatListItem
            key={chat.id}
            chat={chat}
            selected={chat.id === selectedId}
            onSelect={() => onSelect(chat.id)}
            onContextMenu={(event) => openMenu(event, chat)}
          />
        ))}
        {listedChats.length === 0 && (
          <p
            style={{
              padding: "32px 24px",
              color: "var(--text-secondary)",
              textAlign: "center",
            }}
          >
            {showArchived && !trimmedQuery
              ? t("chats.archivedEmpty")
              : t("chats.empty")}
          </p>
        )}

        {!showArchived && trimmedQuery.length >= 2 ? (
          <div className="search-results">
            <p className="search-results-title">
              {t("chats.searchResultsTitle")}
            </p>
            {searching ? (
              <p className="search-results-note">{t("chats.searching")}</p>
            ) : searchError ? (
              <p className="search-results-error">{searchError}</p>
            ) : hits.length === 0 ? (
              hitQuery === trimmedQuery ? (
                <p className="search-results-note">
                  {t("chats.noMessagesFound")}
                </p>
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
                        t(KIND_LABEL_KEYS[hit.kind])
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
        title={
          blockTarget
            ? t("chats.blockConfirmTitle", { name: blockTarget.name })
            : t("chats.blockConfirmFallback")
        }
        body={t("chats.blockConfirmBody")}
        confirmLabel={t("common.block")}
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
    return t("chats.searchUnavailable");
  }
  if (
    /not implemented|unknown command|command .* not found|unrecognized|not compiled/i.test(
      raw,
    )
  ) {
    return t("chats.searchUnavailableBuild");
  }
  return raw || t("chats.searchFailed");
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
  const { t } = useTranslation();
  const isOwnLastMessage =
    chat.lastFromMe ??
    (chat.lastMessagePreview?.startsWith("You:") ?? false);

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
                style={{
                  // Blue only once the last message was read; grey while it
                  // is merely delivered, exactly like the official client.
                  color:
                    chat.lastStatus === "read" || chat.lastStatus === "played"
                      ? "var(--tick-read)"
                      : "var(--text-secondary)",
                  flex: "none",
                }}
              />
            )}
            <span className="preview-text">
              {localizedPreview(chat, t)}
            </span>
          </span>
          <span className="chat-item-icons">
            {chat.muted && <BellOff size={16} aria-label={t("chats.muted")} />}
            {chat.pinned && <Pin size={16} aria-label={t("chats.pinned")} />}
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
