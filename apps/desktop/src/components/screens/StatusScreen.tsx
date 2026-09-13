/**
 * Status screen: the demo Recent/Viewed lists plus a text status composer
 * backed by `channels_post_status`. Inside the Tauri host a short note
 * explains that incoming updates arrive with the status milestone.
 */

import { useEffect, useState } from "react";
import {
  BellOff,
  Camera,
  ChevronRight,
  Info,
  Pencil,
  Plus,
  UserRound,
  X,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { initials } from "../../lib/names";
import { ScreenHeader } from "./shared";

interface StatusUpdate {
  id: string;
  name: string;
  at: string;
  viewed: boolean;
}

const UPDATES: StatusUpdate[] = [
  { id: "status-1", name: "Maya de Vries", at: "Today, 9:41 AM", viewed: false },
  { id: "status-2", name: "Design Weekly", at: "Today, 8:15 AM", viewed: false },
  { id: "status-3", name: "Tom Bakker", at: "Yesterday, 9:03 PM", viewed: false },
  { id: "status-4", name: "Priya Nair", at: "Yesterday, 6:22 PM", viewed: true },
  { id: "status-5", name: "Sanne & Bas", at: "Yesterday, 12:40 PM", viewed: true },
];

const MUTED_UPDATES = 3;

/** Six status backgrounds drawn from the app palette. The core receives the
 * ARGB value; `css` mirrors it for the swatch. */
const STATUS_BACKGROUNDS = [
  { id: "green", name: "Green", css: "#21c063", argb: 0xff21c063 },
  { id: "forest", name: "Forest", css: "#144d37", argb: 0xff144d37 },
  { id: "sky", name: "Sky", css: "#53bdeb", argb: 0xff53bdeb },
  { id: "coral", name: "Coral", css: "#f15c6d", argb: 0xfff15c6d },
  { id: "amber", name: "Amber", css: "#ffbc38", argb: 0xffffbc38 },
  { id: "ink", name: "Ink", css: "#242626", argb: 0xff242626 },
] as const;

const STATUS_MAX_LENGTH = 700;

/** Short, non-technical message for a failed status command. */
function friendlyError(error: unknown, fallback: string): string {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "";
  if (!raw) return fallback;
  if (
    /not linked|no session|session (is )?(closed|missing)|not paired|disconnected/i.test(
      raw,
    )
  ) {
    return "Posting a status needs a linked WhatsApp session. Link your phone in Settings, then try again.";
  }
  if (/not implemented|unknown command|unrecognized/i.test(raw)) {
    return "Status updates aren't available in this build yet.";
  }
  return raw;
}

export function StatusScreen() {
  const recent = UPDATES.filter((update) => !update.viewed);
  const viewed = UPDATES.filter((update) => update.viewed);

  const [composerOpen, setComposerOpen] = useState(false);
  const [text, setText] = useState("");
  const [backgroundArgb, setBackgroundArgb] = useState<number>(
    STATUS_BACKGROUNDS[0].argb,
  );
  const [posting, setPosting] = useState(false);
  const [postError, setPostError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!composerOpen) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !posting) {
        setComposerOpen(false);
        setPostError(null);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [composerOpen, posting]);

  const openComposer = () => {
    setPostError(null);
    setComposerOpen(true);
  };

  const closeComposer = () => {
    if (posting) return;
    setComposerOpen(false);
    setPostError(null);
  };

  const handlePost = async () => {
    const trimmed = text.trim();
    if (!trimmed || posting) return;

    setPosting(true);
    setPostError(null);
    try {
      if (isTauri()) {
        await invokeCore<void>("channels_post_status", {
          text: trimmed,
          backgroundArgb,
        });
        setNotice("Status posted.");
      } else {
        setNotice("Status posted (browser preview).");
      }
      setText("");
      setComposerOpen(false);
    } catch (error) {
      setPostError(friendlyError(error, "Couldn't post your status."));
    } finally {
      setPosting(false);
    }
  };

  return (
    <section className="chat-list screen">
      <ScreenHeader title="Status">
        <button
          type="button"
          className="icon-button"
          title="Text status"
          onClick={openComposer}
        >
          <Pencil size={22} />
        </button>
        <button type="button" className="icon-button" title="Add to my status">
          <Camera size={22} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <button
          type="button"
          className="status-item status-item-button"
          onClick={openComposer}
        >
          <span className="status-ring dashed">
            <span className="avatar">
              <UserRound size={24} strokeWidth={1.6} />
            </span>
            <span className="status-add-badge">
              <Plus size={13} strokeWidth={3} />
            </span>
          </span>
          <span className="status-item-body">
            <span className="status-item-name">My status</span>
            <span className="status-item-preview">
              Click to add status update
            </span>
          </span>
        </button>

        {isTauri() ? (
          <p className="status-milestone-note" role="note">
            <Info size={14} aria-hidden="true" />
            <span>
              Received status updates appear here once the status milestone
              lands. Posting your own status already works.
            </span>
          </p>
        ) : null}

        {notice ? (
          <p className="screen-notice" role="status">
            {notice}
          </p>
        ) : null}

        <div className="screen-section-label accent">Recent</div>
        {recent.map((update) => (
          <StatusRow key={update.id} update={update} />
        ))}

        <div className="screen-section-label">Viewed</div>
        {viewed.map((update) => (
          <StatusRow key={update.id} update={update} />
        ))}

        <button type="button" className="status-muted-header">
          <BellOff size={16} />
          <span>Muted updates</span>
          <span className="status-muted-count">{MUTED_UPDATES}</span>
          <ChevronRight size={16} className="status-muted-chevron" />
        </button>
      </div>

      {composerOpen ? (
        <div className="modal-backdrop" onMouseDown={closeComposer}>
          <div
            className="status-composer"
            role="dialog"
            aria-modal="true"
            aria-label="My status"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <header className="modal-header">
              <h2 className="modal-title">My status</h2>
              <button
                type="button"
                className="icon-button"
                title="Close"
                aria-label="Close"
                onClick={closeComposer}
              >
                <X size={22} />
              </button>
            </header>

            <textarea
              className="status-composer-input"
              placeholder="Type a status update"
              value={text}
              maxLength={STATUS_MAX_LENGTH}
              rows={4}
              autoFocus
              onChange={(event) => {
                setText(event.target.value);
                setPostError(null);
              }}
            />

            <div className="status-composer-count">
              {text.trim().length}/{STATUS_MAX_LENGTH}
            </div>

            <div
              className="status-composer-swatches"
              role="radiogroup"
              aria-label="Background color"
            >
              {STATUS_BACKGROUNDS.map((background) => (
                <button
                  key={background.id}
                  type="button"
                  role="radio"
                  aria-checked={backgroundArgb === background.argb}
                  aria-label={`${background.name} background`}
                  title={background.name}
                  className={`status-swatch${
                    backgroundArgb === background.argb ? " selected" : ""
                  }`}
                  style={{ background: background.css }}
                  onClick={() => setBackgroundArgb(background.argb)}
                />
              ))}
            </div>

            {postError ? (
              <p className="status-composer-error" role="alert">
                {postError}
              </p>
            ) : null}

            <footer className="status-composer-actions">
              <button
                type="button"
                className="modal-action secondary"
                disabled={posting}
                onClick={closeComposer}
              >
                Cancel
              </button>
              <button
                type="button"
                className="modal-action primary"
                disabled={posting || text.trim().length === 0}
                onClick={() => void handlePost()}
              >
                {posting ? "Posting…" : "Post"}
              </button>
            </footer>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function StatusRow({ update }: { update: StatusUpdate }) {
  return (
    <div className="status-item">
      <span className={`status-ring${update.viewed ? " viewed" : ""}`}>
        <span className="avatar">{initials(update.name)}</span>
      </span>
      <span className="status-item-body">
        <span className="status-item-name">{update.name}</span>
        <span className="status-item-preview">{update.at}</span>
      </span>
    </div>
  );
}
