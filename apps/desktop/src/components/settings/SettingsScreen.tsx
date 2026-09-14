import { useEffect, useState, type ReactNode } from "react";
import {
  Bell,
  Download,
  FileText,
  Info,
  Languages,
  LogOut,
  MessageCircle,
  Monitor,
  Power,
  ShieldAlert,
  SunMoon,
} from "lucide-react";
import {
  setLang,
  t,
  useTranslation,
  type Lang,
} from "../../lib/i18n";
import { invokeCore, isTauri } from "../../lib/ipc";
import { useAppStore } from "../../store/app";
import { ScreenHeader } from "../screens/shared";
import { ConfirmDialog } from "./ConfirmDialog";
import { PrivacySection } from "./PrivacySection";
import {
  readAutoDownloadPolicy,
  writeAutoDownloadPolicy,
  type AutoDownloadPolicy,
} from "../message/media";
import { SecuritySection } from "./SecuritySection";
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

const THEME_OPTIONS: { value: ThemePreference; labelKey: string }[] = [
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" },
  { value: "system", labelKey: "settings.themeSystem" },
];

const LANGUAGE_OPTIONS: { value: Lang; labelKey: string }[] = [
  { value: "en", labelKey: "settings.langEnglish" },
  { value: "nl", labelKey: "settings.langNederlands" },
];

function ThemePicker({
  value,
  onChange,
}: {
  value: ThemePreference;
  onChange: (theme: ThemePreference) => void;
}) {
  const { t: translate } = useTranslation();

  return (
    <div
      className="settings-segments"
      role="radiogroup"
      aria-label={translate("settings.themeAria")}
    >
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
          {translate(option.labelKey)}
        </button>
      ))}
    </div>
  );
}

/** English / Nederlands segmented control; applies the choice immediately. */
function LanguagePicker({
  value,
  onChange,
}: {
  value: Lang;
  onChange: (lang: Lang) => void;
}) {
  const { t: translate } = useTranslation();

  return (
    <div
      className="settings-segments"
      role="radiogroup"
      aria-label={translate("settings.languageAria")}
    >
      {LANGUAGE_OPTIONS.map((option) => (
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
          {translate(option.labelKey)}
        </button>
      ))}
    </div>
  );
}

function describePermission(state: Loadable<boolean>): string {
  if (state.status === "loading") return t("settings.checkingPermission");
  if (state.status === "ready") {
    return state.value
      ? t("settings.permissionAllowed")
      : t("settings.permissionDenied");
  }
  return state.message;
}

function describeAutostart(state: Loadable<boolean>): string {
  if (state.status === "loading") return t("settings.checking");
  if (state.status === "ready") {
    return t("settings.autostartDescription");
  }
  return state.message;
}

export function SettingsScreen() {
  const [autoDownload, setAutoDownload] = useState<AutoDownloadPolicy>(
    readAutoDownloadPolicy(),
  );
  const { t: translate, lang } = useTranslation();
  const [theme, setTheme] = useState<ThemePreference>(() =>
    readThemePreference(),
  );
  const [capabilities, setCapabilities] = useState<Loadable<PlatformCapabilities>>(
    isTauri()
      ? { status: "loading" }
      : {
          status: "unavailable",
          message: t("settings.onlyDesktop"),
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
        message: t("settings.notificationsOnlyDesktop"),
      });
      return;
    }
    if (capabilities.status === "loading") return;
    if (capabilities.status === "ready" && !capabilities.value.notifications) {
      setPermission({
        status: "unavailable",
        message: t("settings.notificationsUnsupported"),
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
        message: t("settings.autostartOnlyDesktop"),
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
        message: t("settings.buildInfoOnlyDesktop"),
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

  const changeLanguage = (next: Lang) => {
    setLang(next);
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
      body: t("settings.testNotificationBody"),
    })
      .then(() =>
        setNotifyNote({ kind: "ok", text: t("settings.testNotificationSent") }),
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
      setDangerError(t("settings.actionOnlyDesktop"));
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
        {permissionBusy ? t("settings.sending") : t("settings.sendTest")}
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
        {permissionBusy ? t("settings.requesting") : t("settings.request")}
      </button>
    );
  }

  return (
    <section className="chat-list screen">
      <ScreenHeader title={translate("settings.title")} />

      <div className="screen-body settings-body">
        <h2 className="settings-section-title">
          {translate("settings.appearance")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={<SunMoon size={20} />}
            label={translate("settings.theme")}
            description={translate("settings.themeDescription")}
            control={<ThemePicker value={theme} onChange={changeTheme} />}
          />
          <SettingsRow
            icon={<Languages size={20} />}
            label={translate("settings.language")}
            description={translate("settings.languageDescription")}
            control={
              <LanguagePicker value={lang} onChange={changeLanguage} />
            }
          />
        </div>

        <h2 className="settings-section-title">
          {translate("settings.media")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Download size={20} />}
            label={translate("settings.autoDownload")}
            description={translate("settings.autoDownloadDescription")}
            control={
              <select
                className="settings-select"
                value={autoDownload}
                aria-label={translate("settings.autoDownload")}
                onChange={(event) => {
                  const policy = event.target.value as AutoDownloadPolicy;
                  setAutoDownload(policy);
                  writeAutoDownloadPolicy(policy);
                }}
              >
                <option value="always">
                  {translate("settings.autoDownloadAlways")}
                </option>
                <option value="wifi">
                  {translate("settings.autoDownloadWifi")}
                </option>
                <option value="never">
                  {translate("settings.autoDownloadNever")}
                </option>
              </select>
            }
          />
        </div>

        <h2 className="settings-section-title">
          {translate("settings.notifications")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Bell size={20} />}
            label={translate("settings.notificationPermission")}
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
            label={translate("settings.dockBadge")}
            description={
              badgeError ? (
                <span className="settings-inline-error">{badgeError}</span>
              ) : badgeSupported ? (
                translate("settings.dockBadgeDescription")
              ) : (
                translate("settings.dockBadgeUnsupported")
              )
            }
            control={
              <Toggle
                label={translate("settings.dockBadge")}
                checked={badgeEnabled}
                disabled={!badgeSupported}
                onChange={changeBadge}
              />
            }
          />
        </div>

        <PrivacySection />
        <SecuritySection />

        <h2 className="settings-section-title">
          {translate("settings.startup")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Power size={20} />}
            label={translate("settings.launchAtLogin")}
            description={
              autostartError ? (
                <span className="settings-inline-error">{autostartError}</span>
              ) : (
                describeAutostart(autostart)
              )
            }
            control={
              <Toggle
                label={translate("settings.launchAtLogin")}
                checked={autostart.status === "ready" && autostart.value}
                disabled={autostart.status !== "ready" || autostartBusy}
                onChange={changeAutostart}
              />
            }
          />
        </div>

        <h2 className="settings-section-title">
          {translate("settings.storage")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={<MessageCircle size={20} />}
            label={translate("settings.chats")}
            control={<span className="settings-value">{chatCount}</span>}
          />
          <SettingsRow
            icon={<FileText size={20} />}
            label={translate("settings.messagesCached")}
            control={<span className="settings-value">{messageCount}</span>}
          />
        </div>
        <p className="settings-note">{translate("settings.storageNote")}</p>

        <h2 className="settings-section-title">
          {translate("settings.about")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={<Info size={20} />}
            label={translate("settings.application")}
            control={
              <span className="settings-value">
                {appInfo.status === "ready" ? appInfo.value.name : "—"}
              </span>
            }
          />
          <SettingsRow
            label={translate("settings.version")}
            control={
              <span className="settings-value">
                {appInfo.status === "ready" ? appInfo.value.version : "—"}
              </span>
            }
          />
          <SettingsRow
            label={translate("settings.core")}
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
          <span>{translate("settings.disclaimer")}</span>
        </p>

        <h2 className="settings-section-title danger">
          {translate("settings.dangerZone")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            danger
            icon={<LogOut size={20} />}
            label={translate("settings.logOut")}
            description={translate("settings.logOutDescription")}
            onClick={() => openDanger("logout")}
          />
          <SettingsRow
            danger
            icon={<ShieldAlert size={20} />}
            label={translate("settings.resetSession")}
            description={translate("settings.resetDescription")}
            onClick={() => openDanger("reset")}
          />
        </div>
      </div>

      <ConfirmDialog
        open={dangerAction === "logout"}
        title={translate("settings.logOutTitle")}
        body={translate("settings.logOutBody")}
        confirmLabel={translate("settings.logOutConfirm")}
        danger
        busy={dangerBusy}
        error={dangerError}
        onCancel={closeDanger}
        onConfirm={() => runDangerAction("core_logout")}
      />
      <ConfirmDialog
        open={dangerAction === "reset"}
        title={translate("settings.resetTitle")}
        body={translate("settings.resetBody")}
        confirmLabel={translate("settings.resetConfirm")}
        confirmPhrase={translate("settings.resetPhrase")}
        danger
        busy={dangerBusy}
        error={dangerError}
        onCancel={closeDanger}
        onConfirm={() => runDangerAction("core_reset_session")}
      />
    </section>
  );
}
