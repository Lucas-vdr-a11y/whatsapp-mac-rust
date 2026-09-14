/** localStorage mirror of the app-lock preference.
 *
 * `app_lock_enabled` is an async IPC round-trip, so `LockScreen` cannot know
 * at first paint whether to cover the UI. `SecuritySection` mirrors the
 * preference here whenever the toggle changes; the lock screen reads it
 * synchronously and still verifies against the host afterwards (a stale
 * mirror can only show the overlay for a moment, never hide a lock).
 */

const STORAGE_KEY = "rustwa.app-lock";

/** True when the last toggle (or launch check) enabled app lock. */
export function readAppLockMirror(): boolean {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "on";
  } catch {
    return false;
  }
}

/** Records the preference so the next launch can gate before first paint. */
export function writeAppLockMirror(enabled: boolean): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, enabled ? "on" : "off");
  } catch {
    // Persistence is best-effort; the host preference remains authoritative.
  }
}
