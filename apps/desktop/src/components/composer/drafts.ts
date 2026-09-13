/**
 * Per-chat composer drafts, persisted to `localStorage` under
 * `rustwa.draft.<chatId>`. Persistence is best-effort: storage may be
 * unavailable (private mode, quota) and must never break typing.
 */

import type { Jid } from "../../lib/types";

/** How long typing must pause before a draft is written. */
export const DRAFT_SAVE_DELAY_MS = 400;

const DRAFT_KEY_PREFIX = "rustwa.draft.";

function storageKey(chatId: Jid): string {
  return `${DRAFT_KEY_PREFIX}${chatId}`;
}

export function readDraft(chatId: Jid): string {
  try {
    return window.localStorage.getItem(storageKey(chatId)) ?? "";
  } catch {
    return "";
  }
}

/** Write (or clear, when empty) the draft for one chat. */
export function writeDraft(chatId: Jid, text: string): void {
  try {
    if (text.length > 0) {
      window.localStorage.setItem(storageKey(chatId), text);
    } else {
      window.localStorage.removeItem(storageKey(chatId));
    }
  } catch {
    // Drafts are a convenience; ignore unavailable storage.
  }
}
