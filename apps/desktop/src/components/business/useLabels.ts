/**
 * Labels state for the chat context menu.
 *
 * There is no "labels for this chat" command in the core, so associations are
 * read from the `labels_list` snapshot (each label carries its `chatIds`) and
 * updated optimistically when the user toggles one. A failed toggle reverts
 * the optimistic change and rejects so the menu can show the error inline.
 */

import { useCallback, useEffect, useState } from "react";
import type { Jid } from "../../lib/types";
import {
  addLabel,
  businessErrorMessage,
  fetchLabels,
  removeLabel,
} from "./api";
import type { Label } from "./types";

export interface LabelsState {
  labels: Label[];
  loading: boolean;
  /** Snapshot load failure (toggles surface their own errors). */
  error: string | null;
  reload: () => void;
  /** Label ids with an in-flight association change. */
  pending: ReadonlySet<string>;
  /** Optimistic toggle; throws a friendly message when the call fails. */
  toggle: (chatId: Jid, labelId: string) => Promise<void>;
}

export function useLabels(): LabelsState {
  const [labels, setLabels] = useState<Label[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    void fetchLabels()
      .then((loaded) => {
        if (!cancelled) setLabels(loaded);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(businessErrorMessage(cause, "Couldn't load labels."));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  const toggle = useCallback(
    async (chatId: Jid, labelId: string) => {
      const label = labels.find((candidate) => candidate.id === labelId);
      if (!label) return;
      const associated = label.chatIds.includes(chatId);

      const apply = (on: boolean) => {
        setLabels((current) =>
          current.map((candidate) =>
            candidate.id !== labelId
              ? candidate
              : {
                  ...candidate,
                  chatIds: on
                    ? candidate.chatIds.includes(chatId)
                      ? candidate.chatIds
                      : [...candidate.chatIds, chatId]
                    : candidate.chatIds.filter((id) => id !== chatId),
                },
          ),
        );
      };

      setPending((current) => new Set(current).add(labelId));
      apply(!associated);
      try {
        if (associated) {
          await removeLabel(chatId, labelId);
        } else {
          await addLabel(chatId, labelId);
        }
      } catch (cause) {
        apply(associated);
        throw new Error(
          businessErrorMessage(cause, "Couldn't update the label."),
        );
      } finally {
        setPending((current) => {
          const next = new Set(current);
          next.delete(labelId);
          return next;
        });
      }
    },
    [labels],
  );

  return {
    labels,
    loading,
    error,
    reload: () => setAttempt((value) => value + 1),
    pending,
    toggle,
  };
}
