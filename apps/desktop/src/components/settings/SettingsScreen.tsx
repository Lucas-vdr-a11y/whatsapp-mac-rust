import { useEffect, useState, type ReactNode } from "react";
import {
  Bell,
  FileText,
  Info,
  LogOut,
  MessageCircle,
  Monitor,
  Power,
  ShieldAlert,
  SunMoon,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { useAppStore } from "../../store/app";
import { ScreenHeader } from "../screens/shared";
import { ConfirmDialog } from "./ConfirmDialog";
import { PrivacySection } from "./PrivacySection";
import { SettingsRow } from "./SettingsRow";
import { Toggle } from "./Toggle";
import {
  applyThemePreference,
  readThemePreference,
  storeThemePreference,
  type ThemePreference,
} from "./theme";
import { errorMessage } from "./util";

/** Payload of `app_info`. */
interface AppInfo {
  name: string;
  version: string;
  core: string;
}

/** Payload of `platform_capabilities`. */
interface PlatformCapabilities {
  badge: boolean;
  notifications: boolean;
  tray: boolean;
}

/** Result of an IPC call that may be missing or fail on this build. */
type Loadable<T> =
  | { status: "loading" }
  | { status: "ready"; value: T }
  | { status: "unavailable"; message: string };

const BADGE_STORAGE_KEY = "rustwa.badge";

function readBadgePreference(): boolean {
  try {
    return window.localStorage.getItem(BADGE_STORAGE_KEY) !== "off";
  } catch {
    return true;
  }
}

function writeBadgePreference(enabled: boolean): void {
  try {
    window.localStorage.setItem(BADGE_STORAGE_KEY, enabled ? "on" : "off");
  } catch {
    // Persistence is best-effort; the toggle still works for this session.
  }
}

const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

function ThemePicker({
  value,
  onChange,
}: {
  value: ThemePreference;
  onChange: (theme: ThemePreference) => void;
}) {
  return (
    <div className="settings-segments" role="radiogroup" aria-label="Theme">
      {THEME_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          className={`settings-segment${
            value === option.value ? " active" : ""
          }`}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function describePermission(state: Loadable<boolean>): string {
  if (state.status === "loading") return "Checking notification permission…";
  if (state.status === "ready") {
    return state.value
      ? "Allowed — notifications can appear on this Mac."
      : "Denied — allow RustWA in System Settings › Notifications, then request again.";
  }
  return state.message;
}

function describeAutostart(state: Loadable<boolean>): string {
  if (state.status === "loading") return "Checking…";
  if (state.status === "ready") {
    return "Open RustWA automatically when you sign in to your Mac.";
  }
  return state.message;
}

export function SettingsScreen() {
  const [theme, setTheme] = useState<ThemePreference>(() =>
    readThemePreference(),
  );
  const [capabilities, setCapabilities] = useState<Loadable<PlatformCapabilities>>(
    isTauri()
      ? { status: "loading" }
      : {
          status: "unavailable",
          message: "Only available in the desktop app.",
        },
  );
  const [permission, setPermission] = useState<Loadable<boolean>>({
    status: "loading",
  });
  const [permissionBusy, setPermissionBusy] = useState(false);
  const [notifyNote, setNotifyNote] = useState<{
    kind: "ok" | "error";
    text: string;
  } | null>(null);
  const [badgeEnabled, setBadgeEnabled] = useState(readBadgePreference);
  const [badgeError, setBadgeError] = useState<string | null>(null);
  const [autostart, setAutostart] = useState<Loadable<boolean>>({
    status: "loading",
  });
  const [autostartBusy, setAutostartBusy] = useState(false);
  const [autostartError, setAutostartError] = useState<string | null>(null);
  const [appInfo, setAppInfo] = useState<Loadable<AppInfo>>({
    status: "loading",
  });
  const [dangerAction, setDangerAction] = useState<"logout" | "reset" | null>(
    null,
  );
  const [dangerBusy, setDangerBusy] = useState(false);
  const [dangerError, setDangerError] = useState<string | null>(null);

  const unreadCount = useAppStore((state) =>
    state.chats.reduce(
      (sum, chat) => sum + (chat.muted ? 0 : chat.unreadCount),
      0,
    ),
  );
  const chatCount = useAppStore((state) => state.chats.length);
  const messageCount = useAppStore((state) =>
    Object.values(state.messages).reduce(
      (sum, messages) => sum + messages.length,
      0,
    ),
  );

  // Platform capabilities are advisory: when the command is missing we stay
  // permissive and let each individual action surface its own error.
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    invokeCore<PlatformCapabilities>("platform_capabilities")
      .then((value) => {
        if (!cancelled) setCapabilities({ status: "ready", value });
      })
      .catch((cause) => {
        if (!cancelled) {
          setCapabilities({
            status: "unavailable",
            message: errorMessage(cause),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Query the permission once the platform is known. The command requests
  // permission when the user has not decided yet.
  useEffect(() => {
    if (!isTauri()) {
      setPermission({
        status: "unavailable",
        message: "Notifications are only available in the desktop app.",
      });
      return;
    }
    if (capabilities.status === "loading") return;
    if (capabilities.status === "ready" && !capabilities.value.notifications) {
      setPermission({
        status: "unavailable",
        message: "Notifications are not supported on this platform.",
      });
      return;
    }
    let cancelled = false;
    setPermission({ status: "loading" });
    invokeCore<boolean>("notification_permission")
      .then((granted) => {
        if (!cancelled) setPermission({ status: "ready", value: granted });
      })
      .catch((cause) => {
        if (!cancelled) {
          setPermission({
            status: "unavailable",
            message: errorMessage(cause),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [capabilities]);

  useEffect(() => {
    if (!isTauri()) {
      setAutostart({
        status: "unavailable",
        message: "Launch at login is only available in the desktop app.",
      });
      return;
    }
    let cancelled = false;
    invokeCore<boolean>("autostart_enabled")
      .then((enabled) => {
        if (!cancelled) setAutostart({ status: "ready", value: enabled });
      })
      .catch((cause) => {
        if (!cancelled) {
          setAutostart({
            status: "unavailable",
            message: errorMessage(cause),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      setAppInfo({
        status: "unavailable",
        message: "Build information is only available in the desktop app.",
      });
      return;
    }
    let cancelled = false;
    invokeCore<AppInfo>("app_info")
      .then((value) => {
        if (!cancelled) setAppInfo({ status: "ready", value });
      })
      .catch((cause) => {
        if (!cancelled) {
          setAppInfo({
            status: "unavailable",
            message: errorMessage(cause),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Keep the dock badge in sync with the unread count while enabled.
  useEffect(() => {
    if (!isTauri()) return;
    if (capabilities.status === "loading") return;
    if (capabilities.status === "ready" && !capabilities.value.badge) return;
    const count = badgeEnabled && unreadCount > 0 ? unreadCount : null;
    invokeCore("set_badge", { count })
      .then(() => setBadgeError(null))
      .catch((cause) => setBadgeError(errorMessage(cause)));
  }, [badgeEnabled, unreadCount, capabilities]);

  const badgeSupported =
    capabilities.status === "ready" ? capabilities.value.badge : true;

  const changeTheme = (next: ThemePreference) => {
    setTheme(next);
    storeThemePreference(next);
    applyThemePreference(next);
  };

  const changeBadge = (enabled: boolean) => {
    setBadgeEnabled(enabled);
    writeBadgePreference(enabled);
  };

  const requestPermission = () => {
    if (!isTauri()) return;
    setPermissionBusy(true);
    setNotifyNote(null);
    invokeCore<boolean>("notification_permission")
      .then((granted) => setPermission({ status: "ready", value: granted }))
      .catch((cause) =>
        setPermission({
          status: "unavailable",
          message: errorMessage(cause),
        }),
      )
      .finally(() => setPermissionBusy(false));
  };

  const sendTestNotification = () => {
    setPermissionBusy(true);
    setNotifyNote(null);
    invokeCore("notify", {
      title: "RustWA",
      body: "Notifications are set up correctly.",
    })
      .then(() =>
        setNotifyNote({ kind: "ok", text: "Test notification sent." }),
      )
      .catch((cause) =>
        setNotifyNote({ kind: "error", text: errorMessage(cause) }),
      )
      .finally(() => setPermissionBusy(false));
  };

  const changeAutostart = (enabled: boolean) => {
    if (autostart.status !== "ready") return;
    setAutostartBusy(true);
    setAutostartError(null);
    invokeCore("set_autostart", { enabled })
      .then(() => setAutostart({ status: "ready", value: enabled }))
      .catch((cause) => setAutostartError(errorMessage(cause)))
      .finally(() => setAutostartBusy(false));
  };

  const openDanger = (action: "logout" | "reset") => {
    setDangerError(null);
    setDangerAction(action);
  };

  const closeDanger = () => {
    if (dangerBusy) return;
    setDangerAction(null);
    setDangerError(null);
  };

  const runDangerAction = (command: "core_logout" | "core_reset_session") => {
    if (!isTauri()) {
      setDangerError("This action is only available in the desktop app.");
      return;
    }
    setDangerBusy(true);
    setDangerError(null);
    void invokeCore(command)
      .then(() => {
        // Reloading intentionally clears all in-memory UI state (selected
        // chat, cached messages, store data); the Rust core owns the session.
        window.location.reload();
      })
      .catch((cause) => {
        setDangerError(errorMessage(cause));
        setDangerBusy(false);
      });
  };

  let notificationAction: ReactNode = null;
  if (permission.status === "ready" && permission.value) {
    notificationAction = (
      <button
        type="button"
        className="settings-button"
        disabled={permissionBusy}
        onClick={sendTestNotification}
      >
        {permissionBusy ? "Sending…" : "Send test"}
      </button>
    );
  } else if (permission.status !== "loading" && isTauri()) {
    notificationAction = (
      <button
        type="button"
        className="settings-button"
        disabled={permissionBusy}
        onClick={requestPermission}
      >
        {permissionBusy ? "Requesting…" : "Request"}
      </button>
    );
  }

  return (
    <section className="chat-list screen">
      <ScreenHeader title="Settings" />

      <div className="screen-body settings-body">
        <h2 className="settings-section-title">Appearance</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<SunMoon size={20} />}
            label="Theme"
            description="System follows your Mac's appearance."
            control={<ThemePicker value={theme} onChange={changeTheme} />}
          />
        </div>

        <h2 className="settings-section-title">Notifications</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Bell size={20} />}
            label="Notification permission"
            description={
              notifyNote ? (
                <span
                  className={
                    notifyNote.kind === "error"
                      ? "settings-inline-error"
                      : "settings-inline-ok"
                  }
                >
                  {notifyNote.text}
                </span>
              ) : (
                describePermission(permission)
              )
            }
            control={notificationAction}
          />
          <SettingsRow
            icon={<Monitor size={20} />}
            label="Dock badge"
            description={
              badgeError ? (
                <span className="settings-inline-error">{badgeError}</span>
              ) : badgeSupported ? (
                "Show a count of unread chats on the app icon."
              ) : (
                "Dock badges are not supported on this platform."
              )
            }
            control={
              <Toggle
                label="Dock badge"
                checked={badgeEnabled}
                disabled={!badgeSupported}
                onChange={changeBadge}
              />
            }
          />
        </div>

        <PrivacySection />

        <h2 className="settings-section-title">Startup</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Power size={20} />}
            label="Launch at login"
            description={
              autostartError ? (
                <span className="settings-inline-error">{autostartError}</span>
              ) : (
                describeAutostart(autostart)
              )
            }
            control={
              <Toggle
                label="Launch at login"
                checked={autostart.status === "ready" && autostart.value}
                disabled={autostart.status !== "ready" || autostartBusy}
                onChange={changeAutostart}
              />
            }
          />
        </div>

        <h2 className="settings-section-title">Storage</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<MessageCircle size={20} />}
            label="Chats"
            control={<span className="settings-value">{chatCount}</span>}
          />
          <SettingsRow
            icon={<FileText size={20} />}
            label="Messages cached"
            control={<span className="settings-value">{messageCount}</span>}
          />
        </div>
        <p className="settings-note">
          Counts come from the in-memory store; history is cached as chats are
          opened.
        </p>

        <h2 className="settings-section-title">About</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Info size={20} />}
            label="Application"
            control={
              <span className="settings-value">
                {appInfo.status === "ready" ? appInfo.value.name : "—"}
              </span>
            }
          />
          <SettingsRow
            label="Version"
            control={
              <span className="settings-value">
                {appInfo.status === "ready" ? appInfo.value.version : "—"}
              </span>
            }
          />
          <SettingsRow
            label="Core"
            control={
              <span className="settings-value">
                {appInfo.status === "ready" ? appInfo.value.core : "—"}
              </span>
            }
          />
        </div>
        {appInfo.status === "unavailable" ? (
          <p className="settings-note settings-inline-error">
            {appInfo.message}
          </p>
        ) : null}
        <p className="settings-disclaimer">
          <Info size={14} aria-hidden="true" />
          <span>
            RustWA is an unofficial WhatsApp client and is not affiliated with,
            endorsed by, or sponsored by WhatsApp LLC or Meta Platforms, Inc.
          </span>
        </p>

        <h2 className="settings-section-title danger">Danger zone</h2>
        <div className="settings-group">
          <SettingsRow
            danger
            icon={<LogOut size={20} />}
            label="Log out"
            description="Unlink this device. You will need to scan the QR code again."
            onClick={() => openDanger("logout")}
          />
          <SettingsRow
            danger
            icon={<ShieldAlert size={20} />}
            label="Reset local session"
            description="Delete the local session and cached data on this computer."
            onClick={() => openDanger("reset")}
          />
        </div>
      </div>

      <ConfirmDialog
        open={dangerAction === "logout"}
        title="Log out of RustWA?"
        body="This unlinks this device from your WhatsApp account. You will need to scan the QR code again to use RustWA."
        confirmLabel="Log out"
        danger
        busy={dangerBusy}
        error={dangerError}
        onCancel={closeDanger}
        onConfirm={() => runDangerAction("core_logout")}
      />
      <ConfirmDialog
        open={dangerAction === "reset"}
        title="Reset local session?"
        body="This permanently deletes the local session and all cached data on this computer. You will need to link this device again. This cannot be undone."
        confirmLabel="Reset session"
        confirmPhrase="reset"
        danger
        busy={dangerBusy}
        error={dangerError}
        onCancel={closeDanger}
        onConfirm={() => runDangerAction("core_reset_session")}
      />
    </section>
  );
}
