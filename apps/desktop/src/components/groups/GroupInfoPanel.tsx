/**
 * Right-side group info panel.
 *
 * Renders standalone: given a group `chatId` it fetches `groups_info` and
 * offers the invite link, participant management and leaving the group.
 * In browser mock mode the API layer resolves with demo data.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  Link2,
  LoaderCircle,
  LogOut,
  Pencil,
  RefreshCw,
  Search,
  UserCheck,
  UserMinus,
  UserPlus,
  UserX,
  X,
} from "lucide-react";
import { initials } from "../../lib/names";
import { useTranslation } from "../../lib/i18n";
import { isTauri } from "../../lib/ipc";
import { useAppStore } from "../../store/app";
import { ContactList, jidLabel } from "./ContactList";
import { ConfirmDialog } from "./ConfirmDialog";
import {
  addParticipants,
  approveJoinRequests,
  copyToClipboard,
  fetchGroupInfo,
  fetchInviteLink,
  fetchJoinRequests,
  friendlyError,
  leaveGroup,
  rejectJoinRequests,
  removeParticipants,
  resetInviteLink,
  setGroupSubject,
  useContacts,
} from "./api";
import type { Contact, GroupInfo, JoinRequest } from "./types";

interface GroupInfoPanelProps {
  chatId: string;
  onClose: () => void;
}

type InviteState = "idle" | "loading" | "copied";
type ResetState = "idle" | "loading" | "done";

export function GroupInfoPanel({ chatId, onClose }: GroupInfoPanelProps) {
  const { t } = useTranslation();
  const [info, setInfo] = useState<GroupInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  const [inviteState, setInviteState] = useState<InviteState>("idle");
  const [resetState, setResetState] = useState<ResetState>("idle");
  const [joinRequests, setJoinRequests] = useState<JoinRequest[]>([]);
  const [requestBusy, setRequestBusy] = useState<{
    jid: string;
    action: "approve" | "reject";
  } | null>(null);
  const [subjectEditing, setSubjectEditing] = useState(false);
  const [subjectDraft, setSubjectDraft] = useState("");
  const [subjectSaving, setSubjectSaving] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [addQuery, setAddQuery] = useState("");
  const [addSelection, setAddSelection] = useState<string[]>([]);
  const [adding, setAdding] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [leaveOpen, setLeaveOpen] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const copiedTimer = useRef<number | null>(null);
  const resetTimer = useRef<number | null>(null);

  const {
    contacts,
    error: contactsError,
    reload: reloadContacts,
  } = useContacts();

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(null);
    void fetchGroupInfo(chatId)
      .then((loaded) => {
        if (!cancelled) setInfo(loaded);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setLoadError(friendlyError(cause, t("groups.loadError")));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [chatId, reloadToken]);

  // Join requests load separately: groups without membership approval (or
  // where this account is not an admin) simply have none, and a failure here
  // must not block the rest of the panel.
  useEffect(() => {
    let cancelled = false;
    setJoinRequests([]);
    setSubjectEditing(false);
    setResetState("idle");
    void fetchJoinRequests(chatId)
      .then((requests) => {
        if (!cancelled) setJoinRequests(requests);
      })
      .catch(() => {
        // Keep the section hidden; group info still renders.
      });
    return () => {
      cancelled = true;
    };
  }, [chatId, reloadToken]);

  // Clear the confirmation timers if the panel unmounts mid-feedback.
  useEffect(
    () => () => {
      if (copiedTimer.current !== null) {
        window.clearTimeout(copiedTimer.current);
      }
      if (resetTimer.current !== null) {
        window.clearTimeout(resetTimer.current);
      }
    },
    [],
  );

  const contactNames = useMemo(() => {
    const names = new Map<string, string>();
    contacts.forEach((contact) => names.set(contact.id, contact.name));
    return names;
  }, [contacts]);

  const participantSet = useMemo(
    () => new Set(info?.participants ?? []),
    [info],
  );

  const availableContacts = useMemo(
    () => contacts.filter((contact) => !participantSet.has(contact.id)),
    [contacts, participantSet],
  );

  const filteredContacts = useMemo(() => {
    const needle = addQuery.trim().toLowerCase();
    if (!needle) return availableContacts;
    return availableContacts.filter((contact) =>
      contact.name.toLowerCase().includes(needle),
    );
  }, [availableContacts, addQuery]);

  const displayName = (jid: string) => contactNames.get(jid) ?? jidLabel(jid);

  const handleInvite = async () => {
    if (inviteState === "loading") return;
    setInviteState("loading");
    setActionError(null);
    try {
      const link = await fetchInviteLink(chatId);
      await copyToClipboard(link);
      setInviteState("copied");
      if (copiedTimer.current !== null) {
        window.clearTimeout(copiedTimer.current);
      }
      copiedTimer.current = window.setTimeout(() => {
        copiedTimer.current = null;
        setInviteState("idle");
      }, 2000);
    } catch (cause) {
      setInviteState("idle");
      setActionError(friendlyError(cause, t("groups.inviteError")));
    }
  };

  const handleReset = async () => {
    if (resetState === "loading") return;
    setResetState("loading");
    setActionError(null);
    try {
      const link = await resetInviteLink(chatId);
      await copyToClipboard(link);
      setResetState("done");
      if (resetTimer.current !== null) {
        window.clearTimeout(resetTimer.current);
      }
      resetTimer.current = window.setTimeout(() => {
        resetTimer.current = null;
        setResetState("idle");
      }, 2000);
    } catch (cause) {
      setResetState("idle");
      setActionError(friendlyError(cause, t("groups.resetInviteError")));
    }
  };

  const startSubjectEdit = () => {
    if (!info) return;
    setSubjectDraft(info.subject);
    setSubjectEditing(true);
    setActionError(null);
  };

  const cancelSubjectEdit = () => {
    setSubjectEditing(false);
    setSubjectDraft("");
  };

  const saveSubject = async () => {
    if (!info || subjectSaving) return;
    const subject = subjectDraft.trim();
    if (!subject) {
      setActionError(t("groups.subjectEmpty"));
      return;
    }
    if (subject === info.subject) {
      cancelSubjectEdit();
      return;
    }
    setSubjectSaving(true);
    setActionError(null);
    try {
      await setGroupSubject(chatId, subject);
      setInfo((current) => (current ? { ...current, subject } : current));
      cancelSubjectEdit();
    } catch (cause) {
      setActionError(friendlyError(cause, t("groups.subjectError")));
    } finally {
      setSubjectSaving(false);
    }
  };

  const handleApprove = async (jid: string) => {
    if (requestBusy !== null) return;
    setRequestBusy({ jid, action: "approve" });
    setActionError(null);
    try {
      await approveJoinRequests(chatId, [jid]);
      setJoinRequests((current) =>
        current.filter((request) => request.id !== jid),
      );
      setInfo((current) =>
        current && !current.participants.includes(jid)
          ? {
              ...current,
              participants: [...current.participants, jid],
              participantCount: current.participantCount + 1,
            }
          : current,
      );
    } catch (cause) {
      setActionError(friendlyError(cause, t("groups.joinRequestError")));
    } finally {
      setRequestBusy(null);
    }
  };

  const handleReject = async (jid: string) => {
    if (requestBusy !== null) return;
    setRequestBusy({ jid, action: "reject" });
    setActionError(null);
    try {
      await rejectJoinRequests(chatId, [jid]);
      setJoinRequests((current) =>
        current.filter((request) => request.id !== jid),
      );
    } catch (cause) {
      setActionError(friendlyError(cause, t("groups.joinRequestError")));
    } finally {
      setRequestBusy(null);
    }
  };

  const toggleAddContact = (contact: Contact) => {
    setAddSelection((current) =>
      current.includes(contact.id)
        ? current.filter((id) => id !== contact.id)
        : [...current, contact.id],
    );
  };

  const closePicker = () => {
    setAddOpen(false);
    setAddSelection([]);
    setAddQuery("");
  };

  const handleAdd = async () => {
    if (addSelection.length === 0 || adding) return;
    setAdding(true);
    setActionError(null);
    try {
      await addParticipants(chatId, addSelection);
      setInfo((current) =>
        current
          ? {
              ...current,
              participants: [...current.participants, ...addSelection],
              participantCount:
                current.participantCount + addSelection.length,
            }
          : current,
      );
      closePicker();
    } catch (cause) {
      setActionError(friendlyError(cause, t("groups.addError")));
    } finally {
      setAdding(false);
    }
  };

  const handleRemove = async (jid: string) => {
    if (removingId !== null) return;
    setRemovingId(jid);
    setActionError(null);
    try {
      await removeParticipants(chatId, [jid]);
      setInfo((current) =>
        current
          ? {
              ...current,
              participants: current.participants.filter(
                (participant) => participant !== jid,
              ),
              participantCount: Math.max(0, current.participantCount - 1),
            }
          : current,
      );
    } catch (cause) {
      setActionError(friendlyError(cause, t("groups.removeError")));
    } finally {
      setRemovingId(null);
    }
  };

  const handleLeave = async () => {
    if (leaving) return;
    setLeaving(true);
    setActionError(null);
    try {
      await leaveGroup(chatId);
      // Browser mock mode has no core to update the chat list; mirror the
      // leave locally. Inside Tauri the core's chat update owns the list.
      if (!isTauri()) {
        useAppStore.getState().deleteChat(chatId);
      }
      setLeaveOpen(false);
      onClose();
    } catch (cause) {
      setLeaveOpen(false);
      setActionError(friendlyError(cause, t("groups.leaveError")));
    } finally {
      setLeaving(false);
    }
  };

  const showCopied = inviteState === "copied";

  return (
    <aside className="group-info-panel" aria-label={t("groups.info")}>
      <header className="group-info-header" data-tauri-drag-region>
        <h2 className="group-info-title">{t("groups.info")}</h2>
        <button
          type="button"
          className="icon-button no-drag"
          title={t("common.close")}
          aria-label={t("groups.closeAria")}
          onClick={onClose}
        >
          <X size={22} />
        </button>
      </header>

      <div className="group-info-scroll">
        {loading ? (
          <GroupInfoSkeleton />
        ) : loadError ? (
          <div className="group-info-error">
            <p>{loadError}</p>
            <button
              type="button"
              className="modal-action secondary"
              onClick={() => setReloadToken((value) => value + 1)}
            >
              {t("common.tryAgain")}
            </button>
          </div>
        ) : info ? (
          <>
            <section className="group-info-hero">
              <div className="avatar group-avatar">{initials(info.subject)}</div>
              {subjectEditing ? (
                <form
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    width: "100%",
                    marginTop: 12,
                  }}
                  onSubmit={(event) => {
                    event.preventDefault();
                    void saveSubject();
                  }}
                >
                  <label
                    className="modal-search"
                    style={{ flex: 1, margin: 0 }}
                  >
                    <input
                      type="text"
                      value={subjectDraft}
                      autoFocus
                      maxLength={100}
                      placeholder={t("groups.subjectPlaceholder")}
                      aria-label={t("groups.editSubject")}
                      onChange={(event) => setSubjectDraft(event.target.value)}
                    />
                  </label>
                  <button
                    type="submit"
                    className="modal-action primary"
                    disabled={subjectSaving || subjectDraft.trim().length === 0}
                    title={t("groups.saveSubject")}
                    aria-label={t("groups.saveSubject")}
                  >
                    {subjectSaving ? (
                      <LoaderCircle size={16} className="spin" />
                    ) : (
                      <Check size={16} />
                    )}
                  </button>
                  <button
                    type="button"
                    className="modal-action secondary"
                    disabled={subjectSaving}
                    title={t("common.cancel")}
                    aria-label={t("common.cancel")}
                    onClick={cancelSubjectEdit}
                  >
                    <X size={16} />
                  </button>
                </form>
              ) : (
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    marginTop: 12,
                  }}
                >
                  <h3 className="group-info-subject" style={{ margin: 0 }}>
                    {info.subject}
                  </h3>
                  <button
                    type="button"
                    className="icon-button"
                    title={t("groups.editSubject")}
                    aria-label={t("groups.editSubject")}
                    onClick={startSubjectEdit}
                  >
                    <Pencil size={15} />
                  </button>
                </div>
              )}
              <p className="group-info-count">
                {info.participantCount === 1
                  ? t("groups.participantOne")
                  : t("groups.participants", {
                      count: info.participantCount,
                    })}
              </p>
            </section>

            {actionError ? (
              <p className="group-inline-error" role="alert">
                {actionError}
              </p>
            ) : null}

            <section className="group-info-section">
              <button
                type="button"
                className="group-info-row"
                disabled={inviteState === "loading"}
                onClick={() => void handleInvite()}
              >
                <span className="group-info-row-icon">
                  {showCopied ? <Check size={18} /> : <Link2 size={18} />}
                </span>
                <span className="group-info-row-label">
                  {showCopied ? t("groups.linkCopied") : t("groups.inviteLink")}
                </span>
                <span className="group-info-row-value">
                  {inviteState === "loading" ? (
                    <LoaderCircle size={16} className="spin" />
                  ) : showCopied ? (
                    t("groups.copied")
                  ) : (
                    t("groups.copy")
                  )}
                </span>
              </button>
              <button
                type="button"
                className="group-info-row"
                disabled={resetState === "loading"}
                onClick={() => void handleReset()}
              >
                <span className="group-info-row-icon">
                  {resetState === "done" ? (
                    <Check size={18} />
                  ) : (
                    <RefreshCw size={18} />
                  )}
                </span>
                <span className="group-info-row-label">
                  {resetState === "done"
                    ? t("groups.linkReset")
                    : t("groups.resetLink")}
                </span>
                <span className="group-info-row-value">
                  {resetState === "loading" ? (
                    <LoaderCircle size={16} className="spin" />
                  ) : (
                    t("groups.reset")
                  )}
                </span>
              </button>
            </section>

            {joinRequests.length > 0 ? (
              <section className="group-info-section">
                <header className="group-info-section-header">
                  <span>
                    {t("groups.joinRequests", { count: joinRequests.length })}
                  </span>
                </header>
                <div className="member-list">
                  {joinRequests.map((request) => (
                    <div key={request.id} className="member-row">
                      <span className="avatar small">
                        {initials(displayName(request.id))}
                      </span>
                      <span className="member-body">
                        <span className="member-name">
                          {displayName(request.id)}
                        </span>
                      </span>
                      <button
                        type="button"
                        className="group-info-text-button"
                        title={t("groups.approve")}
                        aria-label={t("groups.approveAria", {
                          name: displayName(request.id),
                        })}
                        disabled={requestBusy !== null}
                        onClick={() => void handleApprove(request.id)}
                      >
                        {requestBusy?.jid === request.id &&
                        requestBusy.action === "approve" ? (
                          <LoaderCircle size={16} className="spin" />
                        ) : (
                          <UserCheck size={16} />
                        )}
                      </button>
                      <button
                        type="button"
                        className="group-info-text-button"
                        style={{ color: "var(--danger)" }}
                        title={t("groups.reject")}
                        aria-label={t("groups.rejectAria", {
                          name: displayName(request.id),
                        })}
                        disabled={requestBusy !== null}
                        onClick={() => void handleReject(request.id)}
                      >
                        {requestBusy?.jid === request.id &&
                        requestBusy.action === "reject" ? (
                          <LoaderCircle size={16} className="spin" />
                        ) : (
                          <UserX size={16} />
                        )}
                      </button>
                    </div>
                  ))}
                </div>
              </section>
            ) : null}

            <section className="group-info-section">
              <header className="group-info-section-header">
                <span>
                  {info.participantCount === 1
                    ? t("groups.memberOne")
                    : t("groups.members", { count: info.participantCount })}
                </span>
                <button
                  type="button"
                  className="group-info-text-button"
                  aria-expanded={addOpen}
                  onClick={() => {
                    if (addOpen) closePicker();
                    else setAddOpen(true);
                  }}
                >
                  <UserPlus size={16} />
                  {t("groups.addParticipants")}
                </button>
              </header>

              {addOpen ? (
                <div className="participant-picker">
                  <label className="modal-search participant-search">
                    <Search size={16} />
                    <input
                      type="text"
                      placeholder={t("groups.searchContacts")}
                      value={addQuery}
                      autoFocus
                      onChange={(event) => setAddQuery(event.target.value)}
                    />
                  </label>
                  <div className="participant-picker-list">
                    {contactsError ? (
                      <div className="participant-picker-error">
                        <p>{contactsError}</p>
                        <button
                          type="button"
                          className="modal-action secondary"
                          onClick={reloadContacts}
                        >
                          {t("common.retry")}
                        </button>
                      </div>
                    ) : (
                      <ContactList
                        contacts={filteredContacts}
                        selectedIds={new Set(addSelection)}
                        onToggle={toggleAddContact}
                        emptyText={
                          availableContacts.length === 0
                            ? t("groups.everyoneIn")
                            : t("groups.noContacts")
                        }
                      />
                    )}
                  </div>
                  <div className="participant-picker-footer">
                    <button
                      type="button"
                      className="modal-action secondary"
                      onClick={closePicker}
                    >
                      {t("common.cancel")}
                    </button>
                    <button
                      type="button"
                      className="modal-action primary"
                      disabled={addSelection.length === 0 || adding}
                      onClick={() => void handleAdd()}
                    >
                      {adding
                        ? t("groups.adding")
                        : addSelection.length > 0
                          ? t("groups.addN", { count: addSelection.length })
                          : t("groups.add")}
                    </button>
                  </div>
                </div>
              ) : null}

              <div className="member-list">
                {info.participants.map((jid) => (
                  <div key={jid} className="member-row">
                    <span className="avatar small">
                      {initials(displayName(jid))}
                    </span>
                    <span className="member-body">
                      <span className="member-name">{displayName(jid)}</span>
                    </span>
                    <button
                      type="button"
                      className={
                        removingId === jid ? "member-remove busy" : "member-remove"
                      }
                      title={t("groups.removeFromGroup")}
                      aria-label={t("groups.removeAria", {
                        name: displayName(jid),
                      })}
                      disabled={removingId !== null}
                      onClick={() => void handleRemove(jid)}
                    >
                      {removingId === jid ? (
                        <LoaderCircle size={16} className="spin" />
                      ) : (
                        <UserMinus size={16} />
                      )}
                    </button>
                  </div>
                ))}
                {info.participants.length === 0 ? (
                  <p className="modal-empty">{t("groups.noParticipants")}</p>
                ) : null}
              </div>
            </section>

            <section className="group-info-section group-info-danger">
              <button
                type="button"
                className="group-info-row danger"
                onClick={() => setLeaveOpen(true)}
              >
                <span className="group-info-row-icon">
                  <LogOut size={18} />
                </span>
                <span className="group-info-row-label">{t("groups.leave")}</span>
              </button>
            </section>
          </>
        ) : null}
      </div>

      {leaveOpen ? (
        <ConfirmDialog
          title={t("groups.leaveTitle")}
          body={t("groups.leaveBody", {
            name: info?.subject ?? t("groups.title"),
          })}
          confirmLabel={t("groups.leaveConfirm")}
          danger
          busy={leaving}
          onConfirm={() => void handleLeave()}
          onCancel={() => {
            if (!leaving) setLeaveOpen(false);
          }}
        />
      ) : null}
    </aside>
  );
}

function GroupInfoSkeleton() {
  return (
    <div className="group-info-skeleton" aria-hidden="true">
      <div className="group-skeleton group-skeleton-avatar" />
      <div className="group-skeleton group-skeleton-line" />
      <div className="group-skeleton group-skeleton-line short" />
      <div className="group-skeleton group-skeleton-block" />
      <div className="group-skeleton group-skeleton-line" />
    </div>
  );
}
