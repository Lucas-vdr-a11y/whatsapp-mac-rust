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

export type ChatFilter = "all" | "unread" | "favorites" | "groups";

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

/** Lifecycle stage the calls UI renders. `idle` means no overlay. */
export type CallUiState =
  | "idle"
  | "ringing-in"
  | "ringing-out"
  | "connecting"
  | "active"
  | "ended";

/** Snapshot of one call (`whatsapp_core::calls::manager::CallInfo`, camelCase
 * over IPC). `callId` is absent on the optimistic outgoing echo, before the
 * core has assigned one. */
export interface CallInfo {
  callId?: string;
  chatId: Jid;
  video: boolean;
  /** Unix seconds when the call was placed or started ringing. */
  timestamp: number;
}

/** Core call phase (`whatsapp_core::calls::manager::CallState`). */
export type CoreCallState =
  | "ringing"
  | "calling"
  | "connecting"
  | "active"
  | "ended"
  | "failed";

/** Payload of the core's `call` event (`calls::manager::CallUpdate`,
 * serde-tagged with `type` / `payload` and camelCase fields). */
export type CallUpdate =
  | { type: "ringing"; payload: CallInfo }
  | { type: "missed"; payload: CallInfo }
  | {
      type: "endedElsewhere";
      payload: {
        callId: string;
        chatId: Jid;
        accepted: boolean;
        timestamp: number;
      };
    }
  | {
      type: "phase";
      payload: { callId: string; state: CoreCallState; reason: string | null };
    }
  | { type: "ended"; payload: { callId: string } };

/** One row in the session's call history. Phone-synced history lands later. */
export interface CallHistoryEntry {
  /** Server call id when known; synthetic for local-only entries. */
  id: string;
  chatId: Jid;
  direction: "incoming" | "outgoing" | "missed";
  video: boolean;
  /** Unix seconds. */
  startedAt: number;
  durationSecs: number | null;
}

/** The call currently shown in the overlay. */
export interface CallSession {
  callId?: string;
  chatId?: Jid;
  info?: CallInfo;
  state: CallUiState;
  muted: boolean;
  /** User-facing detail: "Missed call", a failure reason, ended-elsewhere. */
  reason?: string;
  /** Which side placed the call; drives the history direction. */
  direction?: "incoming" | "outgoing";
  /** Unix seconds when the call was placed or started ringing. */
  startedAt?: number;
  /** Unix seconds when media went live; backs the duration timer. */
  activeSince?: number;
}

interface AppState {
  chats: ChatSummary[];
  /** Messages per chat id, oldest first. */
  messages: Record<Jid, Message[]>;
  /** Chats whose history has been fetched from the core. */
  loadedChatIds: Record<Jid, boolean>;
  /** Chats whose local history has been walked back to the start. */
  olderExhausted: Record<Jid, boolean>;
  /** Chats with an in-flight older-message page load. */
  loadingOlder: Record<Jid, boolean>;
  /** Chats where we already asked the phone for more history. */
  olderRequested: Record<Jid, boolean>;
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
  /** Send a text message, optionally @-mentioning participant JIDs
   * (`chat_send_mentions`, falling back to `send_text` on older builds). */
  sendText: (chatId: Jid, text: string, mentions?: Jid[]) => void;

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
  /** Who reacted with what, per message id (from core reaction events). */
  reactionActors: Record<string, Record<string, string>>;
  /** Toggle our reaction; picking the emoji we already used clears it. */
  toggleReaction: (
    chatId: Jid,
    messageId: string,
    emoji: string,
    fromMe: boolean,
  ) => void;
  /** Apply a reaction that arrived from the protocol. */
  applyCoreReaction: (messageId: string, reactor: string, emoji: string) => void;
  /** Apply an edit that arrived from the protocol. */
  applyCoreEdit: (chatId: Jid, messageId: string, text: string) => void;
  /** Apply a revoke that arrived from the protocol. */
  applyCoreRevoke: (chatId: Jid, messageId: string) => void;

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
  unarchiveChat: (id: Jid) => void;
  markRead: (id: Jid) => void;
  deleteChat: (id: Jid) => void;
  /** Clear a chat's messages locally and via `chat_clear`; rejects on IPC error. */
  clearChat: (id: Jid) => Promise<void>;
  /** Selects an existing chat or creates an empty one for a new contact. */
  startChat: (id: Jid, name: string, isGroup?: boolean) => void;

  setConnection: (connection: ConnectionState) => void;
  setQrCode: (code: string | null, timeoutSecs?: number) => void;
  setPairingExpired: (expired: boolean) => void;
  setPairCode: (code: string | null) => void;
  /** Mark the session usable; `jid` is present on a fresh pairing. */
  markPaired: (jid?: Jid) => void;

  /** App lock overlay is showing. */
  locked: boolean;
  /** Show or hide the lock overlay; locking closes the open conversation. */
  setLocked: (locked: boolean) => void;
  /** Replace the chat list with the core's view. */
  setChats: (chats: ChatSummary[]) => void;
  /** Display names by JID, harvested from `list_chats` (direct chats only).
   * The mention menu uses these to label group senders. */
  contactNames: Record<Jid, string>;
  /** Replace one chat's history with the core's view. */
  setMessages: (chatId: Jid, messages: Message[]) => void;
  /** Fetch a chat's history from the core, once. */
  loadMessages: (chatId: Jid) => void;
  /** Re-read a chat from the store, even if it was already loaded. */
  reloadMessages: (chatId: Jid) => void;
  /** Load one page of older messages from the local store (scroll-up). */
  loadOlderMessages: (chatId: Jid) => Promise<void>;
  /** Ask the primary phone for older messages not present locally. */
  fetchOlderHistory: (chatId: Jid) => Promise<void>;
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
  /** Online/last-seen presence per contact. */
  presenceByChat: Record<Jid, { online: boolean; lastSeenTs: number | null }>;
  /** Apply a presence update from the protocol. */
  setPresence: (jid: Jid, online: boolean, lastSeenTs: number | null) => void;
  /** Update the received typing state for a chat. */
  setChatTyping: (chatId: Jid, isTyping: boolean) => void;
  /** Tell the protocol that we started or stopped typing. */
  sendTyping: (chatId: Jid, typing: boolean) => void;
  /** Cache an avatar reference for a chat. */
  setAvatar: (chatId: Jid, avatar: string) => void;
  /** Best-effort avatar fetch, once per chat. */
  loadAvatar: (chatId: Jid) => void;

  /** Live call state; `idle` means no overlay is shown. */
  callState: CallSession;
  /** Calls from this session, newest first. */
  callHistory: CallHistoryEntry[];
  /** Route a core `call` event into the overlay (records history). */
  setCall: (update: CallUpdate) => void;
  /** Dismiss the overlay, optionally only when it still shows `callId`. */
  clearCall: (callId?: string) => void;
  /** Toggle the local microphone (`calls_mute`). */
  setCallMuted: (muted: boolean) => void;
  /** Place an outgoing voice/video call (`calls_start`). */
  startCall: (chatId: Jid, video: boolean) => void;
  /** Hang up or cancel the call with `chatId` (`calls_end`). */
  endCall: (chatId: Jid) => void;
  /** Accept a ringing incoming call (`calls_answer`). */
  answerCall: (callId: string) => void;
  /** Decline a ringing incoming call (`calls_reject`). */
  rejectCall: (callId: string) => void;
}

/** In a plain browser we run on mock data; inside Tauri the core fills state. */
const mockMode = !isTauri();

/** Shared "no call in progress" session; never mutated in place. */
const IDLE_CALL: CallSession = { state: "idle", muted: false };

/** Copy for the reality of this build: calling needs the media backend. */
const CALLS_UNAVAILABLE = "Calling isn't available yet on this build/device.";

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
    contactNames: mockMode ? contactNamesFromChats(MOCK_CHATS) : {},
    loadedChatIds: {},
    olderExhausted: {},
    loadingOlder: {},
    olderRequested: {},
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
    reactionActors: {},
    deletedMessages: {},
    starred: {},
    callState: IDLE_CALL,
    callHistory: [],

    selectChat: (id) => set({ selectedChatId: id }),

    setLocked: (locked) =>
      set((state) => ({
        locked,
        selectedChatId: locked ? null : state.selectedChatId,
      })),
    setQuery: (query) => set({ query }),
    setFilter: (filter) => set({ filter }),

    appendMessage: (message) =>
      set((state) => {
        const existing = state.messages[message.chatId] ?? [];
        if (existing.some((m) => m.id === message.id)) return state;
        return applyMessageToChatList(
          {
            ...state,
            messages: {
              ...state.messages,
              [message.chatId]: [...existing, message],
            },
          },
          message,
        );
      }),

    sendText: (chatId, text, mentions) => {
      const trimmed = text.trim();
      if (!trimmed) return;

      const replyTo = get().replyTo;
      const activeReply = replyTo && replyTo.chatId === chatId ? replyTo : null;
      const mentionedJids = dedupeJids(mentions);

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

      /** Swap the optimistic bubble for the stored row once the core acks. */
      const reconcile = (stored: Message) => {
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
      };

      const markFailed = (error: unknown) => {
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
      };

      // The quoting command has no mention parameter, so when a reply and
      // mentions are both active the quote wins and the mentions are dropped.
      if (activeReply) {
        void invokeCore<Message>("actions_send_quoting", args)
          .then(reconcile)
          .catch(markFailed);
        return;
      }

      if (mentionedJids.length === 0) {
        void invokeCore<Message>("send_text", args)
          .then(reconcile)
          .catch(markFailed);
        return;
      }

      void invokeCore<Message>("chat_send_mentions", {
        chatId,
        text: trimmed,
        mentions: mentionedJids,
      })
        .then(reconcile)
        .catch((error) => {
          // Older core builds lack the command; retry as a plain text send so
          // the message still goes out (without mention metadata).
          if (isUnknownCommand(error)) {
            void invokeCore<Message>("send_text", args)
              .then(reconcile)
              .catch(markFailed);
            return;
          }
          markFailed(error);
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

    applyCoreReaction: (messageId, reactor, emoji) =>
      set((state) => {
        const actors = { ...(state.reactionActors[messageId] ?? {}) };
        if (emoji) actors[reactor] = emoji;
        else delete actors[reactor];

        const counts: Record<string, number> = {};
        for (const value of Object.values(actors)) {
          counts[value] = (counts[value] ?? 0) + 1;
        }
        return {
          reactionActors: { ...state.reactionActors, [messageId]: actors },
          reactions: { ...state.reactions, [messageId]: counts },
        };
      }),

    applyCoreEdit: (chatId, messageId, text) =>
      set((state) => {
        const messages = state.messages[chatId];
        if (!messages) return state;
        return {
          messages: {
            ...state.messages,
            [chatId]: messages.map((message) =>
              message.id === messageId
                ? { ...message, text, kind: "text" as const }
                : message,
            ),
          },
        };
      }),

    applyCoreRevoke: (_chatId, messageId) =>
      set((state) => ({
        deletedMessages: { ...state.deletedMessages, [messageId]: true },
      })),

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

    unarchiveChat: (id) => {
      patchChat(id, () => ({ isArchived: false }));
      if (isTauri()) {
        void invokeCore("set_chat_archived", {
          chatId: id,
          archived: false,
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

    clearChat: async (id) => {
      // The core clears the account-side history (`chat_clear`); local rows are
      // dropped here because the core's store has no clear helper yet.
      if (isTauri()) {
        await invokeCore("chat_clear", { chatId: id });
      }
      set((state) => {
        const messages = { ...state.messages };
        delete messages[id];
        return { messages };
      });
    },

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
        return {
          chats: [chat, ...state.chats],
          contactNames: isGroup
            ? state.contactNames
            : { ...state.contactNames, [id]: name },
          selectedChatId: id,
        };
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

    setChats: (chats) =>
      set((state) => ({
        chats,
        // Keep the latest `list_chats` names without dropping earlier ones.
        contactNames: {
          ...state.contactNames,
          ...contactNamesFromChats(chats),
        },
      })),

    setMessages: (chatId, messages) =>
      set((state) => ({
        messages: { ...state.messages, [chatId]: messages },
        loadedChatIds: { ...state.loadedChatIds, [chatId]: true },
      })),

    loadMessages: (chatId) => {
      if (!isTauri()) return;
      if (get().loadedChatIds[chatId]) return;
      get().reloadMessages(chatId);
    },

    reloadMessages: (chatId) => {
      if (!isTauri()) return;
      void invokeCore<Message[]>("list_messages", { chatId, limit: 200 })
        .then((incoming) => {
          set((state) => {
            const existing = state.messages[chatId] ?? [];
            const optimistic = existing.filter((message) =>
              message.id.startsWith("local-"),
            );
            const known = new Set(incoming.map((message) => message.id));
            return {
              messages: {
                ...state.messages,
                [chatId]: [
                  ...incoming,
                  ...optimistic.filter((message) => !known.has(message.id)),
                ],
              },
              loadedChatIds: { ...state.loadedChatIds, [chatId]: true },
            };
          });
        })
        .catch((error) => console.error("list_messages failed", error));
    },

    loadOlderMessages: async (chatId) => {
      if (!isTauri()) return;
      const state = get();
      if (state.loadingOlder[chatId] || state.olderExhausted[chatId]) return;
      const messages = state.messages[chatId];
      if (!messages || messages.length === 0) return;

      const beforeId = messages[0].id;
      set((current) => ({
        loadingOlder: { ...current.loadingOlder, [chatId]: true },
      }));
      try {
        const older = await invokeCore<Message[]>("list_messages_before", {
          chatId,
          beforeId,
          limit: 100,
        });
        if (older.length === 0) {
          set((current) => ({
            olderExhausted: { ...current.olderExhausted, [chatId]: true },
          }));
          return;
        }
        set((current) => {
          const existing = current.messages[chatId] ?? [];
          const known = new Set(existing.map((message) => message.id));
          const fresh = older.filter((message) => !known.has(message.id));
          if (fresh.length === 0) return current;
          return {
            messages: { ...current.messages, [chatId]: [...fresh, ...existing] },
          };
        });
      } catch (error) {
        console.error("list_messages_before failed", error);
      } finally {
        set((current) => ({
          loadingOlder: { ...current.loadingOlder, [chatId]: false },
        }));
      }
    },

    fetchOlderHistory: async (chatId) => {
      if (!isTauri()) return;
      const state = get();
      if (state.olderRequested[chatId]) return;
      set((current) => ({
        olderRequested: { ...current.olderRequested, [chatId]: true },
      }));

      try {
        await invokeCore<boolean>("fetch_older_history", {
          chatId,
          count: 100,
        });
      } catch (error) {
        // Older builds do not expose the command; keep the local-only view.
        console.warn("fetch_older_history failed", error);
        return;
      }

      // The phone answers asynchronously with a history-sync chunk. Give the
      // core a few chances to store it, then walk one more page into view.
      for (const delay of [2500, 5000, 10000]) {
        await new Promise((resolve) => window.setTimeout(resolve, delay));
        set((current) => ({
          olderExhausted: { ...current.olderExhausted, [chatId]: false },
        }));
        await get().loadOlderMessages(chatId);
        if (!get().olderExhausted[chatId]) return;
      }
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
    presenceByChat: {},
    locked: false,

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

    setPresence: (jid, online, lastSeenTs) =>
      set((state) => ({
        presenceByChat: {
          ...state.presenceByChat,
          [jid]: { online, lastSeenTs },
        },
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

    setCall: (update) =>
      set((state) => {
        const current = state.callState;

        switch (update.type) {
          case "ringing": {
            const info = update.payload;
            return {
              callState: {
                callId: info.callId,
                chatId: info.chatId,
                info,
                state: "ringing-in",
                muted: false,
                direction: "incoming",
                startedAt: info.timestamp,
              } satisfies CallSession,
            };
          }

          case "phase": {
            const { callId, state: phase, reason } = update.payload;
            // Ignore phases once the overlay has moved on.
            if (current.state === "idle") return state;
            if (current.callId && current.callId !== callId) return state;

            switch (phase) {
              case "calling":
                return {
                  callState: {
                    ...current,
                    callId,
                    state: "ringing-out",
                    direction: current.direction ?? "outgoing",
                    reason: undefined,
                  },
                };
              case "connecting":
                return {
                  callState: { ...current, callId, state: "connecting" },
                };
              case "active":
                return {
                  callState: {
                    ...current,
                    callId,
                    state: "active",
                    activeSince: Math.floor(Date.now() / 1000),
                    reason: undefined,
                  },
                };
              case "failed": {
                const session: CallSession = {
                  ...current,
                  callId,
                  state: "ended",
                  reason: reason ?? "Call failed",
                };
                return {
                  callState: session,
                  callHistory: appendHistory(
                    state.callHistory,
                    sessionHistoryEntry(session, false),
                  ),
                };
              }
              case "ended": {
                const session: CallSession = { ...current, callId, state: "ended" };
                return {
                  callState: session,
                  callHistory: appendHistory(
                    state.callHistory,
                    sessionHistoryEntry(session, false),
                  ),
                };
              }
              default:
                return state;
            }
          }

          case "missed": {
            const info = update.payload;
            const session: CallSession = {
              callId: info.callId,
              chatId: info.chatId,
              info,
              state: "ended",
              muted: false,
              direction: "incoming",
              startedAt: info.timestamp,
              reason: "Missed call",
            };
            return {
              callState: session,
              callHistory: appendHistory(
                state.callHistory,
                sessionHistoryEntry(session, true),
              ),
            };
          }

          case "endedElsewhere": {
            const { callId, chatId, accepted, timestamp } = update.payload;
            const session: CallSession = {
              ...current,
              callId,
              chatId: current.chatId ?? chatId,
              state: "ended",
              direction: current.direction ?? "incoming",
              startedAt: current.startedAt ?? timestamp,
              reason: accepted
                ? "Answered on another device"
                : "Declined on another device",
            };
            return {
              callState: session,
              callHistory: appendHistory(
                state.callHistory,
                sessionHistoryEntry(session, !accepted),
              ),
            };
          }

          case "ended": {
            const { callId } = update.payload;
            if (current.state === "idle") return state;
            if (current.callId && current.callId !== callId) return state;
            const session: CallSession = { ...current, callId, state: "ended" };
            return {
              callState: session,
              callHistory: appendHistory(
                state.callHistory,
                sessionHistoryEntry(session, false),
              ),
            };
          }
        }
      }),

    clearCall: (callId) =>
      set((state) => {
        if (callId && state.callState.callId && state.callState.callId !== callId) {
          return state;
        }
        return { callState: IDLE_CALL };
      }),

    setCallMuted: (muted) => {
      const session = get().callState;
      if (!session.chatId) return;
      const { chatId, callId } = session;

      set({ callState: { ...session, muted } });

      if (!isTauri()) return;
      void invokeCore("calls_mute", { chatId, muted }).catch((error) => {
        console.error("calls_mute failed", error);
        set((state) => {
          // Only revert when the same call is still on screen.
          if (
            state.callState.chatId !== chatId ||
            state.callState.callId !== callId
          ) {
            return state;
          }
          return {
            callState: {
              ...state.callState,
              muted: !muted,
              reason: callErrorMessage(error),
            },
          };
        });
      });
    },

    startCall: (chatId, video) => {
      const session = get().callState;
      if (session.state !== "idle" && session.state !== "ended") return;

      const now = Math.floor(Date.now() / 1000);
      const info: CallInfo = { chatId, video, timestamp: now };
      set({
        callState: {
          chatId,
          info,
          state: "ringing-out",
          muted: false,
          direction: "outgoing",
          startedAt: now,
        },
      });

      if (!isTauri()) {
        // Browser mock mode: exercise the overlay with the same friendly
        // failure a build without the media backend reports.
        set((state) => ({
          callState: {
            ...state.callState,
            state: "ended",
            reason: CALLS_UNAVAILABLE,
          },
        }));
        return;
      }

      void invokeCore("calls_start", { chatId, video }).catch((error) => {
        console.error("calls_start failed", error);
        set((state) => {
          if (
            state.callState.chatId !== chatId ||
            state.callState.direction !== "outgoing"
          ) {
            return state;
          }
          return {
            callState: {
              ...state.callState,
              state: "ended",
              reason: callErrorMessage(error),
            },
          };
        });
      });
    },

    endCall: (chatId) => {
      const session = get().callState;
      if (session.chatId !== chatId) return;

      if (!isTauri()) {
        get().clearCall();
        return;
      }

      void invokeCore("calls_end", { chatId }).catch((error) => {
        console.error("calls_end failed", error);
        set((state) => {
          if (state.callState.chatId !== chatId) return state;
          return {
            callState: {
              ...state.callState,
              state: "ended",
              reason: callErrorMessage(error),
            },
          };
        });
      });
    },

    answerCall: (callId) => {
      const session = get().callState;
      if (session.state !== "ringing-in" || session.callId !== callId) return;

      set({ callState: { ...session, state: "connecting" } });

      if (!isTauri()) {
        set((state) => ({
          callState: {
            ...state.callState,
            state: "ended",
            reason: CALLS_UNAVAILABLE,
          },
        }));
        return;
      }

      void invokeCore("calls_answer", { callId }).catch((error) => {
        console.error("calls_answer failed", error);
        set((state) => {
          if (state.callState.callId !== callId) return state;
          return {
            callState: {
              ...state.callState,
              state: "ended",
              reason: callErrorMessage(error),
            },
          };
        });
      });
    },

    rejectCall: (callId) => {
      const session = get().callState;
      if (session.callId !== callId) return;

      /** Declining leaves a red "missed" row, matching WhatsApp. */
      const dismiss = () =>
        set((state) => {
          if (state.callState.callId !== callId) return state;
          return {
            callHistory: appendHistory(
              state.callHistory,
              sessionHistoryEntry(state.callState, true),
            ),
            callState: IDLE_CALL,
          };
        });

      if (!isTauri()) {
        dismiss();
        return;
      }

      void invokeCore("calls_reject", { callId })
        .then(dismiss)
        .catch((error) => {
          console.error("calls_reject failed", error);
          set((state) => {
            if (state.callState.callId !== callId) return state;
            return {
              callState: {
                ...state.callState,
                state: "ended",
                reason: callErrorMessage(error),
              },
            };
          });
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

/** Normalize an IPC failure (a Tauri error string or an Error) for the UI. */
function callErrorMessage(error: unknown): string {
  const raw =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  const lower = raw.toLowerCase();
  if (
    lower.includes("not compiled") ||
    lower.includes("not connected") ||
    lower.includes("media backend") ||
    lower.includes("voip")
  ) {
    return CALLS_UNAVAILABLE;
  }
  return raw || CALLS_UNAVAILABLE;
}

/** History row for a session that just terminated. */
function sessionHistoryEntry(
  session: CallSession,
  missed: boolean,
): CallHistoryEntry | null {
  if (!session.chatId) return null;
  const now = Math.floor(Date.now() / 1000);
  return {
    id: session.callId ?? `local-${Date.now()}`,
    chatId: session.chatId,
    direction: missed ? "missed" : (session.direction ?? "outgoing"),
    video: session.info?.video ?? false,
    startedAt: session.startedAt ?? now,
    durationSecs:
      session.activeSince !== undefined
        ? Math.max(0, now - session.activeSince)
        : null,
  };
}

/** Prepend `entry` unless the same call is already in the list. */
function appendHistory(
  history: CallHistoryEntry[],
  entry: CallHistoryEntry | null,
): CallHistoryEntry[] {
  if (!entry) return history;
  if (history.some((item) => item.id === entry.id)) return history;
  return [entry, ...history];
}

/** Display names of direct chats from a `list_chats` payload. */
function contactNamesFromChats(chats: ChatSummary[]): Record<Jid, string> {
  const names: Record<Jid, string> = {};
  for (const chat of chats) {
    if (!chat.isGroup) names[chat.id] = chat.name;
  }
  return names;
}

/** Keep the chat-list preview, sort key and unread in lockstep with a live
 * message so the row does not stay stale until the next `list_chats`. */
function applyMessageToChatList<
  T extends { chats: ChatSummary[]; selectedChatId: Jid | null },
>(state: T, message: Message): T {
  const viewing =
    state.selectedChatId === message.chatId &&
    typeof document !== "undefined" &&
    document.visibilityState === "visible";
  const unreadBump = message.fromMe || viewing ? 0 : 1;
  const index = state.chats.findIndex((chat) => chat.id === message.chatId);
  const previous = index >= 0 ? state.chats[index] : null;
  const next: ChatSummary = {
    id: message.chatId,
    name: previous?.name ?? message.chatId.split("@")[0] ?? message.chatId,
    lastMessagePreview: message.text ?? previous?.lastMessagePreview ?? null,
    lastMessageKind: message.kind,
    lastFromMe: message.fromMe,
    lastStatus: message.fromMe ? message.status : (previous?.lastStatus ?? null),
    lastActivityTs: Math.max(previous?.lastActivityTs ?? 0, message.timestamp),
    unreadCount: (previous?.unreadCount ?? 0) + unreadBump,
    muted: previous?.muted ?? false,
    pinned: previous?.pinned ?? false,
    isGroup: previous?.isGroup ?? message.chatId.endsWith("@g.us"),
    isArchived: previous?.isArchived ?? false,
  };
  const rest =
    index >= 0
      ? state.chats.filter((chat) => chat.id !== message.chatId)
      : state.chats;
  return { ...state, chats: [next, ...rest] };
}

/** Stable order for the mention JIDs handed to the core. */
function dedupeJids(jids: Jid[] | undefined): Jid[] {
  return jids ? [...new Set(jids)] : [];
}

/** True when the core rejected a call because the command does not exist yet. */
function isUnknownCommand(error: unknown): boolean {
  const raw =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  return /unknown command|command .* not found|unrecognized command/i.test(raw);
}

/** Chats after applying the search query and the active filter. */
export function selectVisibleChats(state: AppState): ChatSummary[] {
  const query = state.query.trim().toLowerCase();

  return state.chats
    .filter((chat) => {
      if (chat.isArchived) return false;
      if (state.filter === "unread" && chat.unreadCount === 0) return false;
      if (state.filter === "favorites" && !chat.pinned) return false;
      if (state.filter === "groups" && !chat.isGroup) return false;
      if (query && !chat.name.toLowerCase().includes(query)) return false;
      return true;
    })
    .sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
      return b.lastActivityTs - a.lastActivityTs;
    });
}
