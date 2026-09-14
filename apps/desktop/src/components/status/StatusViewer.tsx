/**
 * Fullscreen status viewer.
 *
 * Shows one sender's updates over a dark backdrop: text statuses on their
 * `backgroundArgb` color, image/video/voice statuses as media resolved through
 * `media_download` (the status id is the underlying message id, so the cached
 * message media pipeline serves it directly). While a media file is being
 * fetched the card shows a loading state; a failed fetch offers a retry.
 *
 * The arrows move between the sender's updates, Escape closes, and every
 * update that becomes visible is reported through `status_viewed` by the
 * screen's `onNavigate` / open handler.
 */

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import {
  ChevronLeft,
  ChevronRight,
  LoaderCircle,
  RotateCw,
  X,
} from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import { useTranslation } from "../../lib/i18n";
import { initials } from "../../lib/names";
import { useAppStore } from "../../store/app";
import {
  argbToCss,
  mediaMessageKind,
  relativeTime,
  updateLabel,
} from "./format";
import { isMediaKind, STATUS_TTL_SECS, type StatusGroup } from "./types";

type LoadState = "idle" | "loading" | "error";

interface StatusViewerProps {
  /** All updates of one sender, newest first. */
  group: StatusGroup;
  /** Index of the visible update inside `group.updates`. */
  index: number;
  /** Sender display name, resolved by the screen from the chat list. */
  name: string;
  onClose: () => void;
  onNavigate: (index: number) => void;
}

export function StatusViewer({
  group,
  index,
  name,
  onClose,
  onNavigate,
}: StatusViewerProps) {
  const { t } = useTranslation();
  const update = group.updates[index];
  const path = useAppStore((state) => state.mediaPaths[update.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [mediaState, setMediaState] = useState<LoadState>("idle");
  const rootRef = useRef<HTMLDivElement>(null);

  const needsMedia = isMediaKind(update.kind);
  const messageKind = mediaMessageKind(update.kind);
  const [mediaForId, setMediaForId] = useState(update.id);

  // Drop the previous update's media state before the next paint; the effect
  // below then starts (or skips) the download for the newly shown update.
  if (mediaForId !== update.id) {
    setMediaForId(update.id);
    setMediaState(needsMedia && !path ? "loading" : "idle");
  }

  // Resolve the local file as soon as an update is shown. `downloadMedia`
  // wraps `media_download { messageId }` and caches the path per message id.
  useEffect(() => {
    if (!needsMedia || path) {
      setMediaState("idle");
      return;
    }
    let cancelled = false;
    setMediaState("loading");
    downloadMedia(update.id, messageKind)
      .then(() => {
        if (!cancelled) setMediaState("idle");
      })
      .catch(() => {
        if (!cancelled) setMediaState("error");
      });
    return () => {
      cancelled = true;
    };
  }, [needsMedia, path, update.id, messageKind, downloadMedia]);

  // Focus the dialog for Escape/arrows and keep the page behind it still.
  useEffect(() => {
    rootRef.current?.focus();
    document.body.classList.add("status-viewer-open");
    return () => document.body.classList.remove("status-viewer-open");
  }, []);

  // Escape closes; arrows move inside this sender's updates. Arrow keys that
  // belong to the focused video/audio controls keep their native meaning.
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const mediaControls =
        target?.tagName === "VIDEO" || target?.tagName === "AUDIO";
      if (event.key === "Escape") {
        onClose();
      } else if (!mediaControls && event.key === "ArrowLeft" && index > 0) {
        onNavigate(index - 1);
      } else if (
        !mediaControls &&
        event.key === "ArrowRight" &&
        index < group.updates.length - 1
      ) {
        onNavigate(index + 1);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [group.updates.length, index, onClose, onNavigate]);

  /** Snapshot of the remaining share of the 24 h lifetime when shown. */
  const lifetimePercent = useMemo(() => {
    const remaining = update.expiresAt - Date.now() / 1000;
    return Math.max(0, Math.min(100, (remaining / STATUS_TTL_SECS) * 100));
  }, [update.id, update.expiresAt]);

  const retry = () => {
    setMediaState("loading");
    downloadMedia(update.id, messageKind)
      .then(() => setMediaState("idle"))
      .catch(() => setMediaState("error"));
  };

  const caption = update.text?.trim() ? (
    <div className="status-viewer-caption">{update.text}</div>
  ) : null;

  let content: ReactNode;
  if (update.kind === "text") {
    content = (
      <div
        className="status-viewer-card"
        style={{ background: argbToCss(update.backgroundArgb) }}
      >
        <p className="status-viewer-text">
          {update.text?.trim() || updateLabel(update.kind)}
        </p>
      </div>
    );
  } else if (needsMedia) {
    content = (
      <div className={`status-viewer-card media ${update.kind}`}>
        {path ? (
          update.kind === "image" ? (
            <img
              className="status-viewer-image"
              src={avatarSrc(path)}
              alt={update.text ?? t("media.imagePreview")}
              draggable={false}
            />
          ) : update.kind === "video" ? (
            <video
              className="status-viewer-video"
              src={avatarSrc(path)}
              controls
              autoPlay
              playsInline
            />
          ) : (
            <audio
              className="status-viewer-audio"
              src={avatarSrc(path)}
              controls
              autoPlay
            />
          )
        ) : (
          <StatusMediaState state={mediaState} onRetry={retry} />
        )}
        {caption}
      </div>
    );
  } else {
    content = (
      <div className="status-viewer-card">
        <p className="status-viewer-text">{updateLabel(update.kind)}</p>
        <p className="status-viewer-hint">{t("status.mediaUnavailable")}</p>
      </div>
    );
  }

  return createPortal(
    <div
      ref={rootRef}
      className="status-viewer"
      role="dialog"
      aria-modal="true"
      aria-label={t("status.viewerAria", { name })}
      tabIndex={-1}
      onClick={onClose}
    >
      <button
        type="button"
        className="status-viewer-close"
        title={t("common.close")}
        aria-label={t("common.close")}
        onClick={onClose}
      >
        <X size={26} />
      </button>

      <div
        className={`status-viewer-stage${needsMedia ? " media" : ""}`}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="status-viewer-progress" aria-hidden="true">
          {group.updates.map((item, itemIndex) => (
            <span
              key={item.id}
              className={`status-viewer-segment${
                itemIndex <= index ? " active" : ""
              }`}
            />
          ))}
        </div>

        <div className="status-viewer-lifetime" aria-hidden="true">
          <span style={{ width: `${lifetimePercent}%` }} />
        </div>

        <header className="status-viewer-header">
          <span className="status-ring viewed">
            <span className="avatar">{initials(name)}</span>
          </span>
          <span className="status-viewer-meta">
            <span className="status-viewer-name">{name}</span>
            <span className="status-viewer-time">
              {relativeTime(update.timestamp)}
            </span>
          </span>
        </header>

        {content}
      </div>

      {group.updates.length > 1 ? (
        <>
          <button
            type="button"
            className="status-viewer-nav prev"
            title={t("status.previousUpdate")}
            aria-label={t("status.previousUpdate")}
            disabled={index === 0}
            onClick={(event) => {
              event.stopPropagation();
              onNavigate(index - 1);
            }}
          >
            <ChevronLeft size={26} />
          </button>
          <button
            type="button"
            className="status-viewer-nav next"
            title={t("status.nextUpdate")}
            aria-label={t("status.nextUpdate")}
            disabled={index >= group.updates.length - 1}
            onClick={(event) => {
              event.stopPropagation();
              onNavigate(index + 1);
            }}
          >
            <ChevronRight size={26} />
          </button>
        </>
      ) : null}
    </div>,
    document.body,
  );
}

/** Loading spinner or retry button while no local media path exists. */
function StatusMediaState({
  state,
  onRetry,
}: {
  state: LoadState;
  onRetry: () => void;
}) {
  const { t } = useTranslation();
  if (state === "error") {
    return (
      <button type="button" className="status-viewer-retry" onClick={onRetry}>
        <RotateCw size={26} />
        <span>{t("media.downloadFailed")}</span>
      </button>
    );
  }
  return (
    <div className="status-viewer-media-state" role="status">
      <LoaderCircle size={26} className="spin" />
      <span>{t("media.downloading")}</span>
    </div>
  );
}
