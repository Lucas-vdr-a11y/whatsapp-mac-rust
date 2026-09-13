/** Bridges core events from the Rust host into the UI store. */

import { useEffect } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listenCore } from "./ipc";
import { useAppStore } from "../store/app";

export function useCoreBridge(): void {
  const appendMessage = useAppStore((state) => state.appendMessage);
  const setConnection = useAppStore((state) => state.setConnection);
  const setQrCode = useAppStore((state) => state.setQrCode);
  const setPairCode = useAppStore((state) => state.setPairCode);
  const markPaired = useAppStore((state) => state.markPaired);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    listenCore((event) => {
      switch (event.type) {
        case "connection":
          setConnection(event.payload.state);
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
              break;
            case "pairFailure":
              setQrCode(null);
              break;
          }
          break;
        }

        case "message":
          appendMessage(event.payload);
          break;

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
  }, [appendMessage, setConnection, setQrCode, setPairCode, markPaired]);
}
