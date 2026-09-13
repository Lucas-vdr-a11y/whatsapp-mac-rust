import { useEffect, useState, type ReactNode } from "react";
import {
  Check,
  ChevronDown,
  Fingerprint,
  KeyRound,
  Network,
  Timer,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { writeAppLockMirror } from "../security/preference";
import { SettingsRow } from "./SettingsRow";
import { Toggle } from "./Toggle";
import { errorMessage } from "./util";

/** Result of an IPC call that may be missing or fail on this build. */
type Loadable<T> =
  | { status: "loading" }
  | { status: "ready"; value: T }
  | { status: "unavailable"; message: string };

/** Lock timeouts offered by WhatsApp; stored as seconds. */
const TIMEOUT_OPTIONS: { seconds: number; label: string }[] = [
  { seconds: 300, label: "5 minutes" },
  { seconds: 900, label: "15 minutes" },
  { seconds: 3600, label: "1 hour" },
];

function timeoutLabel(seconds: number): string {
  return (
    TIMEOUT_OPTIONS.find((option) => option.seconds === seconds)?.label ??
    `${seconds} seconds`
  );
}

/**
 * The "Security" settings group: Touch ID app lock backed by
 * `app_lock_enabled` / `set_app_lock` / `set_app_lock_timeout` and
 * `security_biometry_available`, plus honest read-only rows for surfaces the
 * protocol core does not expose yet. State stays local to this component; the
 * app store is not touched.
 */
export function SecuritySection() {
  const [enabled, setEnabled] = useState<Loadable<boolean>>(
    isTauri()
      ? { status: "loading" }
      : {
          status: "unavailable",
          message: "App lock is only available in the desktop app.",
        },
  );
  const [available, setAvailable] = useState<Loadable<boolean>>(
    isTauri()
      ? { status: "loading" }
      : {
          status: "unavailable",
          message: "Touch ID is only available in the desktop app.",
        },
  );
  const [lockTimeout, setLockTimeout] = useState<Loadable<number>>({
    status: "loading",
  });
  const [busy, setBusy] = useState(false);
  const [rowError, setRowError] = useState<string | null>(null);
  const [timeoutOpen, setTimeoutOpen] = useState(false);
  const [timeoutBusy, setTimeoutBusy] = useState(false);
  const [timeoutError, setTimeoutError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;

    void invokeCore<boolean>("app_lock_enabled")
      .then((value) => {
        if (!cancelled) {
          setEnabled({ status: "ready", value });
          writeAppLockMirror(value);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setEnabled({ status: "unavailable", message: errorMessage(cause) });
        }
      });

    void invokeCore<number>("app_lock_timeout")
      .then((value) => {
        if (!cancelled) setLockTimeout({ status: "ready", value });
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setLockTimeout({
            status: "unavailable",
            message: errorMessage(cause),
          });
        }
      });

    // "Available" means the system can evaluate device-owner auth, i.e.
    // Touch ID or the account password. On a Mac without Touch ID the
    // password fallback still works, so the toggle stays usable.
    void invokeCore<boolean>("security_biometry_available")
      .then((value) => {
        if (!cancelled) setAvailable({ status: "ready", value });
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setAvailable({ status: "unavailable", message: errorMessage(cause) });
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const lockReady = enabled.status === "ready";
  const lockEnabled = lockReady && enabled.value;
  const lockAvailable = available.status === "ready" && available.value;

  const changeEnabled = (next: boolean) => {
    if (!lockReady || busy) return;
    setBusy(true);
    setRowError(null);
    void invokeCore("set_app_lock", { enabled: next })
      .then(() => {
        setEnabled({ status: "ready", value: next });
        writeAppLockMirror(next);
      })
      .catch((cause: unknown) => setRowError(errorMessage(cause)))
      .finally(() => setBusy(false));
  };

  const changeTimeout = (seconds: number) => {
    if (timeoutBusy || lockTimeout.status !== "ready") return;
    if (lockTimeout.value === seconds) {
      setTimeoutOpen(false);
      return;
    }

    setTimeoutBusy(true);
    setTimeoutError(null);
    void invokeCore("set_app_lock_timeout", { secs: seconds })
      .then(() => {
        setLockTimeout({ status: "ready", value: seconds });
        setTimeoutOpen(false);
      })
      .catch((cause: unknown) => setTimeoutError(errorMessage(cause)))
      .finally(() => setTimeoutBusy(false));
  };

  let appLockDescription: ReactNode;
  if (rowError) {
    appLockDescription = (
      <span className="settings-inline-error">{rowError}</span>
    );
  } else if (busy) {
    appLockDescription = "Saving…";
  } else if (available.status === "unavailable") {
    appLockDescription = available.message;
  } else if (available.status === "ready" && !available.value) {
    appLockDescription =
      "Add a login password or set up Touch ID in System Settings to use app lock.";
  } else {
    appLockDescription =
      "Require Touch ID or your Mac password to open RustWA. Locks again after the app has been in the background.";
  }

  let timeoutDescription: ReactNode;
  if (timeoutError) {
    timeoutDescription = (
      <span className="settings-inline-error">{timeoutError}</span>
    );
  } else if (lockTimeout.status === "unavailable") {
    timeoutDescription = lockTimeout.message;
  } else if (!lockEnabled) {
    timeoutDescription = "Used once app lock is on.";
  } else if (lockTimeout.status === "loading") {
    timeoutDescription = "Checking…";
  } else {
    timeoutDescription = `Locks after ${timeoutLabel(
      lockTimeout.value,
    ).toLowerCase()} in the background.`;
  }

  return (
    <>
      <h2 className="settings-section-title">Security</h2>
      <div className="settings-group">
        <SettingsRow
          icon={<Fingerprint size={20} />}
          label="App lock"
          description={appLockDescription}
          control={
            <Toggle
              label="App lock"
              checked={lockEnabled}
              disabled={!lockReady || !lockAvailable || busy}
              onChange={changeEnabled}
            />
          }
        />

        <SettingsRow
          icon={<Timer size={20} />}
          label="Lock timeout"
          description={timeoutDescription}
          disabled={!lockEnabled}
          control={
            <button
              type="button"
              className="privacy-value-button"
              aria-expanded={timeoutOpen}
              disabled={
                !lockEnabled ||
                lockTimeout.status !== "ready" ||
                timeoutBusy
              }
              onClick={() => {
                setTimeoutError(null);
                setTimeoutOpen((open) => !open);
              }}
            >
              <span className="privacy-value-text">
                {timeoutBusy
                  ? "Saving…"
                  : lockTimeout.status === "ready"
                    ? timeoutLabel(lockTimeout.value)
                    : "—"}
              </span>
              <ChevronDown size={14} aria-hidden="true" />
            </button>
          }
        />
        {timeoutOpen && lockTimeout.status === "ready" ? (
          <div
            className="privacy-choice-panel"
            role="radiogroup"
            aria-label="Lock timeout"
          >
            {TIMEOUT_OPTIONS.map((option) => {
              const selected = lockTimeout.value === option.seconds;
              return (
                <button
                  key={option.seconds}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  className={`privacy-choice${selected ? " selected" : ""}`}
                  disabled={timeoutBusy}
                  onClick={() => changeTimeout(option.seconds)}
                >
                  <span className="privacy-choice-label">{option.label}</span>
                  {selected ? <Check size={16} aria-hidden="true" /> : null}
                </button>
              );
            })}
          </div>
        ) : null}

        {/* Identity verification lives in the protocol crate
            (`signal::validate_session` / `session_info`) but our core wrapper
            does not expose it yet, so there are no 60-digit codes to show. */}
        <SettingsRow
          icon={<KeyRound size={20} />}
          label="Security code"
          description="Identity verification isn't exposed by this build's core yet."
          disabled
          control={<span className="settings-value">—</span>}
        />

        {/* No proxy transport is wired through the core; WhatsApp Web's proxy
            settings have no equivalent here. */}
        <SettingsRow
          icon={<Network size={20} />}
          label="Proxy"
          description="Proxy connections aren't supported by this build yet."
          disabled
          control={<span className="settings-value">—</span>}
        />
      </div>
    </>
  );
}
