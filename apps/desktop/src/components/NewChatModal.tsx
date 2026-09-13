/**
 * "New chat" modal: search contacts, start a one-to-one chat, or select several
 * contacts and create a group.
 *
 * Contacts come from `list_chats` inside the Tauri host and from demo data in
 * the browser; groups are created through `groups_create`. The second step
 * keeps the single-contact behaviour: one contact and no subject starts a
 * regular chat instead of creating a group.
 */

import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, Search, X } from "lucide-react";
import { initials } from "../lib/names";
import { useAppStore } from "../store/app";
import { ContactList } from "./groups/ContactList";
import { createGroup, friendlyError, useContacts } from "./groups/api";
import type { Contact } from "./groups/types";

interface NewChatModalProps {
  onClose: () => void;
}

type Step = "select" | "details";

export function NewChatModal({ onClose }: NewChatModalProps) {
  const startChat = useAppStore((state) => state.startChat);
  const {
    contacts,
    loading,
    error: contactsError,
    reload,
  } = useContacts();

  const [step, setStep] = useState<Step>("select");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Contact[]>([]);
  const [subject, setSubject] = useState("");
  const [creating, setCreating] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return contacts;
    return contacts.filter((contact) =>
      `${contact.name} ${contact.about ?? ""}`.toLowerCase().includes(needle),
    );
  }, [contacts, query]);

  const selectedIds = useMemo(
    () => new Set(selected.map((contact) => contact.id)),
    [selected],
  );

  const toggleContact = (contact: Contact) => {
    setFormError(null);
    setSelected((current) =>
      current.some((candidate) => candidate.id === contact.id)
        ? current.filter((candidate) => candidate.id !== contact.id)
        : [...current, contact],
    );
  };

  const startSingleChat = (contact: Contact) => {
    startChat(contact.id, contact.name);
    onClose();
  };

  const singleContact = selected.length === 1 ? selected[0] : null;
  const trimmedSubject = subject.trim();
  const primaryLabel =
    singleContact && !trimmedSubject ? "Start chat" : "Create group";
  const primaryDisabled =
    creating || selected.length === 0 || (!trimmedSubject && !singleContact);

  const handlePrimary = async () => {
    if (creating) return;

    // One contact and no subject keeps the old "start chat" behaviour.
    if (singleContact && !trimmedSubject) {
      startSingleChat(singleContact);
      return;
    }
    if (!trimmedSubject || selected.length === 0) return;

    setCreating(true);
    setFormError(null);
    try {
      const jid = await createGroup(
        trimmedSubject,
        selected.map((contact) => contact.id),
      );
      startChat(jid, trimmedSubject, true);
      // TODO(groups): the chat list still needs to be rehydrated from the core
      // after creation (`list_chats`). bridge.ts only rehydrates on connection
      // and `chatUpdated` events; the local `startChat` above keeps the new
      // group visible in the meantime. If the protocol does not emit a chat
      // update for the created group, the parent should refetch here.
      onClose();
    } catch (cause) {
      setFormError(friendlyError(cause, "Couldn't create the group."));
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div
        className="new-chat-modal"
        role="dialog"
        aria-modal="true"
        aria-label={step === "select" ? "New chat" : "New group"}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <h2 className="modal-title">
            {step === "select" ? "New chat" : "New group"}
          </h2>
          <button
            type="button"
            className="icon-button"
            title="Close"
            aria-label="Close"
            onClick={onClose}
          >
            <X size={22} />
          </button>
        </header>

        {step === "select" ? (
          <>
            {selected.length > 0 ? (
              <div className="selected-chips">
                {selected.map((contact) => (
                  <SelectedChip
                    key={contact.id}
                    contact={contact}
                    onRemove={() => toggleContact(contact)}
                  />
                ))}
              </div>
            ) : null}

            <label className="modal-search">
              <Search size={18} />
              <input
                type="text"
                placeholder="Search name"
                value={query}
                autoFocus
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>

            <div className="modal-list">
              {loading ? (
                <p className="modal-empty">Loading contacts…</p>
              ) : contactsError ? (
                <div className="modal-error">
                  <p>{contactsError}</p>
                  <button
                    type="button"
                    className="modal-action secondary"
                    onClick={reload}
                  >
                    Retry
                  </button>
                </div>
              ) : (
                <ContactList
                  contacts={filtered}
                  selectedIds={selectedIds}
                  onToggle={toggleContact}
                  emptyText="No contacts found"
                />
              )}
            </div>

            <footer className="new-chat-footer">
              <span className="new-chat-count">
                {selected.length === 0
                  ? "Select contacts"
                  : `${selected.length} selected`}
              </span>
              <div className="new-chat-actions">
                {singleContact ? (
                  <button
                    type="button"
                    className="modal-action secondary"
                    onClick={() => startSingleChat(singleContact)}
                  >
                    Start chat
                  </button>
                ) : null}
                <button
                  type="button"
                  className="modal-action primary"
                  disabled={selected.length === 0}
                  onClick={() => {
                    setStep("details");
                    setFormError(null);
                  }}
                >
                  Next
                </button>
              </div>
            </footer>
          </>
        ) : (
          <>
            <div className="new-chat-participants">
              <div className="selected-chips">
                {selected.map((contact) => (
                  <SelectedChip
                    key={contact.id}
                    contact={contact}
                    onRemove={() => toggleContact(contact)}
                  />
                ))}
              </div>

              <label className="subject-field">
                <span className="subject-label">Group subject</span>
                <input
                  type="text"
                  placeholder={
                    singleContact
                      ? "Optional — leave empty to start a chat"
                      : "Type a group name"
                  }
                  value={subject}
                  autoFocus
                  onChange={(event) => {
                    setSubject(event.target.value);
                    setFormError(null);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void handlePrimary();
                    }
                  }}
                />
              </label>
            </div>

            {formError ? (
              <p className="group-inline-error" role="alert">
                {formError}
              </p>
            ) : null}

            <div className="new-chat-spacer" />

            <footer className="new-chat-footer">
              <button
                type="button"
                className="modal-action secondary"
                onClick={() => {
                  setStep("select");
                  setFormError(null);
                }}
              >
                <ChevronLeft size={16} />
                Back
              </button>
              <button
                type="button"
                className="modal-action primary"
                disabled={primaryDisabled}
                onClick={() => void handlePrimary()}
              >
                {creating ? "Creating…" : primaryLabel}
              </button>
            </footer>
          </>
        )}
      </div>
    </div>
  );
}

function SelectedChip({
  contact,
  onRemove,
}: {
  contact: Contact;
  onRemove: () => void;
}) {
  return (
    <span className="contact-chip">
      <span className="avatar tiny">{initials(contact.name)}</span>
      <span className="contact-chip-name">{contact.name}</span>
      <button
        type="button"
        className="contact-chip-remove"
        aria-label={`Remove ${contact.name}`}
        onClick={onRemove}
      >
        <X size={14} />
      </button>
    </span>
  );
}
