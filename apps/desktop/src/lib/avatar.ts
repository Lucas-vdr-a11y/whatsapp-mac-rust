/** Avatar source helper: remote URLs pass through, local paths go through the
 * Tauri asset protocol so the webview may load them. */

import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauri } from "./ipc";

export function avatarSrc(value: string): string {
  if (
    value.startsWith("http://") ||
    value.startsWith("https://") ||
    value.startsWith("data:")
  ) {
    return value;
  }
  return isTauri() ? convertFileSrc(value) : value;
}
