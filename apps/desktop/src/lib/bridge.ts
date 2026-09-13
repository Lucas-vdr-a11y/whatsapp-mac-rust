/** Bridges core events from the Rust host into the UI store. */

import { useCallback, useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeCore, isTauri, listenCore } from "./ipc";
import type { ChatSummary } from "./types";
import { useAppStore } from "../store/app";

export function useCoreBridge(): void {
  const appendMessage = useAppStore((state) => state.appendMessage);
  const setConnection = useAppStore((state) => state.setConnection);
  const setQrCode = useAppStore((state) => state.setQrCode);
  const setPairingExpired = useAppStore((state) => state.setPairingExpired);
  const setPairCode = useAppStore((state) => state.setPairCode);
  const markPaired = useAppStore((state) => state.markPaired);
  const setChats = useAppStore((state) => state.setChats);
  const setMessageStatus = useAppStore((state) => state.setMessageStatus);
  const hydrateTimer = useRef<number | null>(null);

  const hydrateChats = useCallback(async () => {
    if (!isTauri()) return;
    try {
      setChats(await invokeCore<ChatSummary[]>("list_chats"));
    } catch (error) {
      console.warn("failed to load chats", error);
    }
  }, [setChats]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let unlistenOpenChat: UnlistenFn | null = null;
    let cancelled = false;

    // A session that survives a restart reconnects without a QR scan; the
    // chat list may already be on disk.
    void hydrateChats();

    /** Open a chat from a deep link, creating it locally when unknown. */
    const openChat = (chatId: string) => {
      const store = useAppStore.getState();
      if (store.chats.some((chat) => chat.id === chatId)) {
        store.selectChat(chatId);
      } else {
        store.startChat(chatId, chatId.split("@")[0] ?? chatId);
      }
    };

    // `rustwa://chat/<jid>` deep links, delivered while the app runs.
    void listen<{ chatId: string }>("ui://open-chat", (event) => {
      const chatId = event.payload?.chatId;
      if (chatId) openChat(chatId);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenOpenChat = fn;
      }
    });

    // A deep link that arrived before the UI mounted (cold start).
    void invokeCore<string | null>("deep_link_ready")
      .then((chatId) => {
        if (chatId && !cancelled) openChat(chatId);
      })
      .catch(() => {
        // Deep links may be unavailable (older build, browser mode).
      });

    listenCore((event) => {
      switch (event.type) {
        case "connection":
          setConnection(event.payload.state);
          if (event.payload.state === "connected") {
            // Reconnects (existing session) also imply a usable client.
            markPaired();
            void hydrateChats();
          }
          break;

        case "pairing": {
          const payload = event.payload;
          switch (payload.kind) {
            case "qrCode":
              setQrCode(payload.code, payload.timeoutSecs);
              break;
            case "qrCodesExhausted":
              setPairingExpired(true);
              break;
            case "pairCode":
              setPairCode(payload.code);
              break;
            case "pairSuccess":
              markPaired(payload.jid);
              void hydrateChats();
              break;
            case "pairFailure":
              setPairingExpired(true);
              break;
          }
          break;
        }

        case "typing":
          useAppStore
            .getState()
            .setChatTyping(event.payload.chatId, event.payload.isTyping);
          break;

        case "messageStatusChanged":
          setMessageStatus(
            event.payload.chatId,
            event.payload.messageId,
            event.payload.status,
          );
          break;

        case "chatUpdated":
          // History sync emits bursts of these; debounce the rehydrate.
          if (hydrateTimer.current === null) {
            hydrateTimer.current = window.setTimeout(() => {
              hydrateTimer.current = null;
              void hydrateChats();
            }, 400);
          }
          break;

        case "message": {
          const message = event.payload;
          appendMessage(message);

          // Native notification for incoming messages while the window is
          // hidden. Best-effort: permission may still be pending.
          if (!message.fromMe && document.hidden) {
            const chat = useAppStore
              .getState()
              .chats.find((candidate) => candidate.id === message.chatId);
            const title = chat?.name ?? "New message";
            const body = message.text ?? "[Media]";
            void invokeCore("notify_for_chat", {
              chatId: message.chatId,
              title,
              body,
            }).catch(() => {
              // Fall back to the plain notification on older builds.
              void invokeCore("notify", { title, body }).catch(() => {
                // Notification delivery is not critical; ignore failures.
              });
            });
          }
          break;
        }

        // The remaining event types are wired up as their milestones land.
        default:
          break;
      }
    }).then((fn) => {
      if (cancelled) {
        fn?.();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
      unlistenOpenChat?.();
      if (hydrateTimer.current !== null) {
        window.clearTimeout(hydrateTimer.current);
        hydrateTimer.current = null;
      }
    };
  }, [
    appendMessage,
    setConnection,
    setQrCode,
    setPairingExpired,
    setPairCode,
    markPaired,
    setMessageStatus,
    hydrateChats,
  ]);
}
