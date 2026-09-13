/** Bridges core events from the Rust host into the UI store. */

import { useEffect } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listenCore } from "./ipc";
import { useAppStore } from "../store/app";

export function useCoreBridge(): void {
  const appendMessage = useAppStore((state) => state.appendMessage);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    listenCore((event) => {
      switch (event.type) {
        case "message":
          appendMessage(event.payload);
          break;
        // Other event types are handled as their milestones land.
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
  }, [appendMessage]);
}
