import { useCallback, useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Fingerprint, KeyRound, Lock } from "lucide-react";
import { useTranslation } from "../../lib/i18n";
import { invokeCore, isTauri } from "../../lib/ipc";
import { errorMessage } from "../settings/util";
import { readAppLockMirror, writeAppLockMirror } from "./preference";

/** Host event emitted when the window refocuses after the lock timeout. */
const LOCK_EVENT = "ui://lock";
/** Host event emitted by `security_unlock` after a successful prompt. */
const UNLOCKED_EVENT = "ui://unlocked";

/**
 * Full-window app-lock overlay.
 *
 * The UI is gated twice: on mount the `app_lock_enabled` preference decides
 * whether to cover the app (an event emitted before this component mounted
 * would otherwise be lost), and a `ui://lock` event from the Rust host covers
 * it again when the window regains focus after the configured timeout.
 *
 * "Unlock with Touch ID" and "Use Mac password" both invoke `security_unlock`:
 * `LAPolicyDeviceOwnerAuthentication` prompts for Touch ID first and falls
 * back to the account password inside the same system sheet, so there is no
 * separate password path to implement.
 */
export function LockScreen() {
  const { t } = useTranslation();
  // Read the mirrored preference synchronously so the overlay can cover the
  // very first paint; the effect below still verifies it against the host.
  const [locked, setLocked] = useState(() => isTauri() && readAppLockMirror());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    const subscribe = (event: string, handler: () => void) => {
      void listen(event, handler).then((unlisten) => {
        if (cancelled) unlisten();
        else unlisteners.push(unlisten);
      });
    };

    subscribe(LOCK_EVENT, () => {
      setError(null);
      setLocked(true);
    });
    subscribe(UNLOCKED_EVENT, () => {
      setError(null);
      setLocked(false);
    });

    // Launch gate: the persisted preference is authoritative. The mirror can
    // be stale, so an enabled preference keeps the overlay up, a disabled one
    // removes it, and an unavailable command (older build) never traps the UI.
    void invokeCore<boolean>("app_lock_enabled")
      .then((enabled) => {
        if (cancelled) return;
        writeAppLockMirror(enabled);
        setLocked(enabled);
      })
      .catch(() => {
        if (!cancelled) setLocked(false);
      });

    return () => {
      cancelled = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  const unlock = useCallback((reason: string) => {
    setBusy(true);
    setError(null);
    void invokeCore("security_unlock", { reason })
      // The command also emits `ui://unlocked`; this keeps the overlay
      // responsive even if an event listener is unavailable.
      .then(() => setLocked(false))
      .catch((cause: unknown) => setError(errorMessage(cause)))
      .finally(() => setBusy(false));
  }, []);

  if (!locked) return null;

  return (
    <div
      className="lock-overlay"
      role="dialog"
      aria-modal="true"
      aria-label={t("security.lockedAria")}
    >
      <div className="lock-card">
        <span className="lock-badge" aria-hidden="true">
          <Lock size={28} strokeWidth={1.8} />
        </span>
        <h1 className="lock-title">{t("security.lockedTitle")}</h1>
        <p className="lock-subtitle">{t("security.lockedSubtitle")}</p>

        <div className="lock-actions">
          <button
            type="button"
            className="lock-button primary"
            autoFocus
            disabled={busy}
            onClick={() => unlock(t("security.unlockReason"))}
          >
            <Fingerprint size={18} aria-hidden="true" />
            {busy
              ? t("security.waitingForTouchId")
              : t("security.unlockWithTouchId")}
          </button>
          <button
            type="button"
            className="lock-button"
            disabled={busy}
            onClick={() => unlock(t("security.unlockPasswordReason"))}
          >
            <KeyRound size={16} aria-hidden="true" />
            {t("security.useMacPassword")}
          </button>
        </div>

        {error ? (
          <p className="lock-error" role="alert">
            {error}
          </p>
        ) : (
          <p className="lock-hint">{t("security.touchIdHint")}</p>
        )}
      </div>
    </div>
  );
}
