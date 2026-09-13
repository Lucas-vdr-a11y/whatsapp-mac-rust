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
  Search,
  UserMinus,
  UserPlus,
  X,
} from "lucide-react";
import { initials } from "../../lib/names";
import { isTauri } from "../../lib/ipc";
import { useAppStore } from "../../store/app";
import { ContactList, jidLabel } from "./ContactList";
import { ConfirmDialog } from "./ConfirmDialog";
import {
  addParticipants,
  copyToClipboard,
  fetchGroupInfo,
  fetchInviteLink,
  friendlyError,
  leaveGroup,
  removeParticipants,
  useContacts,
} from "./api";
import type { Contact, GroupInfo } from "./types";

interface GroupInfoPanelProps {
  chatId: string;
  onClose: () => void;
}

type InviteState = "idle" | "loading" | "copied";

export function GroupInfoPanel({ chatId, onClose }: GroupInfoPanelProps) {
  const [info, setInfo] = useState<GroupInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  const [inviteState, setInviteState] = useState<InviteState>("idle");
  const [actionError, setActionError] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [addQuery, setAddQuery] = useState("");
  const [addSelection, setAddSelection] = useState<string[]>([]);
  const [adding, setAdding] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [leaveOpen, setLeaveOpen] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const copiedTimer = useRef<number | null>(null);

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
          setLoadError(friendlyError(cause, "Couldn't load group info."));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [chatId, reloadToken]);

  // Clear the "Copied" timer if the panel unmounts mid-confirmation.
  useEffect(
    () => () => {
      if (copiedTimer.current !== null) {
        window.clearTimeout(copiedTimer.current);
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
      setActionError(friendlyError(cause, "Couldn't fetch the invite link."));
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
      setActionError(friendlyError(cause, "Couldn't add participants."));
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
      setActionError(friendlyError(cause, "Couldn't remove the participant."));
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
      setActionError(friendlyError(cause, "Couldn't leave the group."));
    } finally {
      setLeaving(false);
    }
  };

  const showCopied = inviteState === "copied";

  return (
    <aside className="group-info-panel" aria-label="Group info">
      <header className="group-info-header" data-tauri-drag-region>
        <h2 className="group-info-title">Group info</h2>
        <button
          type="button"
          className="icon-button no-drag"
          title="Close"
          aria-label="Close group info"
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
              Try again
            </button>
          </div>
        ) : info ? (
          <>
            <section className="group-info-hero">
              <div className="avatar group-avatar">{initials(info.subject)}</div>
              <h3 className="group-info-subject">{info.subject}</h3>
              <p className="group-info-count">
                {info.participantCount === 1
                  ? "1 participant"
                  : `${info.participantCount} participants`}
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
                  {showCopied ? "Link copied" : "Invite link"}
                </span>
                <span className="group-info-row-value">
                  {inviteState === "loading" ? (
                    <LoaderCircle size={16} className="spin" />
                  ) : showCopied ? (
                    "Copied"
                  ) : (
                    "Copy"
                  )}
                </span>
              </button>
            </section>

            <section className="group-info-section">
              <header className="group-info-section-header">
                <span>
                  {info.participantCount === 1
                    ? "1 member"
                    : `${info.participantCount} members`}
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
                  Add participants
                </button>
              </header>

              {addOpen ? (
                <div className="participant-picker">
                  <label className="modal-search participant-search">
                    <Search size={16} />
                    <input
                      type="text"
                      placeholder="Search contacts"
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
                          Retry
                        </button>
                      </div>
                    ) : (
                      <ContactList
                        contacts={filteredContacts}
                        selectedIds={new Set(addSelection)}
                        onToggle={toggleAddContact}
                        emptyText={
                          availableContacts.length === 0
                            ? "Everyone is already in this group"
                            : "No contacts found"
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
                      Cancel
                    </button>
                    <button
                      type="button"
                      className="modal-action primary"
                      disabled={addSelection.length === 0 || adding}
                      onClick={() => void handleAdd()}
                    >
                      {adding
                        ? "Adding…"
                        : addSelection.length > 0
                          ? `Add (${addSelection.length})`
                          : "Add"}
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
                      title="Remove from group"
                      aria-label={`Remove ${displayName(jid)}`}
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
                  <p className="modal-empty">No participants listed</p>
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
                <span className="group-info-row-label">Leave group</span>
              </button>
            </section>
          </>
        ) : null}
      </div>

      {leaveOpen ? (
        <ConfirmDialog
          title="Leave group?"
          body={`You will no longer receive messages from ${
            info?.subject ?? "this group"
          }.`}
          confirmLabel="Leave group"
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
