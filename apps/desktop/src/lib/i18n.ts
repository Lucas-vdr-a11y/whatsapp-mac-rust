/**
 * Tiny dependency-free localization layer.
 *
 * - English and Dutch live in `src/locales/*.json` as flat, dot-namespaced
 *   key maps.
 * - The active language persists in `localStorage` under `rustwa.lang` and
 *   defaults to the browser language (`nl` when it starts with "nl", else
 *   `en`).
 * - Components subscribe through `useTranslation()`, which re-renders when
 *   the language changes.
 * - Missing keys fall back to English, then to the key itself, so a missing
 *   translation can never crash the UI.
 */

import { useSyncExternalStore } from "react";
import en from "../locales/en.json";
import nl from "../locales/nl.json";

export type Lang = "en" | "nl";

const STORAGE_KEY = "rustwa.lang";

const translations: Record<Lang, Record<string, string>> = {
  en: en as Record<string, string>,
  nl: nl as Record<string, string>,
};

/** Stored preference first, then the browser language. */
function detectLang(): Lang {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === "en" || stored === "nl") return stored;
  } catch {
    // localStorage may be unavailable; fall through to the browser language.
  }
  const browser =
    typeof navigator !== "undefined" ? navigator.language ?? "" : "";
  return browser.toLowerCase().startsWith("nl") ? "nl" : "en";
}

let currentLang: Lang = detectLang();

if (typeof document !== "undefined") {
  document.documentElement.lang = currentLang;
}

const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The active language. */
export function getLang(): Lang {
  return currentLang;
}

/** Persists and broadcasts a new language. */
export function setLang(lang: Lang): void {
  if (lang === currentLang) return;
  currentLang = lang;
  try {
    window.localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    // Persistence is best-effort; the session still switches.
  }
  if (typeof document !== "undefined") {
    document.documentElement.lang = lang;
  }
  for (const listener of listeners) listener();
}

export type TranslationVars = Record<string, string | number>;

/**
 * Translates `key`, interpolating `{name}` placeholders from `vars`.
 * Missing keys fall back to English and finally to the key itself.
 */
export function t(key: string, vars?: TranslationVars): string {
  const table = translations[currentLang] ?? translations.en;
  const raw = table[key] ?? translations.en[key] ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = vars[name];
    return value === undefined || value === null ? match : String(value);
  });
}

/** Subscribes the calling component to language changes. */
export function useTranslation(): { t: typeof t; lang: Lang } {
  const lang = useSyncExternalStore(subscribe, getLang, getLang);
  return { t, lang };
}
