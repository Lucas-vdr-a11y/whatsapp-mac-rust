/** Bridges core events from the Rust host into the UI store. */

import { useCallback, useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeCore, isTauri, listenCore } from "./ipc";
import type { ChatSummary, CoreEvent } from "./types";
import { useAppStore, type CallUpdate } from "../store/app";

/** How long an ended/missed card stays up before the overlay is dismissed. */
const CALL_CLEAR_DELAY_MS = 2000;

/**
 * The core's `call` event is feature-gated on the Rust side (`calls` cargo
 * feature), so it is not yet part of the static [`CoreEvent`] union. Its shape
 * mirrors `CoreEvent::Call(CallUpdate)` serialized by serde:
 * `{ type: "call", payload: <CallUpdate> }`.
 */
interface CallCoreEvent {
  type: "call";
  payload: CallUpdate;
}

export function useCoreBridge(): void {
  const appendMessage = useAppStore((state) => state.appendMessage);
  const setConnection = useAppStore((state) => state.setConnection);
  const setQrCode = useAppStore((state) => state.setQrCode);
  const setPairingExpired = useAppStore((state) => state.setPairingExpired);
  const setPairCode = useAppStore((state) => state.setPairCode);
  const markPaired = useAppStore((state) => state.markPaired);
  const setChats = useAppStore((state) => state.setChats);
  const setMessageStatus = useAppStore((state) => state.setMessageStatus);
  const setCall = useAppStore((state) => state.setCall);
  const clearCall = useAppStore((state) => state.clearCall);
  const hydrateTimer = useRef<number | null>(null);
  const callClearTimer = useRef<number | null>(null);

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
      // Asserted to the wider union: the `call` variant is feature-gated on
      // the Rust side and not in the static `CoreEvent` union yet.
      const coreEvent = event as CoreEvent | CallCoreEvent;
      switch (coreEvent.type) {
        case "connection":
          setConnection(coreEvent.payload.state);
          if (coreEvent.payload.state === "connected") {
            // Reconnects (existing session) also imply a usable client.
            markPaired();
            void hydrateChats();
          }
          break;

        case "pairing": {
          const payload = coreEvent.payload;
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
            .setChatTyping(
              coreEvent.payload.chatId,
              coreEvent.payload.isTyping,
            );
          break;

        case "messageStatusChanged":
          setMessageStatus(
            coreEvent.payload.chatId,
            coreEvent.payload.messageId,
            coreEvent.payload.status,
          );
          break;

        case "reaction":
          useAppStore
            .getState()
            .applyCoreReaction(
              coreEvent.payload.messageId,
              coreEvent.payload.reactor,
              coreEvent.payload.emoji,
            );
          break;

        case "messageEdited":
          useAppStore
            .getState()
            .applyCoreEdit(
              coreEvent.payload.chatId,
              coreEvent.payload.messageId,
              coreEvent.payload.text,
            );
          break;

        case "messageRevoked":
          useAppStore
            .getState()
            .applyCoreRevoke(
              coreEvent.payload.chatId,
              coreEvent.payload.messageId,
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
          const message = coreEvent.payload;
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

        case "call": {
          const update = coreEvent.payload;

          // `setCall` also records the history row synchronously, so a
          // terminal update can be cleared without losing the entry.
          setCall(update);

          if (callClearTimer.current !== null) {
            window.clearTimeout(callClearTimer.current);
            callClearTimer.current = null;
          }

          if (
            update.type === "ended" ||
            update.type === "missed" ||
            update.type === "endedElsewhere"
          ) {
            const { callId } = update.payload;
            callClearTimer.current = window.setTimeout(() => {
              callClearTimer.current = null;
              clearCall(callId);
            }, CALL_CLEAR_DELAY_MS);
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
      if (callClearTimer.current !== null) {
        window.clearTimeout(callClearTimer.current);
        callClearTimer.current = null;
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
    setCall,
    clearCall,
    hydrateChats,
  ]);
}
