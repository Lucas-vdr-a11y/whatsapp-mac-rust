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
