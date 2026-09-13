/** Shared helpers for the settings surfaces. */

import { t } from "../../lib/i18n";

/** Extracts a readable message from anything an IPC call can reject with. */
export function errorMessage(cause: unknown): string {
  if (cause instanceof Error && cause.message.trim()) {
    return cause.message;
  }
  if (typeof cause === "string" && cause.trim()) {
    return cause;
  }
  if (typeof cause === "object" && cause !== null && "message" in cause) {
    const message = (cause as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  return t("common.unexpectedError");
}
