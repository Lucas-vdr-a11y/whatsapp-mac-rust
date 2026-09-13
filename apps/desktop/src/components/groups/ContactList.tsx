/**
 * Contact rows shared by the new-chat modal and the group info panel's
 * participant picker. Rows toggle a selection and show a checkmark.
 */

import { Check } from "lucide-react";
import { initials } from "../../lib/names";
import type { Contact } from "./types";

/** Last-resort label for a JID that has no known contact name. */
export function jidLabel(jid: string): string {
  const [user] = jid.split("@");
  return user || jid;
}

interface ContactRowProps {
  contact: Contact;
  selected: boolean;
  onToggle: () => void;
}

export function ContactRow({ contact, selected, onToggle }: ContactRowProps) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      className={`contact-row${selected ? " selected" : ""}`}
      onClick={onToggle}
    >
      <span className="avatar small">{initials(contact.name)}</span>
      <span className="contact-row-body">
        <span className="contact-row-name">{contact.name}</span>
        <span className="contact-row-about">
          {contact.about ?? jidLabel(contact.id)}
        </span>
      </span>
      <span
        className={`contact-check${selected ? " checked" : ""}`}
        aria-hidden="true"
      >
        {selected ? <Check size={15} strokeWidth={3} /> : null}
      </span>
    </button>
  );
}

interface ContactListProps {
  contacts: Contact[];
  selectedIds: ReadonlySet<string>;
  onToggle: (contact: Contact) => void;
  emptyText: string;
}

export function ContactList({
  contacts,
  selectedIds,
  onToggle,
  emptyText,
}: ContactListProps) {
  if (contacts.length === 0) {
    return <p className="modal-empty">{emptyText}</p>;
  }

  return (
    <div className="contact-list" role="listbox" aria-multiselectable="true">
      {contacts.map((contact) => (
        <ContactRow
          key={contact.id}
          contact={contact}
          selected={selectedIds.has(contact.id)}
          onToggle={() => onToggle(contact)}
        />
      ))}
    </div>
  );
}
