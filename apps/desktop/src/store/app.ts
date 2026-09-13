/** Application store. The single source of truth for the UI. */

import { create } from "zustand";
import { invokeCore, isTauri } from "../lib/ipc";
import type {
  ChatSummary,
  ConnectionState,
  Jid,
  Message,
} from "../lib/types";
import { MOCK_CHATS, MOCK_MESSAGES } from "../mocks/data";

export type ChatFilter = "all" | "unread" | "groups";

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
  /** Typing indicators by chat, as reported by the protocol. */
  typingByChat: Record<Jid, boolean>;
  /** Update the received typing state for a chat. */
  setChatTyping: (chatId: Jid, isTyping: boolean) => void;
  /** Tell the protocol that we started or stopped typing. */
  sendTyping: (chatId: Jid, typing: boolean) => void;
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

      if (!isTauri()) return;

      void invokeCore<Message>("send_text", { chatId, text: trimmed })
        .then((stored) => {
          set((state) => ({
            messages: {
              ...state.messages,
              [chatId]: (state.messages[chatId] ?? []).map((message) =>
                message.id === optimistic.id ? stored : message,
              ),
            },
          }));
        })
        .catch((error) => {
          console.error("send_text failed", error);
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

    typingByChat: {},

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
  };
});

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
