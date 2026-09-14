/**
 * Local, UI-only state for message actions whose protocol snapshots are not
 * exposed to the UI yet.
 *
 * - Pinned messages: `message_pin` / `message_unpin` mutate the protocol, but
 *   nothing reports the set of pinned message ids back to the UI, so this Set
 *   only reflects pins made in this session. It resets on reload.
 * - Poll options: `Message` carries only the poll question (`text`); the
 *   option list lives in the protobuf body, which the core does not persist.
 *   Options created through `PollComposer` in this session are remembered by
 *   message id so the bubble can render them. Incoming polls from another
 *   device render the question until the core exposes the body.
 * - Poll votes: `poll_vote` sends the vote but no vote tally comes back, so
 *   the chosen option is remembered locally. Percentages stay unknown.
 *
 * Everything here is intentionally outside the zustand store (owned elsewhere)
 * and shared through `useSyncExternalStore` so every bubble stays in sync.
 */

import { useCallback, useSyncExternalStore } from "react";

type Listener = () => void;
type ListenerMap = Map<string, Set<Listener>>;

/** Stable empty array so `useSyncExternalStore` snapshots never thrash. */
const EMPTY: readonly string[] = [];

function subscribeKey(
  listeners: ListenerMap,
  key: string,
  listener: Listener,
): () => void {
  let bucket = listeners.get(key);
  if (!bucket) {
    bucket = new Set();
    listeners.set(key, bucket);
  }
  bucket.add(listener);
  return () => {
    bucket.delete(listener);
    if (bucket.size === 0) listeners.delete(key);
  };
}

function notifyKey(listeners: ListenerMap, key: string): void {
  listeners.get(key)?.forEach((listener) => listener());
}

/* ------------------------------------------------------------------ */
/* Pinned messages                                                     */
/* ------------------------------------------------------------------ */

const pinned = new Set<string>();
const pinnedListeners: ListenerMap = new Map();

/** Optimistically record (or clear) a message's locally-known pin state. */
export function setMessagePinned(messageId: string, value: boolean): void {
  if (pinned.has(messageId) === value) return;
  if (value) pinned.add(messageId);
  else pinned.delete(messageId);
  notifyKey(pinnedListeners, messageId);
}

export function useMessagePinned(messageId: string): boolean {
  const subscribe = useCallback(
    (listener: Listener) => subscribeKey(pinnedListeners, messageId, listener),
    [messageId],
  );
  const getSnapshot = useCallback(() => pinned.has(messageId), [messageId]);
  return useSyncExternalStore(subscribe, getSnapshot);
}

/* ------------------------------------------------------------------ */
/* Poll options                                                        */
/* ------------------------------------------------------------------ */

const pollOptions = new Map<string, readonly string[]>();
const pollOptionListeners: ListenerMap = new Map();

/** Cache the options of a poll created in this session. */
export function rememberPollOptions(
  messageId: string,
  options: readonly string[],
): void {
  pollOptions.set(messageId, [...options]);
  notifyKey(pollOptionListeners, messageId);
}

/** Options for a poll, or `null` when only the question is known. */
export function usePollOptions(messageId: string): readonly string[] | null {
  const subscribe = useCallback(
    (listener: Listener) =>
      subscribeKey(pollOptionListeners, messageId, listener),
    [messageId],
  );
  const getSnapshot = useCallback(
    () => pollOptions.get(messageId) ?? null,
    [messageId],
  );
  return useSyncExternalStore(subscribe, getSnapshot);
}

/* ------------------------------------------------------------------ */
/* Poll votes                                                          */
/* ------------------------------------------------------------------ */

const pollVotes = new Map<string, readonly string[]>();
const pollVoteListeners: ListenerMap = new Map();

/** Remember our vote; an empty list clears it (matching `poll_vote`). */
export function setLocalPollVote(
  messageId: string,
  optionNames: readonly string[],
): void {
  if (optionNames.length > 0) pollVotes.set(messageId, [...optionNames]);
  else pollVotes.delete(messageId);
  notifyKey(pollVoteListeners, messageId);
}

export function useLocalPollVote(messageId: string): readonly string[] {
  const subscribe = useCallback(
    (listener: Listener) => subscribeKey(pollVoteListeners, messageId, listener),
    [messageId],
  );
  const getSnapshot = useCallback(
    () => pollVotes.get(messageId) ?? EMPTY,
    [messageId],
  );
  return useSyncExternalStore(subscribe, getSnapshot);
}
