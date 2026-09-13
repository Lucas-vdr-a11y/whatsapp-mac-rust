/** Timestamp formatting helpers matching WhatsApp's conventions. */

import { t } from "./i18n";
export function formatListTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const now = new Date();

  if (isSameDay(date, now)) {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) return t("time.yesterday");

  const withinWeek = now.getTime() - date.getTime() < 6 * 24 * 60 * 60 * 1000;
  if (withinWeek) {
    return date.toLocaleDateString([], { weekday: "short" });
  }

  return date.toLocaleDateString([], {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
  });
}

export function formatBubbleTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDateDivider(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const now = new Date();

  if (isSameDay(date, now)) return t("time.today");

  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (isSameDay(date, yesterday)) return t("time.yesterday");

  return date.toLocaleDateString([], {
    day: "numeric",
    month: "long",
    year: "numeric",
  });
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}
