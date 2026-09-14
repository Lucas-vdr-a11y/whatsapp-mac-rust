/**
 * Status screen: post a text status ("My status") and browse the status
 * updates contacts posted in the last 24 hours. Updates arrive as messages on
 * `status@broadcast` and are served by `statuses_list`; tapping a sender's row
 * opens the fullscreen viewer (`components/status/StatusViewer`) and reports
 * `status_viewed`. Plain-browser runs keep a small demo list so the screen can
 * be reviewed without the Rust host.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Camera,
  Image as ImageIcon,
  Mic,
  Pencil,
  Plus,
  UserRound,
  Video,
  X,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { t, useTranslation } from "../../lib/i18n";
import { initials } from "../../lib/names";
import { useAppStore } from "../../store/app";
import { senderName, statusTime, updatePreview } from "../status/format";
import {
  STATUS_TTL_SECS,
  type StatusGroup,
  type StatusKind,
  type StatusUpdate,
} from "../status/types";
import { StatusViewer } from "../status/StatusViewer";
import { EmptyState, ScreenHeader } from "./shared";

/** Six status backgrounds drawn from the app palette. The core receives the
 * ARGB value; `css` mirrors it for the swatch. */
const STATUS_BACKGROUNDS = [
  { id: "green", labelKey: "status.background.green", css: "#21c063", argb: 0xff21c063 },
  { id: "forest", labelKey: "status.background.forest", css: "#144d37", argb: 0xff144d37 },
  { id: "sky", labelKey: "status.background.sky", css: "#53bdeb", argb: 0xff53bdeb },
  { id: "coral", labelKey: "status.background.coral", css: "#f15c6d", argb: 0xfff15c6d },
  { id: "amber", labelKey: "status.background.amber", css: "#ffbc38", argb: 0xffffbc38 },
  { id: "ink", labelKey: "status.background.ink", css: "#242626", argb: 0xff242626 },
] as const;

const STATUS_MAX_LENGTH = 700;

/** Browser-preview rows; Tauri builds start empty and fill from the core. */
function demoUpdates(): StatusUpdate[] {
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + STATUS_TTL_SECS;
  return [
    {
      id: "status-demo-1",
      sender: "alice@s.whatsapp.net",
      timestamp: now - 9 * 60,
      kind: "text",
      text: "Sunrise run done. 12 km before work — the city was completely quiet.",
      backgroundArgb: 0xff144d37,
      expiresAt,
      viewed: false,
    },
    {
      id: "status-demo-2",
      sender: "alice@s.whatsapp.net",
      timestamp: now - 47 * 60,
      kind: "image",
      text: "Golden hour over the canals",
      backgroundArgb: null,
      expiresAt,
      viewed: false,
    },
    {
      id: "status-demo-3",
      sender: "bob@s.whatsapp.net",
      timestamp: now - 3 * 60 * 60,
      kind: "text",
      text: "New studio corner is finally done.",
      backgroundArgb: 0xff53bdeb,
      expiresAt,
      viewed: false,
    },
    {
      id: "status-demo-4",
      sender: "marieke@s.whatsapp.net",
      timestamp: now - 5 * 60 * 60,
      kind: "voice",
      text: null,
      backgroundArgb: null,
      expiresAt,
      viewed: true,
    },
  ];
}

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
    return t("status.needsSession");
  }
  if (/not implemented|unknown command|unrecognized/i.test(raw)) {
    return t("status.unavailable");
  }
  return raw;
}

export function StatusScreen() {
  const { t } = useTranslation();
  const chats = useAppStore((state) => state.chats);
  const names = useMemo(() => {
    const map: Record<string, string> = {};
    for (const chat of chats) {
      if (!chat.isGroup && chat.name) map[chat.id] = chat.name;
    }
    return map;
  }, [chats]);

  const [updates, setUpdates] = useState<StatusUpdate[]>(() =>
    isTauri() ? [] : demoUpdates(),
  );
  const [loading, setLoading] = useState(() => isTauri());
  const [listError, setListError] = useState<string | null>(null);

  const [composerOpen, setComposerOpen] = useState(false);
  const [text, setText] = useState("");
  const [backgroundArgb, setBackgroundArgb] = useState<number>(
    STATUS_BACKGROUNDS[0].argb,
  );
  const [posting, setPosting] = useState(false);
  const [postError, setPostError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [viewer, setViewer] = useState<{
    group: StatusGroup;
    index: number;
  } | null>(null);

  const loadUpdates = useCallback(async () => {
    if (!isTauri()) {
      setUpdates(demoUpdates());
      return;
    }
    setLoading(true);
    setListError(null);
    try {
      const rows = await invokeCore<StatusUpdate[]>("statuses_list");
      const cutoff = Math.floor(Date.now() / 1000) - STATUS_TTL_SECS;
      setUpdates(rows.filter((row) => row.timestamp >= cutoff));
    } catch (error) {
      setListError(friendlyError(error, t("status.loadError")));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadUpdates();
  }, [loadUpdates]);

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

  /** Optimistically mark one update read and persist it in the core. */
  const markViewed = useCallback((update: StatusUpdate) => {
    if (update.viewed) return;
    setUpdates((current) =>
      current.map((item) =>
        item.id === update.id ? { ...item, viewed: true } : item,
      ),
    );
    if (isTauri()) {
      void invokeCore<void>("status_viewed", { id: update.id }).catch(
        (error) => console.error("status_viewed failed", error),
      );
    }
  }, []);

  /** Open a sender's updates, preferring their first unviewed one. */
  const openViewer = (group: StatusGroup) => {
    const firstUnviewed = group.updates.findIndex((update) => !update.viewed);
    const index = firstUnviewed >= 0 ? firstUnviewed : 0;
    setViewer({ group, index });
    markViewed(group.updates[index]);
  };

  const navigateViewer = useCallback(
    (index: number) => {
      if (!viewer) return;
      const update = viewer.group.updates[index];
      if (!update) return;
      setViewer({ ...viewer, index });
      markViewed(update);
    },
    [viewer, markViewed],
  );

  const groups = useMemo(() => {
    const bySender = new Map<string, StatusUpdate[]>();
    for (const update of updates) {
      const list = bySender.get(update.sender);
      if (list) list.push(update);
      else bySender.set(update.sender, [update]);
    }

    const result: StatusGroup[] = [];
    for (const [sender, items] of bySender) {
      items.sort((a, b) => b.timestamp - a.timestamp);
      result.push({
        sender,
        updates: items,
        latest: items[0],
        unviewed: items.filter((item) => !item.viewed).length,
      });
    }
    return result.sort((a, b) => b.latest.timestamp - a.latest.timestamp);
  }, [updates]);

  const recentGroups = groups.filter((group) => group.unviewed > 0);
  const viewedGroups = groups.filter((group) => group.unviewed === 0);

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
        setNotice(t("status.posted"));
      } else {
        setNotice(t("status.postedPreview"));
      }
      setText("");
      setComposerOpen(false);
    } catch (error) {
      setPostError(friendlyError(error, t("status.postError")));
    } finally {
      setPosting(false);
    }
  };

  return (
    <section className="chat-list screen">
      <ScreenHeader title={t("status.title")}>
        <button
          type="button"
          className="icon-button"
          title={t("status.textStatus")}
          onClick={openComposer}
        >
          <Pencil size={22} />
        </button>
        <button
          type="button"
          className="icon-button"
          title={t("status.addToMyStatus")}
        >
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
            <span className="status-item-name">{t("status.myStatus")}</span>
            <span className="status-item-preview">
              {t("status.clickToAdd")}
            </span>
          </span>
        </button>

        {notice ? (
          <p className="screen-notice" role="status">
            {notice}
          </p>
        ) : null}

        <div className="screen-section-label accent">
          {t("status.recentUpdates")}
        </div>

        {loading && updates.length === 0 ? (
          <p className="screen-loading">{t("status.loading")}</p>
        ) : null}

        {listError ? (
          <div className="screen-inline-error" role="alert">
            <p>{listError}</p>
            <button
              type="button"
              className="modal-action secondary"
              onClick={() => void loadUpdates()}
            >
              {t("common.retry")}
            </button>
          </div>
        ) : null}

        {!loading && !listError && groups.length === 0 ? (
          <EmptyState
            icon={<UserRound size={26} strokeWidth={1.5} />}
            title={t("status.emptyTitle")}
            hint={t("status.emptyHint")}
          />
        ) : null}

        {recentGroups.map((group) => (
          <StatusGroupRow
            key={group.sender}
            group={group}
            name={senderName(group.sender, names)}
            onOpen={openViewer}
          />
        ))}

        {viewedGroups.length > 0 ? (
          <div className="screen-section-label">{t("status.viewed")}</div>
        ) : null}
        {viewedGroups.map((group) => (
          <StatusGroupRow
            key={group.sender}
            group={group}
            name={senderName(group.sender, names)}
            onOpen={openViewer}
          />
        ))}
      </div>

      {viewer ? (
        <StatusViewer
          group={viewer.group}
          index={viewer.index}
          name={senderName(viewer.group.sender, names)}
          onClose={() => setViewer(null)}
          onNavigate={navigateViewer}
        />
      ) : null}

      {composerOpen ? (
        <div className="modal-backdrop" onMouseDown={closeComposer}>
          <div
            className="status-composer"
            role="dialog"
            aria-modal="true"
            aria-label={t("status.myStatus")}
            onMouseDown={(event) => event.stopPropagation()}
          >
            <header className="modal-header">
              <h2 className="modal-title">{t("status.myStatus")}</h2>
              <button
                type="button"
                className="icon-button"
                title={t("common.close")}
                aria-label={t("common.close")}
                onClick={closeComposer}
              >
                <X size={22} />
              </button>
            </header>

            <textarea
              className="status-composer-input"
              placeholder={t("status.composerPlaceholder")}
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
              aria-label={t("status.backgroundsAria")}
            >
              {STATUS_BACKGROUNDS.map((background) => (
                <button
                  key={background.id}
                  type="button"
                  role="radio"
                  aria-checked={backgroundArgb === background.argb}
                  aria-label={t("status.backgroundAria", {
                    name: t(background.labelKey),
                  })}
                  title={t(background.labelKey)}
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
                {t("common.cancel")}
              </button>
              <button
                type="button"
                className="modal-action primary"
                disabled={posting || text.trim().length === 0}
                onClick={() => void handlePost()}
              >
                {posting ? t("status.posting") : t("status.post")}
              </button>
            </footer>
          </div>
        </div>
      ) : null}
    </section>
  );
}

/** Small glyph for the newest update's kind (localization-free). */
function LatestKindIcon({ kind }: { kind: StatusKind }) {
  switch (kind) {
    case "image":
      return <ImageIcon size={14} className="status-item-kind" aria-hidden="true" />;
    case "video":
      return <Video size={14} className="status-item-kind" aria-hidden="true" />;
    case "voice":
      return <Mic size={14} className="status-item-kind" aria-hidden="true" />;
    default:
      return null;
  }
}

/** One sender row: ring, name, newest update preview and time. */
function StatusGroupRow({
  group,
  name,
  onOpen,
}: {
  group: StatusGroup;
  name: string;
  onOpen: (group: StatusGroup) => void;
}) {
  const { t } = useTranslation();
  const count = group.updates.length;
  return (
    <button
      type="button"
      className="status-item status-item-button"
      aria-label={
        group.unviewed > 0
          ? t("status.groupAriaUnviewed", {
              name,
              count: group.unviewed,
            })
          : t("status.groupAriaViewed", { name })
      }
      onClick={() => onOpen(group)}
    >
      <span className={`status-ring${group.unviewed === 0 ? " viewed" : ""}`}>
        <span className="avatar">{initials(name)}</span>
      </span>
      <span className="status-item-body">
        <span className="status-item-name">
          {name}
          {count > 1 ? (
            <span className="status-group-count">{count}</span>
          ) : null}
        </span>
        <span className="status-item-preview">
          <LatestKindIcon kind={group.latest.kind} />
          {statusTime(group.latest.timestamp)} · {updatePreview(group.latest)}
        </span>
      </span>
    </button>
  );
}
