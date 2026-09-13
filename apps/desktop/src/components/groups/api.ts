/**
 * Groups API and contact loading.
 *
 * Every core call for the groups UI goes through this module. Inside Tauri the
 * commands are invoked over IPC; in a plain browser (mock mode) the same
 * functions resolve with demo data so the UI stays reviewable before the core
 * is wired up. Failures are normalised into short, user-facing messages.
 */

import { useEffect, useState } from "react";
import { invokeCore, isTauri } from "../../lib/ipc";
import type { ChatSummary, Jid } from "../../lib/types";
import { useAppStore } from "../../store/app";
import type { Contact, GroupInfo } from "./types";

/** Demo address book used in browser mock mode. */
export const DEMO_CONTACTS: Contact[] = [
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

const MOCK_LATENCY_MS = 220;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

/** Small deterministic hash so mock data stays stable per chat. */
function hashString(value: string): number {
  let result = 0;
  for (let index = 0; index < value.length; index += 1) {
    result = (result * 31 + value.charCodeAt(index)) | 0;
  }
  return Math.abs(result);
}

/**
 * Normalises an IPC failure into a short message safe to show inline. Commands
 * that are not implemented yet (the groups workstream is still landing) get a
 * friendlier phrasing than the raw "not implemented" error.
 */
export function friendlyError(error: unknown, fallback: string): string {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "";
  if (!raw) return fallback;
  if (/not implemented|unknown command|command .* not found|unrecognized/i.test(raw)) {
    return "Group features aren't available in this build yet.";
  }
  return raw;
}

/** Copies text using the async clipboard API, falling back to execCommand. */
export async function copyToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const field = document.createElement("textarea");
  field.value = text;
  field.setAttribute("readonly", "");
  field.style.position = "fixed";
  field.style.top = "-1000px";
  document.body.appendChild(field);
  field.select();
  const copied = document.execCommand("copy");
  document.body.removeChild(field);
  if (!copied) throw new Error("Could not copy to the clipboard");
}

/** Loads direct-chat contacts: `list_chats` in Tauri, demo data elsewhere. */
export async function fetchContacts(): Promise<Contact[]> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return DEMO_CONTACTS;
  }

  const chats = await invokeCore<ChatSummary[]>("list_chats");
  return chats
    .filter((chat) => !chat.isGroup)
    .map((chat) => ({ id: chat.id, name: chat.name }));
}

export interface ContactsState {
  contacts: Contact[];
  loading: boolean;
  error: string | null;
  reload: () => void;
}

/** React hook around {@link fetchContacts} with retry support. */
export function useContacts(): ContactsState {
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    void fetchContacts()
      .then((loaded) => {
        if (!cancelled) setContacts(loaded);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(friendlyError(cause, "Couldn't load contacts."));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  return {
    contacts,
    loading,
    error,
    reload: () => setAttempt((value) => value + 1),
  };
}

/* ------------------------------------------------------------------ */
/* Group commands                                                      */
/* ------------------------------------------------------------------ */

/** Creates a group and resolves with the new group JID. */
export async function createGroup(
  subject: string,
  participants: Jid[],
): Promise<Jid> {
  if (!isTauri()) {
    await delay(400);
    return `group-${Date.now()}@g.us`;
  }
  return invokeCore<Jid>("groups_create", { subject, participants });
}

/** Fetches metadata for one group. */
export async function fetchGroupInfo(chatId: Jid): Promise<GroupInfo> {
  if (isTauri()) {
    return invokeCore<GroupInfo>("groups_info", { chatId });
  }

  await delay(MOCK_LATENCY_MS);
  const participants = DEMO_CONTACTS.filter(
    (contact) => contact.id !== chatId,
  )
    .slice(0, 3 + (hashString(chatId) % 4))
    .map((contact) => contact.id);
  const chat = useAppStore
    .getState()
    .chats.find((candidate) => candidate.id === chatId);

  return {
    id: chatId,
    subject: chat?.name ?? "Group",
    participants,
    participantCount: participants.length,
  };
}

export async function addParticipants(
  chatId: Jid,
  participants: Jid[],
): Promise<void> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return;
  }
  await invokeCore<void>("groups_add", { chatId, participants });
}

export async function removeParticipants(
  chatId: Jid,
  participants: Jid[],
): Promise<void> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return;
  }
  await invokeCore<void>("groups_remove", { chatId, participants });
}

export async function leaveGroup(chatId: Jid): Promise<void> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    return;
  }
  await invokeCore<void>("groups_leave", { chatId });
}

export async function fetchInviteLink(chatId: Jid): Promise<string> {
  if (!isTauri()) {
    await delay(MOCK_LATENCY_MS);
    const token = hashString(chatId).toString(36).toUpperCase();
    return `https://chat.whatsapp.com/${token}DEMO`;
  }
  return invokeCore<string>("groups_invite_link", { chatId });
}
