/** Bridges core events from the Rust host into the UI store. */

import { useCallback, useEffect } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { invokeCore, isTauri, listenCore } from "./ipc";
import type { ChatSummary } from "./types";
import { useAppStore } from "../store/app";

export function useCoreBridge(): void {
  const appendMessage = useAppStore((state) => state.appendMessage);
  const setConnection = useAppStore((state) => state.setConnection);
  const setQrCode = useAppStore((state) => state.setQrCode);
  const setPairCode = useAppStore((state) => state.setPairCode);
  const markPaired = useAppStore((state) => state.markPaired);
  const setChats = useAppStore((state) => state.setChats);

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
    let cancelled = false;

    // A session that survives a restart reconnects without a QR scan; the
    // chat list may already be on disk.
    void hydrateChats();

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
              setQrCode(payload.code);
              break;
            case "pairCode":
              setPairCode(payload.code);
              break;
            case "pairSuccess":
              markPaired(payload.jid);
              void hydrateChats();
              break;
            case "pairFailure":
              setQrCode(null);
              break;
          }
          break;
        }

        case "typing":
          useAppStore
            .getState()
            .setChatTyping(event.payload.chatId, event.payload.isTyping);
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
            void invokeCore("notify", {
              title: chat?.name ?? "New message",
              body: message.text ?? "[Media]",
            }).catch(() => {
              // Notification delivery is not critical; ignore failures.
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
    };
  }, [
    appendMessage,
    setConnection,
    setQrCode,
    setPairCode,
    markPaired,
    hydrateChats,
  ]);
}
