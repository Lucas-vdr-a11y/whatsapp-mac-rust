/**
 * "New chat" modal: search across a local contact list and start a chat.
 *
 * The contact list is mock data for browser mode; in Tauri builds the core
 * will provide the real address book later.
 */

import { useEffect, useMemo, useState } from "react";
import { Search, Users, X } from "lucide-react";
import { initials } from "../lib/names";
import { useAppStore } from "../store/app";

interface NewChatModalProps {
  onClose: () => void;
}

interface Contact {
  id: string;
  name: string;
  about: string;
}

const CONTACTS: Contact[] = [
  { id: "alice@s.whatsapp.net", name: "Alice", about: "See you tomorrow!" },
  { id: "bob@s.whatsapp.net", name: "Bob", about: "Thanks for the update" },
  { id: "marieke@s.whatsapp.net", name: "Marieke", about: "Haha that's great" },
  { id: "sander@s.whatsapp.net", name: "Sander", about: "I'll bring the tent" },
  { id: "daan@s.whatsapp.net", name: "Daan", about: "Chapter 12 next week?" },
  { id: "lotte@s.whatsapp.net", name: "Lotte", about: "At the office" },
  { id: "jeroen@s.whatsapp.net", name: "Jeroen", about: "Can't talk, driving" },
  { id: "sophie@s.whatsapp.net", name: "Sophie", about: "Available" },
  { id: "tom@s.whatsapp.net", name: "Tom", about: "Busy right now" },
  { id: "nadia@s.whatsapp.net", name: "Nadia", about: "Coffee later?" },
];

export function NewChatModal({ onClose }: NewChatModalProps) {
  const [query, setQuery] = useState("");
  const startChat = useAppStore((state) => state.startChat);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return CONTACTS;
    return CONTACTS.filter(
      (contact) =>
        contact.name.toLowerCase().includes(needle) ||
        contact.about.toLowerCase().includes(needle),
    );
  }, [query]);

  const openContact = (contact: Contact) => {
    startChat(contact.id, contact.name);
    onClose();
  };

  const openNewGroup = () => {
    startChat(`group-${Date.now()}@g.us`, "New group", true);
    onClose();
  };

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div
        className="new-chat-modal"
        role="dialog"
        aria-modal="true"
        aria-label="New chat"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <h2 className="modal-title">New chat</h2>
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
          <button
            type="button"
            className="contact-row"
            onClick={openNewGroup}
          >
            <span className="contact-row-icon">
              <Users size={20} />
            </span>
            <span className="contact-row-body">
              <span className="contact-row-name">New group</span>
              <span className="contact-row-about">Add participants</span>
            </span>
          </button>

          <div className="modal-divider" role="separator" />

          {filtered.map((contact) => (
            <button
              key={contact.id}
              type="button"
              className="contact-row"
              onClick={() => openContact(contact)}
            >
              <span className="avatar small">{initials(contact.name)}</span>
              <span className="contact-row-body">
                <span className="contact-row-name">{contact.name}</span>
                <span className="contact-row-about">{contact.about}</span>
              </span>
            </button>
          ))}

          {filtered.length === 0 && (
            <p className="modal-empty">No contacts found</p>
          )}
        </div>
      </div>
    </div>
  );
}
