/**
 * Dedicated single-conversation window shell.
 *
 * Loaded by the `open_chat_window` Rust command as
 * `index.html?window=chat&chatId=<percent-encoded jid>`; `main.tsx` routes
 * that query to this component instead of the full app.
 *
 * Unlike the main window, this shell does **not** run `useCoreBridge()`:
 * that hook also owns desktop notifications, deep-link delivery and the call
 * overlay, and running it in every webview would duplicate notifications and
 * could consume the cold-start deep link before the main window sees it.
 * Instead the window subscribes to `core://event` itself and applies only the
 * events for its own chat; notifications stay with the main window.
 *
 * Each webview owns its own zustand store, so `chats` starts empty and the
 * summary is hydrated once through `list_chats`. A JID that is not in the
 * store once hydration finished (deleted chat, stale link) gets a friendly
 * localized "chat not found" state with a way back to the main window.
 */

import { useEffect, useRef, useState, type ReactNode } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "../lib/i18n";
import { invokeCore, isTauri, listenCore } from "../lib/ipc";
import type { ChatSummary, Message } from "../lib/types";
import { useAppStore } from "../store/app";
import { Conversation } from "./Conversation";

interface ChatWindowProps {
  /** JID of the chat this window shows, from the `chatId` query parameter. */
  chatId: string;
}

export function ChatWindow({ chatId }: ChatWindowProps) {
  const { t } = useTranslation();

  const chat = useAppStore(
    (state) => state.chats.find((candidate) => candidate.id === chatId) ?? null,
  );
  const setChats = useAppStore((state) => state.setChats);
  const setMessages = useAppStore((state) => state.setMessages);
  const appendMessage = useAppStore((state) => state.appendMessage);
  const setMessageStatus = useAppStore((state) => state.setMessageStatus);
  const applyCoreReaction = useAppStore((state) => state.applyCoreReaction);
  const applyCoreEdit = useAppStore((state) => state.applyCoreEdit);
  const applyCoreRevoke = useAppStore((state) => state.applyCoreRevoke);
  const setChatTyping = useAppStore((state) => state.setChatTyping);
  const setPresence = useAppStore((state) => state.setPresence);

  // In browser mock mode the store already ships with chats and messages, so
  // both flags start settled.
  const [hydrated, setHydrated] = useState(!isTauri());
  const [messagesReady, setMessagesReady] = useState(!isTauri());
  const refreshTimer = useRef<number | null>(null);

  // Hydrate the chat summary once. An unknown `chatId` (chrome/first boot) is
  // reported as "not found" after this settles.
  useEffect(() => {
    if (!chatId || !isTauri()) return;
    let cancelled = false;
    void invokeCore<ChatSummary[]>("list_chats")
      .then((chats) => {
        if (!cancelled) setChats(chats);
      })
      .catch((error) => console.error("list_chats failed", error))
      .finally(() => {
        if (!cancelled) setHydrated(true);
      });
    return () => {
      cancelled = true;
    };
  }, [chatId, setChats]);

  // Fetch this chat's history. `setMessages` marks the chat loaded, so
  // `Conversation`'s own `loadMessages` call becomes a no-op: history is read
  // exactly once per window.
  useEffect(() => {
    if (!chatId || !isTauri()) return;
    let cancelled = false;
    void invokeCore<Message[]>("list_messages", { chatId, limit: 200 })
      .then((messages) => {
        if (!cancelled) setMessages(chatId, messages);
      })
      .catch((error) => {
        console.error("list_messages failed", error);
        // Settle on an empty history instead of leaving the shell loading
        // forever; the conversation then renders its own empty state.
        if (!cancelled) setMessages(chatId, []);
      })
      .finally(() => {
        if (!cancelled) setMessagesReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, [chatId, setMessages]);

  // Subscribe to core events directly and apply only the ones that concern
  // this chat. No notifications are posted here, so extra windows never
  // duplicate the main window's desktop notifications.
  useEffect(() => {
    if (!chatId || !isTauri()) return;
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    const refreshChats = () => {
      void invokeCore<ChatSummary[]>("list_chats")
        .then((chats) => useAppStore.getState().setChats(chats))
        .catch((error) => console.error("list_chats failed", error));
    };

    void listenCore((event) => {
      switch (event.type) {
        case "message":
          if (event.payload.chatId === chatId) appendMessage(event.payload);
          break;

        case "messageStatusChanged":
          if (event.payload.chatId === chatId) {
            setMessageStatus(
              event.payload.chatId,
              event.payload.messageId,
              event.payload.status,
            );
          }
          break;

        case "messageEdited":
          if (event.payload.chatId === chatId) {
            applyCoreEdit(
              event.payload.chatId,
              event.payload.messageId,
              event.payload.text,
            );
          }
          break;

        case "messageRevoked":
          if (event.payload.chatId === chatId) {
            applyCoreRevoke(event.payload.chatId, event.payload.messageId);
          }
          break;

        case "reaction":
          if (event.payload.chatId === chatId) {
            applyCoreReaction(
              event.payload.messageId,
              event.payload.reactor,
              event.payload.emoji,
            );
          }
          break;

        case "typing":
          if (event.payload.chatId === chatId) {
            setChatTyping(event.payload.chatId, event.payload.isTyping);
          }
          break;

        case "presence":
          if (event.payload.jid === chatId) {
            setPresence(
              event.payload.jid,
              event.payload.online,
              event.payload.lastSeenTs,
            );
          }
          break;

        case "chatUpdated":
          // The summary feeds the header (name, unread). History sync emits
          // bursts of these, so debounce the rehydrate like the main bridge.
          if (
            event.payload.chatId === chatId &&
            refreshTimer.current === null
          ) {
            refreshTimer.current = window.setTimeout(() => {
              refreshTimer.current = null;
              refreshChats();
            }, 400);
          }
          break;

        default:
          break;
      }
    }).then((fn) => {
      if (cancelled) fn?.();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
      if (refreshTimer.current !== null) {
        window.clearTimeout(refreshTimer.current);
        refreshTimer.current = null;
      }
    };
  }, [
    chatId,
    appendMessage,
    setMessageStatus,
    applyCoreEdit,
    applyCoreRevoke,
    applyCoreReaction,
    setChatTyping,
    setPresence,
  ]);

  if (!hydrated) {
    return (
      <ChatWindowShell chatId={chatId}>
        <div className="empty-conversation">
          <p>{t("chatWindow.loading")}</p>
        </div>
      </ChatWindowShell>
    );
  }

  if (!chat) {
    return (
      <ChatWindowShell chatId={chatId}>
        <div className="empty-conversation">
          <h2>{t("chatWindow.notFoundTitle")}</h2>
          <p>{t("chatWindow.notFoundBody")}</p>
        </div>
      </ChatWindowShell>
    );
  }

  return (
    <ChatWindowShell chatId={chatId}>
      {messagesReady ? (
        <Conversation chat={chat} />
      ) : (
        <div className="empty-conversation">
          <p>{t("chatWindow.loading")}</p>
        </div>
      )}
    </ChatWindowShell>
  );
}

interface ChatWindowShellProps {
  chatId: string;
  children: ReactNode;
}

/** Compact chrome around the conversation: a back-to-main affordance and a
 * full-height content slot. */
function ChatWindowShell({ chatId, children }: ChatWindowShellProps) {
  const { t } = useTranslation();

  const backToMain = () => {
    // Idempotent on the Rust side; closing this window hands focus back to
    // the main window through the regular AppKit window order.
    void invokeCore("close_chat_window", { chatId }).catch((error) =>
      console.error("close_chat_window failed", error),
    );
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        background: "var(--bg-chat)",
      }}
    >
      <header
        style={{
          flex: "none",
          display: "flex",
          alignItems: "center",
          gap: 4,
          height: 40,
          padding: "0 8px",
          background: "var(--bg-header)",
          borderBottom: "1px solid var(--divider)",
        }}
      >
        <button
          type="button"
          className="icon-button"
          style={{ width: 32, height: 32 }}
          title={t("chatWindow.back")}
          onClick={backToMain}
        >
          <ArrowLeft size={18} />
        </button>
        <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>
          {t("chatWindow.back")}
        </span>
      </header>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          display: "flex",
          flexDirection: "column",
        }}
      >
        {children}
      </div>
    </div>
  );
}
