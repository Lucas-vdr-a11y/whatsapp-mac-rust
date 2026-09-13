/** Application store. The single source of truth for the UI. */

import { create } from "zustand";
import { isTauri } from "../lib/ipc";
import type { ChatSummary, Jid, Message } from "../lib/types";
import { MOCK_CHATS, MOCK_MESSAGES } from "../mocks/data";

export type ChatFilter = "all" | "unread" | "groups";

interface AppState {
  chats: ChatSummary[];
  /** Messages per chat id, oldest first. */
  messages: Record<Jid, Message[]>;
  selectedChatId: Jid | null;
  query: string;
  filter: ChatFilter;

  selectChat: (id: Jid) => void;
  setQuery: (query: string) => void;
  setFilter: (filter: ChatFilter) => void;
  appendMessage: (message: Message) => void;
  sendText: (chatId: Jid, text: string) => void;
}

/** In a plain browser we run on mock data; inside Tauri the core fills state. */
const mockMode = !isTauri();

export const useAppStore = create<AppState>((set, get) => ({
  chats: mockMode ? MOCK_CHATS : [],
  messages: mockMode ? MOCK_MESSAGES : {},
  selectedChatId: null,
  query: "",
  filter: "all",

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
}));

/** Chats after applying the search query and the active filter. */
export function selectVisibleChats(state: AppState): ChatSummary[] {
  const query = state.query.trim().toLowerCase();

  return state.chats
    .filter((chat) => {
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
