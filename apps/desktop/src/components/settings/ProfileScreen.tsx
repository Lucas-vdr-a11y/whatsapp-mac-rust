import { useEffect, useState, type CSSProperties } from "react";
import {
  Camera,
  LoaderCircle,
  LogOut,
  MessageCircle,
  Pencil,
  Trash2,
  UserRound,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { avatarSrc } from "../../lib/avatar";
import { useTranslation } from "../../lib/i18n";
import { invokeCore, isTauri } from "../../lib/ipc";
import { initials } from "../../lib/names";
import { useAppStore } from "../../store/app";
import { ScreenHeader } from "../screens/shared";
import { ConfirmDialog } from "./ConfirmDialog";
import { SettingsRow } from "./SettingsRow";
import { errorMessage } from "./util";

/** Push-name limit WhatsApp enforces; mirrors `PUSH_NAME_MAX_CHARS` in the core. */
const NAME_MAX = 25;
/** About-text limit WhatsApp enforces; mirrors `ABOUT_MAX_CHARS` in the core. */
const ABOUT_MAX = 139;

/** Payload of `profile_get` (camelCase over IPC). */
interface OwnProfile {
  jid: string | null;
  pushName: string | null;
  about: string | null;
  avatarUrl: string | null;
}

type ProfileState =
  | { status: "loading" }
  | { status: "ready"; value: OwnProfile }
  | { status: "unavailable"; message: string };

type Feedback = { kind: "ok" | "error"; text: string };

const INLINE_BUTTON: CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: 6,
};

const EDIT_INPUT: CSSProperties = {
  width: 150,
  height: 30,
  padding: "0 10px",
  border: "1px solid var(--divider)",
  borderRadius: "var(--radius-md)",
  background: "var(--bg-input)",
  color: "var(--text-primary)",
  fontSize: 13,
};

const EDIT_INPUT_WIDE: CSSProperties = { ...EDIT_INPUT, width: 200 };

/** Counts Unicode characters the way the core validates them. */
function charCount(value: string): number {
  return Array.from(value).length;
}

/** Phone number for display: the user part of the JID. */
function jidDisplay(jid: string): string {
  const user = jid.split("@")[0];
  return user || jid;
}

/** Own-profile editing plus linked-account status and quick actions. */
export function ProfileScreen() {
  const { t } = useTranslation();
  const connection = useAppStore((state) => state.connection);
  const paired = useAppStore((state) => state.paired);
  const [profile, setProfile] = useState<ProfileState>({ status: "loading" });
  const [reloadToken, setReloadToken] = useState(0);
  const [editing, setEditing] = useState<"name" | "about" | null>(null);
  const [nameDraft, setNameDraft] = useState("");
  const [aboutDraft, setAboutDraft] = useState("");
  const [busy, setBusy] = useState<
    "name" | "about" | "photo" | "remove" | null
  >(null);
  const [notice, setNotice] = useState<Feedback | null>(null);
  const [photoConfirmOpen, setPhotoConfirmOpen] = useState(false);
  const [logoutOpen, setLogoutOpen] = useState(false);
  const [logoutBusy, setLogoutBusy] = useState(false);
  const [logoutError, setLogoutError] = useState<string | null>(null);

  // Load (or retry) the own profile. Re-runs when linking completes.
  useEffect(() => {
    if (!paired) {
      setProfile({ status: "unavailable", message: t("profile.notLinked") });
      return;
    }
    if (!isTauri()) {
      setProfile({
        status: "unavailable",
        message: t("profile.editOnlyDesktop"),
      });
      return;
    }
    let cancelled = false;
    setProfile({ status: "loading" });
    invokeCore<OwnProfile>("profile_get")
      .then((value) => {
        if (!cancelled) setProfile({ status: "ready", value });
      })
      .catch((cause) => {
        if (!cancelled) {
          setProfile({ status: "unavailable", message: errorMessage(cause) });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [paired, reloadToken, t]);

  const value = profile.status === "ready" ? profile.value : null;
  const canEdit = profile.status === "ready" && busy === null && editing === null;

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
    setLogoutError(null);
    setLogoutOpen(true);
  };

  const cancelLogout = () => {
    if (logoutBusy) return;
    setLogoutOpen(false);
    setLogoutError(null);
  };

  const logOut = () => {
    if (!isTauri()) {
      setLogoutError(t("profile.logOutOnlyDesktop"));
      return;
    }
    setLogoutBusy(true);
    setLogoutError(null);
    void invokeCore("core_logout")
      .then(() => {
        // Reloading intentionally clears all in-memory UI state (selected
        // chat, cached messages, store data); the Rust core owns the session.
        window.location.reload();
      })
      .catch((cause) => {
        setLogoutError(errorMessage(cause));
        setLogoutBusy(false);
      });
  };

  const startNameEdit = () => {
    if (!canEdit) return;
    setNotice(null);
    setNameDraft(value?.pushName ?? "");
    setEditing("name");
  };

  const startAboutEdit = () => {
    if (!canEdit) return;
    setNotice(null);
    setAboutDraft(value?.about ?? "");
    setEditing("about");
  };

  const cancelEdit = () => {
    if (busy !== null) return;
    setEditing(null);
  };

  const trimmedName = nameDraft.trim();
  const nameLength = charCount(nameDraft);
  const nameValid = trimmedName.length > 0 && nameLength <= NAME_MAX;
  const aboutLength = charCount(aboutDraft);
  const aboutValid = aboutLength <= ABOUT_MAX;

  const saveName = () => {
    if (!nameValid || busy !== null) return;
    setBusy("name");
    setNotice(null);
    invokeCore("profile_set_name", { name: trimmedName })
      .then(() => {
        setProfile((current) =>
          current.status === "ready"
            ? {
                status: "ready",
                value: { ...current.value, pushName: trimmedName },
              }
            : current,
        );
        setEditing(null);
        setNotice({ kind: "ok", text: t("profile.nameUpdated") });
      })
      .catch((cause) => setNotice({ kind: "error", text: errorMessage(cause) }))
      .finally(() => setBusy(null));
  };

  const saveAbout = () => {
    if (!aboutValid || busy !== null) return;
    const text = aboutDraft.trim();
    setBusy("about");
    setNotice(null);
    invokeCore("profile_set_about", { text })
      .then(() => {
        setProfile((current) =>
          current.status === "ready"
            ? {
                status: "ready",
                value: { ...current.value, about: text.length > 0 ? text : null },
              }
            : current,
        );
        setEditing(null);
        setNotice({ kind: "ok", text: t("profile.aboutUpdated") });
      })
      .catch((cause) => setNotice({ kind: "error", text: errorMessage(cause) }))
      .finally(() => setBusy(null));
  };

  const pickPhoto = () => {
    if (!canEdit) return;
    setNotice(null);
    void open({
      title: t("profile.pickPhotoTitle"),
      multiple: false,
      filters: [{ name: t("profile.photoFilter"), extensions: ["jpg", "jpeg"] }],
    })
      .then(async (picked) => {
        if (typeof picked !== "string") return;
        setBusy("photo");
        try {
          await invokeCore("profile_set_picture", { path: picked });
          setNotice({ kind: "ok", text: t("profile.photoUpdated") });
          // The picture URL changes server-side; fetch the fresh one.
          setReloadToken((token) => token + 1);
        } catch (cause) {
          setNotice({ kind: "error", text: errorMessage(cause) });
        } finally {
          setBusy(null);
        }
      })
      .catch((cause) => setNotice({ kind: "error", text: errorMessage(cause) }));
  };

  const openRemovePhoto = () => {
    if (!canEdit) return;
    setNotice(null);
    setPhotoConfirmOpen(true);
  };

  const cancelRemovePhoto = () => {
    if (busy !== null) return;
    setPhotoConfirmOpen(false);
    setNotice(null);
  };

  const removePhoto = () => {
    if (busy !== null) return;
    setBusy("remove");
    setNotice(null);
    invokeCore("profile_remove_picture")
      .then(() => {
        setProfile((current) =>
          current.status === "ready"
            ? { status: "ready", value: { ...current.value, avatarUrl: null } }
            : current,
        );
        setPhotoConfirmOpen(false);
        setNotice({ kind: "ok", text: t("profile.photoRemoved") });
      })
      .catch((cause) => setNotice({ kind: "error", text: errorMessage(cause) }))
      .finally(() => setBusy(null));
  };

  const displayName = value?.pushName ?? t("profile.you");

  return (
    <section className="chat-list screen">
      <ScreenHeader title={t("profile.title")} />

      <div className="screen-body">
        <div className="profile-hero">
          <div className="avatar profile-avatar">
            {profile.status === "loading" ? (
              <LoaderCircle size={28} className="spin" aria-hidden="true" />
            ) : value?.avatarUrl ? (
              <img src={avatarSrc(value.avatarUrl)} alt={t("profile.avatarAlt")} />
            ) : (
              initials(displayName)
            )}
          </div>
          <p className="profile-name">{displayName}</p>
          <p className="profile-subtitle">
            {profile.status === "loading"
              ? t("common.loading")
              : profile.status === "unavailable"
                ? profile.message
                : (value?.about ?? t("profile.aboutEmpty"))}
          </p>
          <div style={{ display: "flex", gap: 8, marginTop: 4 }}>
            <button
              type="button"
              className="settings-button"
              style={INLINE_BUTTON}
              disabled={!canEdit}
              onClick={pickPhoto}
            >
              <Camera size={14} aria-hidden="true" />
              {busy === "photo"
                ? t("profile.uploadingPhoto")
                : t("profile.changePhoto")}
            </button>
            {value?.avatarUrl ? (
              <button
                type="button"
                className="settings-button"
                style={INLINE_BUTTON}
                disabled={!canEdit}
                onClick={openRemovePhoto}
              >
                <Trash2 size={14} aria-hidden="true" />
                {busy === "remove"
                  ? t("profile.removingPhoto")
                  : t("profile.removePhoto")}
              </button>
            ) : null}
          </div>
          {notice && !photoConfirmOpen ? (
            <p
              className={`profile-subtitle ${
                notice.kind === "ok"
                  ? "settings-inline-ok"
                  : "settings-inline-error"
              }`}
              role={notice.kind === "error" ? "alert" : undefined}
            >
              {notice.text}
            </p>
          ) : null}
          {profile.status === "unavailable" && paired && isTauri() ? (
            <button
              type="button"
              className="settings-button"
              onClick={() => setReloadToken((token) => token + 1)}
            >
              {t("common.retry")}
            </button>
          ) : null}
        </div>

        <h2 className="settings-section-title">{t("profile.details")}</h2>
        <div className="settings-group">
          <SettingsRow
            icon={<UserRound size={20} />}
            label={t("profile.name")}
            description={
              editing === "name" ? (
                trimmedName.length === 0 ? (
                  <span className="settings-inline-error">
                    {t("profile.nameRequired")}
                  </span>
                ) : nameLength > NAME_MAX ? (
                  <span className="settings-inline-error">
                    {t("profile.nameTooLong", { max: NAME_MAX })}
                  </span>
                ) : (
                  t("profile.nameCounter", { count: nameLength, max: NAME_MAX })
                )
              ) : (
                t("profile.nameDescription")
              )
            }
            control={
              editing === "name" ? (
                <>
                  <input
                    type="text"
                    value={nameDraft}
                    autoFocus
                    aria-label={t("profile.name")}
                    placeholder={t("profile.you")}
                    style={EDIT_INPUT}
                    onChange={(event) => setNameDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") saveName();
                      if (event.key === "Escape") cancelEdit();
                    }}
                  />
                  <button
                    type="button"
                    className="settings-button primary"
                    disabled={!nameValid || busy === "name"}
                    onClick={saveName}
                  >
                    {busy === "name" ? t("profile.saving") : t("profile.save")}
                  </button>
                  <button
                    type="button"
                    className="settings-button"
                    disabled={busy === "name"}
                    onClick={cancelEdit}
                  >
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <>
                  <span className="settings-value">
                    {value?.pushName ?? "—"}
                  </span>
                  <button
                    type="button"
                    className="settings-button"
                    style={INLINE_BUTTON}
                    disabled={!canEdit}
                    onClick={startNameEdit}
                  >
                    <Pencil size={14} aria-hidden="true" />
                    {t("profile.edit")}
                  </button>
                </>
              )
            }
          />
          <SettingsRow
            label={t("profile.about")}
            description={
              editing === "about" ? (
                aboutLength > ABOUT_MAX ? (
                  <span className="settings-inline-error">
                    {t("profile.aboutTooLong", { max: ABOUT_MAX })}
                  </span>
                ) : (
                  t("profile.aboutCounter", {
                    count: aboutLength,
                    max: ABOUT_MAX,
                  })
                )
              ) : (
                t("profile.aboutDescription")
              )
            }
            control={
              editing === "about" ? (
                <>
                  <input
                    type="text"
                    value={aboutDraft}
                    autoFocus
                    aria-label={t("profile.about")}
                    placeholder={t("profile.aboutPlaceholder")}
                    style={EDIT_INPUT_WIDE}
                    onChange={(event) => setAboutDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") saveAbout();
                      if (event.key === "Escape") cancelEdit();
                    }}
                  />
                  <button
                    type="button"
                    className="settings-button primary"
                    disabled={!aboutValid || busy === "about"}
                    onClick={saveAbout}
                  >
                    {busy === "about" ? t("profile.saving") : t("profile.save")}
                  </button>
                  <button
                    type="button"
                    className="settings-button"
                    disabled={busy === "about"}
                    onClick={cancelEdit}
                  >
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <>
                  <span className="settings-value">{value?.about ?? "—"}</span>
                  <button
                    type="button"
                    className="settings-button"
                    style={INLINE_BUTTON}
                    disabled={!canEdit}
                    onClick={startAboutEdit}
                  >
                    <Pencil size={14} aria-hidden="true" />
                    {t("profile.edit")}
                  </button>
                </>
              )
            }
          />
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
          <SettingsRow
            label={t("profile.phoneNumber")}
            description={t("profile.phoneDescription")}
            control={
              <span className="settings-value">
                {value?.jid ? jidDisplay(value.jid) : "—"}
              </span>
            }
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
        open={photoConfirmOpen}
        title={t("profile.removePhotoTitle")}
        body={t("profile.removePhotoBody")}
        confirmLabel={t("profile.removePhoto")}
        danger
        busy={busy === "remove"}
        error={notice?.kind === "error" ? notice.text : null}
        onCancel={cancelRemovePhoto}
        onConfirm={removePhoto}
      />

      <ConfirmDialog
        open={logoutOpen}
        title={t("settings.logOutTitle")}
        body={t("settings.logOutBody")}
        confirmLabel={t("settings.logOutConfirm")}
        danger
        busy={logoutBusy}
        error={logoutError}
        onCancel={cancelLogout}
        onConfirm={logOut}
      />
    </section>
  );
}
