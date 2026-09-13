import { useState } from "react";
import { LogOut, MessageCircle } from "lucide-react";
import { useTranslation } from "../../lib/i18n";
import { invokeCore, isTauri } from "../../lib/ipc";
import { initials } from "../../lib/names";
import { useAppStore } from "../../store/app";
import { ScreenHeader } from "../screens/shared";
import { ConfirmDialog } from "./ConfirmDialog";
import { SettingsRow } from "./SettingsRow";
import { errorMessage } from "./util";

/** Own profile placeholder plus linked-account status and quick actions. */
export function ProfileScreen() {
  const { t } = useTranslation();
  const connection = useAppStore((state) => state.connection);
  const paired = useAppStore((state) => state.paired);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const statusLabel = !paired
    ? t("profile.notLinked")
    : connection === "connected"
      ? t("profile.connected")
      : connection === "connecting"
        ? t("profile.connecting")
        : t("profile.disconnected");
  const statusTone = !paired
    ? "off"
    : connection === "connected"
      ? "ok"
      : connection === "connecting"
        ? "pending"
        : "off";

  const openLogout = () => {
    setError(null);
    setConfirmOpen(true);
  };

  const cancelLogout = () => {
    if (busy) return;
    setConfirmOpen(false);
    setError(null);
  };

  const logOut = () => {
    if (!isTauri()) {
      setError(t("profile.logOutOnlyDesktop"));
      return;
    }
    setBusy(true);
    setError(null);
    void invokeCore("core_logout")
      .then(() => {
        // Reloading intentionally clears all in-memory UI state (selected
        // chat, cached messages, store data); the Rust core owns the session.
        window.location.reload();
      })
      .catch((cause) => {
        setError(errorMessage(cause));
        setBusy(false);
      });
  };

  return (
    <section className="chat-list screen">
      <ScreenHeader title={t("profile.title")} />

      <div className="screen-body">
        <div className="profile-hero">
          <div className="avatar profile-avatar">
            {initials(t("profile.you"))}
          </div>
          <p className="profile-name">{t("profile.you")}</p>
          <p className="profile-subtitle">{t("profile.subtitle")}</p>
        </div>

        <h2 className="settings-section-title">
          {t("profile.linkedAccount")}
        </h2>
        <div className="settings-group">
          <SettingsRow
            icon={
              <span className={`profile-dot ${statusTone}`} aria-hidden="true" />
            }
            label={t("profile.connection")}
            control={<span className="settings-value">{statusLabel}</span>}
          />
          {/* TODO: render the self JID here once the core exposes a command
              for it. There is no `self_jid` IPC yet, so show a placeholder. */}
          <SettingsRow
            label={t("profile.phoneNumber")}
            description={t("profile.phoneDescription")}
            control={<span className="settings-value">—</span>}
          />
        </div>

        <h2 className="settings-section-title">{t("profile.quickActions")}</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<MessageCircle size={20} />}
            label={t("profile.newChat")}
            description={t("profile.comingSoon")}
            disabled
            title={t("profile.newChatUnavailable")}
          />
          <SettingsRow
            danger
            icon={<LogOut size={20} />}
            label={t("profile.logOut")}
            description={t("profile.logOutDescription")}
            onClick={openLogout}
          />
        </div>
      </div>

      <ConfirmDialog
        open={confirmOpen}
        title={t("settings.logOutTitle")}
        body={t("settings.logOutBody")}
        confirmLabel={t("settings.logOutConfirm")}
        danger
        busy={busy}
        error={error}
        onCancel={cancelLogout}
        onConfirm={logOut}
      />
    </section>
  );
}
