/**
 * Poll card rendered inside a `kind === "poll"` bubble.
 *
 * `Message.text` holds the question. The option list is only known when the
 * poll was created on this device through `PollComposer` (the core does not
 * persist poll bodies yet), so received polls show a short notice instead.
 *
 * Vote tallies are not part of the message stream, so the progress bars are
 * neutral: no percentages are invented. Our own vote is tracked locally in
 * `messageLocalState` and `poll_vote` is fired for the protocol (guarded with
 * `isTauri()`; browser preview keeps the local mark only).
 */

import { useState } from "react";
import { CircleCheck, LoaderCircle } from "lucide-react";
import { useTranslation } from "../../lib/i18n";
import { invokeCore, isTauri } from "../../lib/ipc";
import type { Message } from "../../lib/types";
import {
  setLocalPollVote,
  useLocalPollVote,
  usePollOptions,
} from "./messageLocalState";

export function PollContent({ message }: { message: Message }) {
  const { t } = useTranslation();
  const options = usePollOptions(message.id);
  const vote = useLocalPollVote(message.id);
  const [selected, setSelected] = useState<string | null>(
    () => vote[0] ?? null,
  );
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const question = message.text?.trim() || t("poll.fallbackQuestion");
  const voted = vote.length > 0;
  // Nothing to send while the selection matches the recorded vote.
  const canVote =
    selected !== null && !pending && !(voted && vote[0] === selected);

  const submit = () => {
    if (selected === null || !canVote) return;
    const previous = vote;

    setError(null);
    setLocalPollVote(message.id, [selected]);
    if (!isTauri()) return;

    setPending(true);
    void invokeCore("poll_vote", {
      chatId: message.chatId,
      pollMessageId: message.id,
      optionNames: [selected],
    })
      .catch((cause: unknown) => {
        setLocalPollVote(message.id, previous);
        setError(cause instanceof Error ? cause.message : String(cause));
      })
      .finally(() => setPending(false));
  };

  return (
    <div className="poll-card">
      <span className="poll-question">{question}</span>

      {options && options.length > 0 ? (
        <>
          <div className="poll-options" role="radiogroup" aria-label={question}>
            {options.map((option) => {
              const chosen = selected === option;
              const mine = vote.includes(option);
              return (
                <button
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={chosen}
                  className={`poll-option${chosen ? " chosen" : ""}${
                    mine ? " mine" : ""
                  }`}
                  onClick={() => setSelected(option)}
                >
                  <span className="poll-radio" aria-hidden="true" />
                  <span className="poll-option-name">{option}</span>
                  <span className="poll-bar" aria-hidden="true">
                    <span className="poll-bar-fill" />
                  </span>
                </button>
              );
            })}
          </div>

          <div className="poll-actions">
            {voted ? (
              <span className="poll-voted-note">
                <CircleCheck size={14} />
                {t("poll.voted")}
              </span>
            ) : (
              <span className="poll-hint">{t("poll.selectOne")}</span>
            )}
            <button
              type="button"
              className="poll-vote"
              disabled={!canVote}
              onClick={submit}
            >
              {pending ? (
                <LoaderCircle size={14} className="spin" />
              ) : null}
              {voted ? t("poll.changeVote") : t("poll.vote")}
            </button>
          </div>

          {error ? (
            <p className="poll-error" role="alert">
              {error}
            </p>
          ) : null}
        </>
      ) : (
        <p className="poll-note">{t("poll.optionsUnavailable")}</p>
      )}
    </div>
  );
}
