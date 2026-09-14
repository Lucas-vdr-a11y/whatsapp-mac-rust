/**
 * Mention autocomplete helpers.
 *
 * The composer tracks mentions as spans in the draft text: when the user picks
 * a participant we insert `@Name ` and remember where it landed. Every edit
 * re-validates the spans, so deleting part of a name simply drops that mention
 * from the outgoing list instead of sending a stale JID.
 */

import type { Jid, Message } from "../../lib/types";

/** A person the composer can @-mention. */
export interface MentionParticipant {
  id: Jid;
  name: string;
}

/** A mention inserted in the composer text. */
export interface MentionRef {
  jid: Jid;
  name: string;
  /** Index of the `@` in the composer text. */
  start: number;
  /** Index just past the inserted `@Name ` text. */
  end: number;
}

/** The in-progress `@query` right before the caret, when one is active. */
export interface MentionTrigger {
  /** Index of the `@` in the composer text. */
  start: number;
  /** Text between `@` and the caret; empty right after typing `@`. */
  query: string;
}

/** Longest query treated as an in-progress mention. */
const MAX_QUERY_LENGTH = 40;

/**
 * Group participants known to the UI: every distinct sender in the loaded
 * history (minus ourselves), with display names from the chat list
 * (`list_chats`). The core exposes no participant command yet, so people who
 * never posted in the loaded window are not listed — an accepted approximation.
 */
export function deriveGroupParticipants(
  messages: Message[],
  contactNames: Record<Jid, string>,
): MentionParticipant[] {
  const participants = new Map<Jid, MentionParticipant>();

  for (const message of messages) {
    const jid = message.senderId;
    if (message.fromMe || !jid || jid === "me" || jid.endsWith("@g.us")) {
      continue;
    }
    if (participants.has(jid)) continue;
    participants.set(jid, {
      id: jid,
      name: contactNames[jid] ?? displayNameFromJid(jid),
    });
  }

  return [...participants.values()].sort((a, b) => a.name.localeCompare(b.name));
}

/**
 * Find an in-progress `@mention` immediately before `caret`, if any.
 *
 * The `@` must sit at a word boundary and the query may not contain whitespace,
 * so ordinary prose ("mail@example", "wow @ that") never opens the menu.
 */
export function findMentionTrigger(
  text: string,
  caret: number,
): MentionTrigger | null {
  if (caret <= 0) return null;

  const before = text.slice(0, caret);
  const start = before.lastIndexOf("@");
  if (start === -1) return null;

  const preceding = start === 0 ? "" : before.charAt(start - 1);
  if (preceding !== "" && !/\s/.test(preceding)) return null;

  const query = before.slice(start + 1);
  if (query.length > MAX_QUERY_LENGTH || /\s/.test(query)) return null;

  return { start, query };
}

/** Filter participants by name, prefix matches first. */
export function matchParticipants(
  participants: MentionParticipant[],
  query: string,
): MentionParticipant[] {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return participants;

  const matches: Array<{ participant: MentionParticipant; rank: number }> = [];
  for (const participant of participants) {
    const index = participant.name.toLocaleLowerCase().indexOf(needle);
    if (index === -1) continue;
    matches.push({ participant, rank: index === 0 ? 0 : 1 });
  }
  matches.sort((a, b) => a.rank - b.rank);
  return matches.map((match) => match.participant);
}

/**
 * True while `mention`'s `@Name` still sits at its recorded span. The trailing
 * space is optional (deleting it must not cancel the mention), but a letter or
 * digit directly after the name means the user rewrote it and the span no
 * longer represents the picked participant.
 */
export function isMentionIntact(text: string, mention: MentionRef): boolean {
  if (!text.startsWith(`@${mention.name}`, mention.start)) return false;
  const after = text.charAt(mention.start + mention.name.length + 1);
  return !/[\p{L}\p{N}]/u.test(after);
}

/** Drop mentions whose spans no longer match the text. */
export function pruneMentions(
  text: string,
  mentions: MentionRef[],
): MentionRef[] {
  return mentions.filter((mention) => isMentionIntact(text, mention));
}

/** JIDs of the mentions still present in `text`, deduplicated. */
export function mentionJids(text: string, mentions: MentionRef[]): Jid[] {
  const jids = new Set<Jid>();
  for (const mention of mentions) {
    if (isMentionIntact(text, mention)) jids.add(mention.jid);
  }
  return [...jids];
}

/** Fallback display name for a JID the chat list does not know. */
function displayNameFromJid(jid: Jid): string {
  const [local] = jid.split("@");
  return local.replace(/:.*$/, "");
}
