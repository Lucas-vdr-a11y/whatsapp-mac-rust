/** Application store. The single source of truth for the UI. */

import { create } from "zustand";
import { isTauri } from "../lib/ipc";
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
  selectedChatId: Jid | null;
  query: string;
  filter: ChatFilter;

  /** Connection state reported by the core. */
  connection: ConnectionState;
  /** True once the device is linked and usable. */
  paired: boolean;
  /** Current QR payload while pairing, if any. */
  qrCode: string | null;
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
  setQrCode: (code: string | null) => void;
  setPairCode: (code: string | null) => void;
  markPaired: (jid: Jid) => void;
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
    selectedChatId: null,
    query: "",
    filter: "all",

    connection: mockMode ? "connected" : "disconnected",
    paired: mockMode,
    qrCode: null,
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

      // Optimistic local echo. TODO(M1): forward to the core via
      // `invokeCore("send_text", { chatId, text })` and reconcile the real id.
      get().appendMessage({
        id: `local-${crypto.randomUUID()}`,
        chatId,
        senderId: "me",
        fromMe: true,
        timestamp: Math.floor(Date.now() / 1000),
        kind: "text",
        text: trimmed,
        status: "pending",
      });
    },

    togglePinned: (id) => patchChat(id, (chat) => ({ pinned: !chat.pinned })),
    toggleMuted: (id) => patchChat(id, (chat) => ({ muted: !chat.muted })),
    archiveChat: (id) => patchChat(id, () => ({ isArchived: true })),
    markRead: (id) => patchChat(id, () => ({ unreadCount: 0 })),

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
    setQrCode: (qrCode) => set({ qrCode }),
    setPairCode: (pairCode) => set({ pairCode }),
    markPaired: () => set({ paired: true, qrCode: null, pairCode: null }),
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
