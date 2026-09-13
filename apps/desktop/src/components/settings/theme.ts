/** Theme preference handling: persisted in localStorage, applied on <html>. */

export type ThemePreference = "dark" | "light" | "system";

export const THEME_STORAGE_KEY = "rustwa.theme";

/** Reads the stored preference, defaulting to "system". */
export function readThemePreference(): ThemePreference {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === "dark" || stored === "light" || stored === "system") {
      return stored;
    }
  } catch {
    // localStorage can be unavailable (private mode, locked-down webview).
  }
  return "system";
}

/**
 * Applies a preference to the document root. "system" removes the attribute so
 * the `prefers-color-scheme` rules in settings.css take over.
 */
export function applyThemePreference(theme: ThemePreference): void {
  const root = document.documentElement;
  if (theme === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.dataset.theme = theme;
  }
}

/** Persists a preference; failures are non-fatal. */
export function storeThemePreference(theme: ThemePreference): void {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Persistence is best-effort.
  }
}

/**
 * Boot-time theme application. The `?theme=` URL param is a dev-only override
 * and wins over the stored preference; otherwise the stored value is used.
 */
export function applyBootTheme(search: string): void {
  const override = new URLSearchParams(search).get("theme");
  if (override === "light" || override === "dark") {
    document.documentElement.dataset.theme = override;
    return;
  }
  applyThemePreference(readThemePreference());
}
