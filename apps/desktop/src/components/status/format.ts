/** Formatting helpers shared by the status list and the fullscreen viewer. */

import { t } from "../../lib/i18n";
import type { MessageKind } from "../../lib/types";
import type { StatusKind, StatusUpdate } from "./types";

/** 0xAARRGGBB background to a CSS color (alpha ignored; the viewer is dark). */
export function argbToCss(argb: number | null): string {
  const value = (argb ?? 0xff144d37) >>> 0;
  return `#${(value & 0xffffff).toString(16).padStart(6, "0")}`;
}

/** Placeholder copy for updates whose text is not available. */
export function updateLabel(kind: StatusKind): string {
  switch (kind) {
    case "image":
      return t("media.photo");
    case "video":
      return t("media.video");
    case "voice":
      return t("media.voice");
    default:
      return t("status.updateLabel");
  }
}

export function updatePreview(update: StatusUpdate): string {
  const text = update.text?.trim();
  return text ? text : updateLabel(update.kind);
}

/** "09:41" today, "Yesterday, 21:03" inside the 24 h window. */
export function statusTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const now = new Date();
  const time = date.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  return sameDay ? time : t("status.yesterday", { time });
}

/** "Just now", "9 min ago", "3 h ago"; older updates fall back to the clock. */
export function relativeTime(unixSeconds: number): string {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
  if (seconds < 60) return t("status.justNow");
  if (seconds < 60 * 60) {
    return t("status.minutesAgo", { count: Math.floor(seconds / 60) });
  }
  if (seconds < 24 * 60 * 60) {
    return t("status.hoursAgo", { count: Math.floor(seconds / 3600) });
  }
  return statusTime(unixSeconds);
}

/** Contact name when the chat list knows it, otherwise the JID user part. */
export function senderName(
  sender: string,
  names: Record<string, string>,
): string {
  const known = names[sender];
  if (known) return known;
  return sender.split("@")[0] || sender;
}

/** The message kind `downloadMedia` expects for a status kind. Only browser
 * mock mode uses it; the desktop path keys off the message id alone. */
export function mediaMessageKind(kind: StatusKind): MessageKind {
  switch (kind) {
    case "image":
      return "image";
    case "video":
      return "video";
    case "voice":
      return "voiceNote";
    case "text":
      return "text";
    default:
      return "document";
  }
}
