/** Shared helpers for media bubbles and message previews. */

import type { Message, MessageKind } from "../../lib/types";

/** Deterministic hash in [0, 1) from a string and a salt. */
function hashUnit(value: string, salt: number): number {
  let hash = (2166136261 ^ salt) >>> 0;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return hash / 4294967296;
}

/** Static waveform bar heights (0.18..1) derived deterministically from the
 * message id, so a voice note always draws the same shape. */
export function waveformBars(messageId: string, count = 34): number[] {
  return Array.from({ length: count }, (_, index) =>
    0.18 + hashUnit(messageId, index * 7919 + 17) * 0.82,
  );
}

/** Human-readable byte size, e.g. "1.4 MB". Empty string for unknown sizes. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const rounded =
    value >= 10 || unit === 0 ? Math.round(value) : value.toFixed(1);
  return `${rounded} ${units[unit]}`;
}

/** `m:ss` duration, e.g. "0:07". */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "--:--";
  const total = Math.round(seconds);
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

/** One-line preview used for reply bars and quoted context. */
export function messagePreview(message: Message, deleted = false): string {
  if (deleted) return "This message was deleted";
  const text = message.text?.trim();
  if (text) return text;
  return mediaLabel(message.kind);
}

/** True when the core marked this message as view-once. Older payloads omit the
 * field, so it is read structurally and defaults to false. */
export function isViewOnce(message: Message): boolean {
  return (message as Message & { viewOnce?: boolean }).viewOnce === true;
}

/** localStorage prefix for the per-message "already viewed" marker. */
const VIEW_ONCE_VIEWED_PREFIX = "rustwa.viewonce.";

/** True once this view-once message was opened. Persisted per message id so
 * the reveal cannot be replayed after a reload, matching WhatsApp. */
export function isViewOnceViewed(messageId: string): boolean {
  return readStorage(`${VIEW_ONCE_VIEWED_PREFIX}${messageId}`) === "1";
}

/** Persist the "already viewed" marker for a view-once message. */
export function markViewOnceViewed(messageId: string): void {
  writeStorage(`${VIEW_ONCE_VIEWED_PREFIX}${messageId}`, "1");
}

/** Auto-download preference shape stored in localStorage. */
export type AutoDownloadPolicy = "wifi" | "always" | "never";

/** Desktop default: media may be fetched whenever the user asks for it. */
export const DEFAULT_AUTO_DOWNLOAD_POLICY: AutoDownloadPolicy = "always";

/** localStorage key backing the policy (a future settings screen writes it). */
const AUTO_DOWNLOAD_KEY = "rustwa.autodownload";

/** Read the auto-download policy, tolerating a missing or corrupt value. */
export function readAutoDownloadPolicy(): AutoDownloadPolicy {
  const raw = readStorage(AUTO_DOWNLOAD_KEY);
  return raw === "wifi" || raw === "always" || raw === "never"
    ? raw
    : DEFAULT_AUTO_DOWNLOAD_POLICY;
}

/** True when the policy forbids automatic media fetches. Explicit user
 * downloads stay allowed; this only drives the placeholder hint today. */
export function isAutoDownloadDisabled(): boolean {
  return readAutoDownloadPolicy() === "never";
}

/** localStorage access that survives private mode / disabled storage. */
function readStorage(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Storage may be unavailable; the reveal state then lasts this session.
  }
}

/** Neutral fallback label for a message kind. */
export function mediaLabel(kind: MessageKind): string {
  switch (kind) {
    case "image":
      return "Photo";
    case "video":
      return "Video";
    case "voiceNote":
      return "Voice message";
    case "audio":
      return "Audio";
    case "document":
      return "Document";
    case "sticker":
      return "Sticker";
    case "gif":
      return "GIF";
    case "location":
      return "Location";
    case "contact":
      return "Contact";
    case "poll":
      return "Poll";
    case "system":
      return "System message";
    case "unsupported":
      return "Unsupported message";
    default:
      return "Message";
  }
}
