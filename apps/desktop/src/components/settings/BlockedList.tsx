import { useState, type FormEvent } from "react";
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
      setError(
        "Enter a phone number or a JID like 15551234567@s.whatsapp.net.",
      );
      return;
    }
    if (entries.includes(jid)) {
      setError(null);
      setNotice(`${jid} is already in this session's list.`);
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);
    void blockContact(jid)
      .then(() => {
        setEntries((list) => [...list, jid]);
        setInput("");
        setNotice(`${jid} blocked.`);
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
        setNotice(`${jid} unblocked.`);
      })
      .catch((cause: unknown) => setError(privacyErrorMessage(cause)))
      .finally(() => setUnblocking(null));
  };

  return (
    <div className="blocked-panel">
      <p className="blocked-note">
        This build can't read your existing blocklist yet — the core has no
        command for it. Contacts you block here are listed for this session
        only.
      </p>

      <form className="blocked-add" onSubmit={submit}>
        <input
          className="blocked-input"
          type="text"
          value={input}
          placeholder="Phone number or JID"
          aria-label="Phone number or JID to block"
          spellCheck={false}
          disabled={busy}
          onChange={(event) => setInput(event.target.value)}
        />
        <button
          type="submit"
          className="settings-button danger"
          disabled={busy || input.trim().length === 0}
        >
          {busy ? "Blocking…" : "Block"}
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
                {unblocking === jid ? "Unblocking…" : "Unblock"}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="blocked-empty">No contacts blocked in this session.</p>
      )}
    </div>
  );
}
