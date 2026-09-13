import { useState } from "react";
import { LogOut, MessageCircle } from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { initials } from "../../lib/names";
import { useAppStore } from "../../store/app";
import { ScreenHeader } from "../screens/shared";
import { ConfirmDialog } from "./ConfirmDialog";
import { SettingsRow } from "./SettingsRow";
import { errorMessage } from "./util";

/** Own profile placeholder plus linked-account status and quick actions. */
export function ProfileScreen() {
  const connection = useAppStore((state) => state.connection);
  const paired = useAppStore((state) => state.paired);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const statusLabel = !paired
    ? "Not linked"
    : connection === "connected"
      ? "Connected"
      : connection === "connecting"
        ? "Connecting…"
        : "Disconnected";
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
      setError("Logging out is only available in the desktop app.");
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
      <ScreenHeader title="Profile" />

      <div className="screen-body">
        <div className="profile-hero">
          <div className="avatar profile-avatar">{initials("You")}</div>
          <p className="profile-name">You</p>
          <p className="profile-subtitle">
            Your local profile. Photo, name and about text are coming soon.
          </p>
        </div>

        <h2 className="settings-section-title">Linked account</h2>
        <div className="settings-group">
          <SettingsRow
            icon={
              <span className={`profile-dot ${statusTone}`} aria-hidden="true" />
            }
            label="Connection"
            control={<span className="settings-value">{statusLabel}</span>}
          />
          {/* TODO: render the self JID here once the core exposes a command
              for it. There is no `self_jid` IPC yet, so show a placeholder. */}
          <SettingsRow
            label="Phone number"
            description="The number this device is linked to."
            control={<span className="settings-value">—</span>}
          />
        </div>

        <h2 className="settings-section-title">Quick actions</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<MessageCircle size={20} />}
            label="New chat"
            description="Coming soon."
            disabled
            title="New chat is not available yet"
          />
          <SettingsRow
            danger
            icon={<LogOut size={20} />}
            label="Log out"
            description="Unlink this device from your account."
            onClick={openLogout}
          />
        </div>
      </div>

      <ConfirmDialog
        open={confirmOpen}
        title="Log out of RustWA?"
        body="This unlinks this device from your WhatsApp account. You will need to scan the QR code again to use RustWA."
        confirmLabel="Log out"
        danger
        busy={busy}
        error={error}
        onCancel={cancelLogout}
        onConfirm={logOut}
      />
    </section>
  );
}
