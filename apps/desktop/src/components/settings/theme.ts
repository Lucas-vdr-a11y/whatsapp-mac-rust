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

function systemPrefersLight(): boolean {
  return window.matchMedia("(prefers-color-scheme: light)").matches;
}

/**
 * Resolves a preference to a concrete appearance. "system" follows the OS
 * appearance; the other values are returned unchanged.
 */
export function resolveTheme(theme: ThemePreference): "light" | "dark" {
  if (theme === "system") {
    return systemPrefersLight() ? "light" : "dark";
  }
  return theme;
}

/**
 * Applies a preference to the document root. The resolved value is always
 * written as `data-theme` so tokens.css stays the single source of truth.
 */
export function applyThemePreference(theme: ThemePreference): void {
  document.documentElement.dataset.theme = resolveTheme(theme);
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
 * Re-applies the theme whenever the OS appearance changes while the stored
 * preference is "system". Returns an unsubscribe function.
 */
export function watchSystemTheme(): () => void {
  const query = window.matchMedia("(prefers-color-scheme: light)");
  const onChange = () => {
    if (readThemePreference() === "system") {
      applyThemePreference("system");
    }
  };
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
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
