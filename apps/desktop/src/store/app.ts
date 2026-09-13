/** Application store. The single source of truth for the UI. */

import { create } from "zustand";
import { invokeCore, isTauri } from "../lib/ipc";
import type {
  ChatSummary,
  ConnectionState,
  Jid,
  Message,
  MessageKind,
  MessageStatus,
} from "../lib/types";
import { MOCK_CHATS, MOCK_MESSAGES } from "../mocks/data";

export type ChatFilter = "all" | "unread" | "groups";

/** Media metadata as reported by the core's `media_download` command. */
export interface MediaInfo {
  path: string;
  mime: string;
  fileName: string;
  size: number;
}

/** Quoted context: either the composer's reply target or a locally captured
 * quote for one of our outgoing messages. */
export interface MessageQuote {
  chatId: Jid;
  messageId: string;
  preview: string;
  senderName?: string;
}

interface AppState {
  chats: ChatSummary[];
  /** Messages per chat id, oldest first. */
  messages: Record<Jid, Message[]>;
  /** Chats whose history has been fetched from the core. */
  loadedChatIds: Record<Jid, boolean>;
  selectedChatId: Jid | null;
  query: string;
  filter: ChatFilter;

  /** Connection state reported by the core. */
  connection: ConnectionState;
  /** True once the device is linked and usable. */
  paired: boolean;
  /** Current QR payload while pairing, if any. */
  qrCode: string | null;
  /** Seconds until the current QR payload is rotated, when known. */
  qrTimeoutSecs: number | null;
  /** True when the server's QR rotation budget ran out. */
  pairingExpired: boolean;
  /** Alternative 8-character pairing code. */
  pairCode: string | null;

  selectChat: (id: Jid) => void;
  setQuery: (query: string) => void;
  setFilter: (filter: ChatFilter) => void;
  appendMessage: (message: Message) => void;
  sendText: (chatId: Jid, text: string) => void;

  /** Message being replied to in the composer, if any. */
  replyTo: MessageQuote | null;
  /** Set (or clear) the active reply target. */
  setReplyTo: (reply: MessageQuote | null) => void;
  /** Quoted context captured locally for outgoing messages by message id.
   * `Message` carries no quote field yet, so server-echoed quotes cannot be
   * decoded; this map only backs our own local echo. */
  quotes: Record<string, MessageQuote>;

  /** Local file path per message id once media has been resolved. */
  mediaPaths: Record<string, string>;
  /** Media metadata per message id once media has been resolved. */
  mediaMeta: Record<string, MediaInfo>;
  /** Resolve (and cache) the local file for a message (`media_download`). */
  downloadMedia: (messageId: string, kind: MessageKind) => Promise<MediaInfo>;

  /** Reaction counts per message id, emoji -> count. */
  reactions: Record<string, Record<string, number>>;
  /** Our own current reaction per message id. Optimistic until the protocol
   * exposes incoming reaction summaries. */
  myReactions: Record<string, string | null>;
  /** Toggle our reaction; picking the emoji we already used clears it. */
  toggleReaction: (
    chatId: Jid,
    messageId: string,
    emoji: string,
    fromMe: boolean,
  ) => void;

  /** Locally revoked (deleted) message ids. */
  deletedMessages: Record<string, true>;
  /** Revoke a message for everyone or just for this device (`actions_revoke`). */
  revokeMessage: (chatId: Jid, messageId: string, forEveryone: boolean) => void;

  /** Starred message ids. */
  starred: Record<string, true>;
  /** Toggle the starred state of a message (`actions_star`). */
  toggleStar: (chatId: Jid, messageId: string, fromMe: boolean) => void;

  /** Rewrite one of our own text messages (`actions_edit`). */
  editMessage: (chatId: Jid, messageId: string, text: string) => void;

  /** Local, in-memory chat-list mutations (persistence lands with the core). */
  togglePinned: (id: Jid) => void;
  toggleMuted: (id: Jid) => void;
  archiveChat: (id: Jid) => void;
  markRead: (id: Jid) => void;
  deleteChat: (id: Jid) => void;
  /** Selects an existing chat or creates an empty one for a new contact. */
  startChat: (id: Jid, name: string, isGroup?: boolean) => void;

  setConnection: (connection: ConnectionState) => void;
  setQrCode: (code: string | null, timeoutSecs?: number) => void;
  setPairingExpired: (expired: boolean) => void;
  setPairCode: (code: string | null) => void;
  /** Mark the session usable; `jid` is present on a fresh pairing. */
  markPaired: (jid?: Jid) => void;

  /** Replace the chat list with the core's view. */
  setChats: (chats: ChatSummary[]) => void;
  /** Replace one chat's history with the core's view. */
  setMessages: (chatId: Jid, messages: Message[]) => void;
  /** Fetch a chat's history from the core, once. */
  loadMessages: (chatId: Jid) => void;
  /** Update the delivery state of one message. */
  setMessageStatus: (
    chatId: Jid,
    messageId: string,
    status: MessageStatus,
  ) => void;
  /** Typing indicators by chat, as reported by the protocol. */
  typingByChat: Record<Jid, boolean>;
  /** Avatar URLs or local paths by chat, when resolved. */
  avatars: Record<Jid, string>;
  /** Update the received typing state for a chat. */
  setChatTyping: (chatId: Jid, isTyping: boolean) => void;
  /** Tell the protocol that we started or stopped typing. */
  sendTyping: (chatId: Jid, typing: boolean) => void;
  /** Cache an avatar reference for a chat. */
  setAvatar: (chatId: Jid, avatar: string) => void;
  /** Best-effort avatar fetch, once per chat. */
  loadAvatar: (chatId: Jid) => void;
}

/** In a plain browser we run on mock data; inside Tauri the core fills state. */
const mockMode = !isTauri();

export const useAppStore = create<AppState>((set, get) => {
  /** Applies a patch to a single chat, leaving the rest of the list alone. */
  const patchChat = (
    id: Jid,
    patch: (chat: ChatSummary) => Partial<ChatSummary>,
  ) =>
    set((state) => ({
      chats: state.chats.map((chat) =>
        chat.id === id ? { ...chat, ...patch(chat) } : chat,
      ),
    }));

  return {
    chats: mockMode ? MOCK_CHATS : [],
    messages: mockMode ? MOCK_MESSAGES : {},
    loadedChatIds: {},
    selectedChatId: null,
    query: "",
    filter: "all",

    connection: mockMode ? "connected" : "disconnected",
    paired: mockMode,
    qrCode: null,
    qrTimeoutSecs: null,
    pairingExpired: false,
    pairCode: null,

    replyTo: null,
    quotes: {},
    mediaPaths: {},
    mediaMeta: {},
    reactions: {},
    myReactions: {},
    deletedMessages: {},
    starred: {},

    selectChat: (id) => set({ selectedChatId: id }),
    setQuery: (query) => set({ query }),
    setFilter: (filter) => set({ filter }),

    appendMessage: (message) =>
      set((state) => {
        const existing = state.messages[message.chatId] ?? [];
        if (existing.some((m) => m.id === message.id)) return state;
        return {
          messages: {
            ...state.messages,
            [message.chatId]: [...existing, message],
          },
        };
      }),

    sendText: (chatId, text) => {
      const trimmed = text.trim();
      if (!trimmed) return;

      const replyTo = get().replyTo;
      const activeReply = replyTo && replyTo.chatId === chatId ? replyTo : null;

      // Optimistic local echo, reconciled with the stored message when the
      // core acknowledges the send.
      const optimistic: Message = {
        id: `local-${crypto.randomUUID()}`,
        chatId,
        senderId: "me",
        fromMe: true,
        timestamp: Math.floor(Date.now() / 1000),
        kind: "text",
        text: trimmed,
        status: "pending",
      };
      get().appendMessage(optimistic);

      // `Message` has no quote field yet: remember the quoted preview locally
      // so the echo can render it, and clear the composer's reply target.
      set((state) => ({
        quotes: activeReply
          ? { ...state.quotes, [optimistic.id]: activeReply }
          : state.quotes,
        replyTo: activeReply ? null : state.replyTo,
      }));

      if (!isTauri()) return;

      const args: Record<string, unknown> = { chatId, text: trimmed };
      if (activeReply) args.quotedMessageId = activeReply.messageId;

      void invokeCore<Message>(
        activeReply ? "actions_send_quoting" : "send_text",
        args,
      )
        .then((stored) => {
          set((state) => {
            // Move the locally captured quote over to the stored message id.
            const quotes = { ...state.quotes };
            const localQuote = quotes[optimistic.id];
            if (localQuote) {
              quotes[stored.id] = localQuote;
              delete quotes[optimistic.id];
            }
            return {
              quotes,
              messages: {
                ...state.messages,
                [chatId]: (state.messages[chatId] ?? []).map((message) =>
                  message.id === optimistic.id ? stored : message,
                ),
              },
            };
          });
        })
        .catch((error) => {
          console.error("send message failed", error);
          set((state) => ({
            messages: {
              ...state.messages,
              [chatId]: (state.messages[chatId] ?? []).map((message) =>
                message.id === optimistic.id
                  ? { ...message, status: "failed" as const }
                  : message,
              ),
            },
          }));
        });
    },

    setReplyTo: (reply) => set({ replyTo: reply }),

    downloadMedia: async (messageId, kind) => {
      const cached = get().mediaMeta[messageId];
      if (cached) return cached;

      if (!isTauri()) {
        // Browser mock mode: synthesize a small inline preview so that media
        // bubbles can be exercised without the Rust host. Kinds we cannot
        // fake surface a retry state in the UI instead.
        const mock = mockMediaInfo(messageId, kind);
        if (!mock) {
          throw new Error("Media is only available in the desktop app");
        }
        set((state) => ({
          mediaPaths: { ...state.mediaPaths, [messageId]: mock.path },
          mediaMeta: { ...state.mediaMeta, [messageId]: mock },
        }));
        return mock;
      }

      const info = await invokeCore<MediaInfo>("media_download", { messageId });
      set((state) => ({
        mediaPaths: { ...state.mediaPaths, [messageId]: info.path },
        mediaMeta: { ...state.mediaMeta, [messageId]: info },
      }));
      return info;
    },

    toggleReaction: (chatId, messageId, emoji, fromMe) => {
      const state = get();
      const previous = state.myReactions[messageId] ?? null;
      const clearing = previous === emoji;
      const counts = { ...(state.reactions[messageId] ?? {}) };

      const adjust = (target: string, delta: number) => {
        const next = (counts[target] ?? 0) + delta;
        if (next > 0) counts[target] = next;
        else delete counts[target];
      };

      if (clearing) {
        adjust(emoji, -1);
      } else {
        if (previous) adjust(previous, -1);
        adjust(emoji, 1);
      }

      set((current) => ({
        reactions: { ...current.reactions, [messageId]: counts },
        myReactions: {
          ...current.myReactions,
          [messageId]: clearing ? null : emoji,
        },
      }));

      if (!isTauri()) return;
      // An empty emoji clears our reaction per the IPC contract.
      void invokeCore("actions_react", {
        chatId,
        messageId,
        emoji: clearing ? "" : emoji,
        fromMe,
      }).catch((error) => console.error("actions_react failed", error));
    },

    revokeMessage: (chatId, messageId, forEveryone) => {
      set((state) => ({
        deletedMessages: { ...state.deletedMessages, [messageId]: true },
      }));
      if (!isTauri()) return;
      void invokeCore("actions_revoke", {
        chatId,
        messageId,
        forEveryone,
      }).catch((error) => console.error("actions_revoke failed", error));
    },

    toggleStar: (chatId, messageId, fromMe) => {
      const star = !get().starred[messageId];
      set((state) => {
        const starred = { ...state.starred };
        if (star) starred[messageId] = true;
        else delete starred[messageId];
        return { starred };
      });
      if (!isTauri()) return;
      void invokeCore("actions_star", {
        chatId,
        messageId,
        fromMe,
        star,
      }).catch((error) => console.error("actions_star failed", error));
    },

    editMessage: (chatId, messageId, text) => {
      const trimmed = text.trim();
      if (!trimmed) return;

      const previous =
        get().messages[chatId]?.find((message) => message.id === messageId)
          ?.text ?? null;

      set((state) => ({
        messages: {
          ...state.messages,
          [chatId]: (state.messages[chatId] ?? []).map((message) =>
            message.id === messageId ? { ...message, text: trimmed } : message,
          ),
        },
      }));

      if (!isTauri()) return;
      void invokeCore("actions_edit", {
        chatId,
        messageId,
        text: trimmed,
      }).catch((error) => {
        console.error("actions_edit failed", error);
        // Put the original text back rather than leave a lie on screen.
        set((state) => ({
          messages: {
            ...state.messages,
            [chatId]: (state.messages[chatId] ?? []).map((message) =>
              message.id === messageId
                ? { ...message, text: previous }
                : message,
            ),
          },
        }));
      });
    },

    togglePinned: (id) => {
      const chat = get().chats.find((candidate) => candidate.id === id);
      if (!chat) return;
      const pinned = !chat.pinned;
      patchChat(id, () => ({ pinned }));
      if (isTauri()) {
        void invokeCore("set_chat_pinned", { chatId: id, pinned }).catch(
          (error) => console.error("set_chat_pinned failed", error),
        );
      }
    },

    toggleMuted: (id) => {
      const chat = get().chats.find((candidate) => candidate.id === id);
      if (!chat) return;
      const muted = !chat.muted;
      patchChat(id, () => ({ muted }));
      if (isTauri()) {
        void invokeCore("set_chat_muted", { chatId: id, muted }).catch(
          (error) => console.error("set_chat_muted failed", error),
        );
      }
    },

    archiveChat: (id) => {
      patchChat(id, () => ({ isArchived: true }));
      if (isTauri()) {
        void invokeCore("set_chat_archived", {
          chatId: id,
          archived: true,
        }).catch((error) => console.error("set_chat_archived failed", error));
      }
    },

    markRead: (id) => {
      patchChat(id, () => ({ unreadCount: 0 }));
      if (isTauri()) {
        void invokeCore("mark_chat_read", { chatId: id }).catch((error) =>
          console.error("mark_chat_read failed", error),
        );
      }
    },

    deleteChat: (id) =>
      set((state) => {
        const messages = { ...state.messages };
        delete messages[id];
        return {
          chats: state.chats.filter((chat) => chat.id !== id),
          messages,
          selectedChatId:
            state.selectedChatId === id ? null : state.selectedChatId,
        };
      }),

    startChat: (id, name, isGroup = false) =>
      set((state) => {
        if (state.chats.some((chat) => chat.id === id)) {
          return { selectedChatId: id };
        }

        const chat: ChatSummary = {
          id,
          name,
          lastMessagePreview: null,
          lastActivityTs: Math.floor(Date.now() / 1000),
          unreadCount: 0,
          muted: false,
          pinned: false,
          isGroup,
          isArchived: false,
        };
        return { chats: [chat, ...state.chats], selectedChatId: id };
      }),

    setConnection: (connection) => set({ connection }),
    setQrCode: (qrCode, timeoutSecs) =>
      set({
        qrCode,
        qrTimeoutSecs: timeoutSecs ?? null,
        pairingExpired: false,
      }),
    setPairingExpired: (expired) =>
      set({
        pairingExpired: expired,
        qrCode: null,
        qrTimeoutSecs: null,
      }),
    setPairCode: (pairCode) => set({ pairCode }),
    markPaired: () =>
      set({
        paired: true,
        qrCode: null,
        qrTimeoutSecs: null,
        pairCode: null,
        pairingExpired: false,
      }),

    setChats: (chats) => set({ chats }),

    setMessages: (chatId, messages) =>
      set((state) => ({
        messages: { ...state.messages, [chatId]: messages },
        loadedChatIds: { ...state.loadedChatIds, [chatId]: true },
      })),

    loadMessages: (chatId) => {
      if (!isTauri()) return;
      if (get().loadedChatIds[chatId]) return;

      void invokeCore<Message[]>("list_messages", { chatId, limit: 200 })
        .then((messages) => get().setMessages(chatId, messages))
        .catch((error) => console.error("list_messages failed", error));
    },

    setMessageStatus: (chatId, messageId, status) =>
      set((state) => {
        const messages = state.messages[chatId];
        if (!messages) return state;
        return {
          messages: {
            ...state.messages,
            [chatId]: messages.map((message) =>
              message.id === messageId ? { ...message, status } : message,
            ),
          },
        };
      }),

    typingByChat: {},
    avatars: {},

    setChatTyping: (chatId, isTyping) =>
      set((state) => ({
        typingByChat: { ...state.typingByChat, [chatId]: isTyping },
      })),

    sendTyping: (chatId, typing) => {
      if (!isTauri()) return;
      void invokeCore("set_typing", { chatId, typing }).catch((error) =>
        console.error("set_typing failed", error),
      );
    },

    setAvatar: (chatId, avatar) =>
      set((state) => ({
        avatars: { ...state.avatars, [chatId]: avatar },
      })),

    loadAvatar: (chatId) => {
      if (!isTauri()) return;
      if (get().avatars[chatId]) return;
      void invokeCore<string | null>("contacts_avatar", { jid: chatId })
        .then((url) => {
          if (url) get().setAvatar(chatId, url);
        })
        .catch(() => {
          // Avatar resolution is best-effort and may be unavailable while
          // the contact commands are being wired up.
        });
    },
  };
});

/** Browser preview stand-in for `media_download`. Returns an inline SVG only
 * for still-image kinds; animated video/audio remain unavailable in the
 * browser and are reported through the UI's retry state. */
function mockMediaInfo(messageId: string, kind: MessageKind): MediaInfo | null {
  if (kind !== "image" && kind !== "sticker" && kind !== "gif") return null;

  let hash = 0;
  for (let index = 0; index < messageId.length; index += 1) {
    hash = (hash * 31 + messageId.charCodeAt(index)) | 0;
  }
  const hue = Math.abs(hash) % 360;
  const title = kind === "sticker" ? "Sticker" : "Photo";
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="420" viewBox="0 0 640 420">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="hsl(${hue} 55% 42%)"/>` +
    `<stop offset="1" stop-color="hsl(${(hue + 60) % 360} 55% 22%)"/>` +
    `</linearGradient></defs>` +
    `<rect width="640" height="420" fill="url(#g)"/>` +
    `<text x="320" y="214" font-family="-apple-system, sans-serif" font-size="26" ` +
    `fill="rgba(255,255,255,0.88)" text-anchor="middle">${title} preview</text>` +
    `<text x="320" y="248" font-family="-apple-system, sans-serif" font-size="15" ` +
    `fill="rgba(255,255,255,0.6)" text-anchor="middle">browser mock mode</text></svg>`;

  return {
    path: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`,
    mime: "image/svg+xml",
    fileName: `${kind}-preview.svg`,
    size: svg.length,
  };
}

/** Chats after applying the search query and the active filter. */
export function selectVisibleChats(state: AppState): ChatSummary[] {
  const query = state.query.trim().toLowerCase();

  return state.chats
    .filter((chat) => {
      if (chat.isArchived) return false;
      if (state.filter === "unread" && chat.unreadCount === 0) return false;
      if (state.filter === "groups" && !chat.isGroup) return false;
      if (query && !chat.name.toLowerCase().includes(query)) return false;
      return true;
    })
    .sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.lastActivityTs - a.lastActivityTs;
    });
}
