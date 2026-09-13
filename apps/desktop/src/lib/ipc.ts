/** Typed wrappers around the Tauri IPC surface. */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CoreEvent } from "./types";

/** True when the UI runs inside the RustWA host rather than a plain browser. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Invoke a Rust command. Throws in browser (mock) mode. */
export async function invokeCore<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    throw new Error("IPC is unavailable outside the RustWA host");
  }
  return invoke<T>(command, args);
}

/** Subscribe to core events. Resolves to `null` in browser (mock) mode. */
export async function listenCore(
  handler: (event: CoreEvent) => void,
): Promise<UnlistenFn | null> {
  if (!isTauri()) return null;
  return listen<CoreEvent>("core://event", (event) => handler(event.payload));
}
