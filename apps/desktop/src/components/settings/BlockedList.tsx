import { useState, type FormEvent } from "react";
import { useTranslation } from "../../lib/i18n";
import {
  blockContact,
  normalizeJid,
  privacyErrorMessage,
  unblockContact,
} from "./privacy";

/**
 * Inline panel for the "Blocked contacts" row.
 *
 * The core can block and unblock contacts but has no command that returns the
 * blocklist, so this panel is honest about what it can show: the server list
 * cannot be fetched yet, and contacts blocked through this form are kept in
 * session-local state only. Unblocking always goes through `privacy_unblock`.
 */
export function BlockedList() {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<string[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [unblocking, setUnblocking] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const jid = normalizeJid(input);
    if (!jid) {
      setNotice(null);
      setError(t("blocked.invalid"));
      return;
    }
    if (entries.includes(jid)) {
      setError(null);
      setNotice(t("blocked.already", { jid }));
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);
    void blockContact(jid)
      .then(() => {
        setEntries((list) => [...list, jid]);
        setInput("");
        setNotice(t("blocked.blocked", { jid }));
      })
      .catch((cause: unknown) => setError(privacyErrorMessage(cause)))
      .finally(() => setBusy(false));
  };

  const unblock = (jid: string) => {
    setUnblocking(jid);
    setError(null);
    setNotice(null);
    void unblockContact(jid)
      .then(() => {
        setEntries((list) => list.filter((entry) => entry !== jid));
        setNotice(t("blocked.unblocked", { jid }));
      })
      .catch((cause: unknown) => setError(privacyErrorMessage(cause)))
      .finally(() => setUnblocking(null));
  };

  return (
    <div className="blocked-panel">
      <p className="blocked-note">{t("blocked.note")}</p>

      <form className="blocked-add" onSubmit={submit}>
        <input
          className="blocked-input"
          type="text"
          value={input}
          placeholder={t("blocked.placeholder")}
          aria-label={t("blocked.inputAria")}
          spellCheck={false}
          disabled={busy}
          onChange={(event) => setInput(event.target.value)}
        />
        <button
          type="submit"
          className="settings-button danger"
          disabled={busy || input.trim().length === 0}
        >
          {busy ? t("blocked.blocking") : t("blocked.block")}
        </button>
      </form>

      {error ? (
        <p className="blocked-feedback error" role="alert">
          {error}
        </p>
      ) : notice ? (
        <p className="blocked-feedback ok">{notice}</p>
      ) : null}

      {entries.length > 0 ? (
        <ul className="blocked-list">
          {entries.map((jid) => (
            <li key={jid} className="blocked-item">
              <span className="blocked-jid" title={jid}>
                {jid}
              </span>
              <button
                type="button"
                className="settings-button"
                disabled={unblocking !== null}
                onClick={() => unblock(jid)}
              >
                {unblocking === jid ? t("blocked.unblocking") : t("blocked.unblock")}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="blocked-empty">{t("blocked.empty")}</p>
      )}
    </div>
  );
}
