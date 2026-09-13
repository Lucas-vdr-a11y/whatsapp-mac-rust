/**
 * Standalone "Create poll" modal (question + 2–6 options + selectable count).
 *
 * `poll_create` (`{ chatId, question, options, selectableCount }`) returns the
 * new message id; the options are cached locally by that id so the emitted
 * poll bubble can render them, then the modal closes.
 *
 * Integration for the parent agent (Conversation.tsx owns the attachment menu,
 * which already reports `kind === "poll"`):
 *
 *   const [pollOpen, setPollOpen] = useState(false);
 *   // in handleAttach: if (kind === "poll") setPollOpen(true);
 *   {pollOpen ? (
 *     <PollComposer chatId={chat.id} onClose={() => setPollOpen(false)} />
 *   ) : null}
 *
 * In browser (mock) mode the host is absent, so the flow just closes.
 */

import { useEffect, useState } from "react";
import { Plus, X } from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import type { Jid } from "../../lib/types";
import { rememberPollOptions } from "./messageLocalState";

interface PollComposerProps {
  chatId: Jid;
  onClose: () => void;
}

const MIN_OPTIONS = 2;
const MAX_OPTIONS = 6;

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export function PollComposer({ chatId, onClose }: PollComposerProps) {
  const [question, setQuestion] = useState("");
  const [options, setOptions] = useState<string[]>(["", ""]);
  const [selectableCount, setSelectableCount] = useState(1);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onClose]);

  const updateOption = (index: number, value: string) =>
    setOptions((current) =>
      current.map((option, position) => (position === index ? value : option)),
    );

  const addOption = () =>
    setOptions((current) =>
      current.length >= MAX_OPTIONS ? current : [...current, ""],
    );

  const removeOption = (index: number) => {
    if (options.length <= MIN_OPTIONS) return;
    const next = options.filter((_option, position) => position !== index);
    setOptions(next);
    setSelectableCount((count) => clamp(count, 1, next.length));
  };

  const submit = () => {
    if (busy) return;

    const trimmedQuestion = question.trim();
    const names = options.map((option) => option.trim());
    if (!trimmedQuestion) {
      setError("Enter a question.");
      return;
    }
    if (names.length < MIN_OPTIONS || names.some((name) => !name)) {
      setError("Fill in at least two options.");
      return;
    }
    if (new Set(names).size !== names.length) {
      setError("Options must be unique.");
      return;
    }

    setError(null);
    if (!isTauri()) {
      onClose();
      return;
    }

    setBusy(true);
    void invokeCore<string>("poll_create", {
      chatId,
      question: trimmedQuestion,
      options: names,
      selectableCount: clamp(selectableCount, 1, names.length),
    })
      .then((messageId) => {
        rememberPollOptions(messageId, names);
        onClose();
      })
      .catch((cause: unknown) => {
        setError(cause instanceof Error ? cause.message : String(cause));
      })
      .finally(() => setBusy(false));
  };

  return (
    <div
      className="modal-backdrop"
      onMouseDown={() => {
        if (!busy) onClose();
      }}
    >
      <div
        className="message-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Create poll"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modal-header">
          <h2 className="modal-title">Create poll</h2>
          <button
            type="button"
            className="icon-button"
            title="Close"
            aria-label="Close"
            disabled={busy}
            onClick={onClose}
          >
            <X size={22} />
          </button>
        </header>

        <div className="poll-composer-body">
          <label className="poll-field">
            <span className="poll-field-label">Question</span>
            <input
              className="poll-input"
              type="text"
              placeholder="Ask a question"
              value={question}
              autoFocus
              maxLength={500}
              onChange={(event) => {
                setQuestion(event.target.value);
                setError(null);
              }}
            />
          </label>

          <div className="poll-fields">
            <span className="poll-field-label">Options</span>
            {options.map((option, index) => (
              <div className="poll-option-row" key={index}>
                <input
                  className="poll-input"
                  type="text"
                  placeholder={`Option ${index + 1}`}
                  value={option}
                  maxLength={200}
                  onChange={(event) => {
                    updateOption(index, event.target.value);
                    setError(null);
                  }}
                />
                {options.length > MIN_OPTIONS ? (
                  <button
                    type="button"
                    className="icon-button poll-option-remove"
                    title="Remove option"
                    aria-label={`Remove option ${index + 1}`}
                    onClick={() => removeOption(index)}
                  >
                    <X size={18} />
                  </button>
                ) : null}
              </div>
            ))}
            {options.length < MAX_OPTIONS ? (
              <button
                type="button"
                className="poll-add-option"
                onClick={addOption}
              >
                <Plus size={16} />
                Add option
              </button>
            ) : null}
          </div>

          <label className="poll-field poll-count">
            <span className="poll-field-label">Selectable answers</span>
            <input
              className="poll-input"
              type="number"
              min={1}
              max={options.length}
              value={selectableCount}
              onChange={(event) => {
                const parsed = Number(event.target.value);
                setSelectableCount(
                  Number.isFinite(parsed)
                    ? clamp(Math.trunc(parsed), 1, options.length)
                    : 1,
                );
              }}
            />
          </label>
        </div>

        {error ? (
          <p className="forward-error" role="alert">
            {error}
          </p>
        ) : null}

        <footer className="new-chat-footer">
          <span className="new-chat-count">2–6 options</span>
          <div className="new-chat-actions">
            <button
              type="button"
              className="modal-action secondary"
              disabled={busy}
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="button"
              className="modal-action primary"
              disabled={busy}
              onClick={submit}
            >
              {busy ? "Creating…" : "Create poll"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
