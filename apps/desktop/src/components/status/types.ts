/** Shared types for the status (stories) screen and its viewer.
 *
 * Mirrors of the core's `StatusUpdate` (camelCase over IPC) and of the
 * sender-grouping the list builds on top of it. */

export type StatusKind = "text" | "image" | "video" | "voice" | "unknown";

/** One status update as served by `statuses_list`. */
export interface StatusUpdate {
  id: string;
  sender: string;
  timestamp: number;
  kind: StatusKind;
  text: string | null;
  backgroundArgb: number | null;
  expiresAt: number;
  viewed: boolean;
}

/** Updates of one sender, grouped for the list. Newest first. */
export interface StatusGroup {
  sender: string;
  updates: StatusUpdate[];
  latest: StatusUpdate;
  unviewed: number;
}

/** WhatsApp's story lifetime, mirroring the core's `STATUS_TTL_SECS`. */
export const STATUS_TTL_SECS = 24 * 60 * 60;

/** True when the update's payload lives in the media store, not on the card. */
export function isMediaKind(kind: StatusKind): boolean {
  return kind === "image" || kind === "video" || kind === "voice";
}
