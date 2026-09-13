/** Media bubble for `video` messages.
 *
 * A muted first-frame tile with a play glyph. Clicking resolves the local file
 * through `media_download` (when needed) and opens the lightbox player.
 * View-once videos render as a covered "1" card with a single-use reveal that
 * persists the viewed marker per message id. */

import { useState } from "react";
import { createPortal } from "react-dom";
import { Eye, EyeOff, LoaderCircle, Play, RotateCw } from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { Lightbox } from "./Lightbox";
import {
  isAutoDownloadDisabled,
  isViewOnce,
  isViewOnceViewed,
  markViewOnceViewed,
} from "./media";

type LoadState = "idle" | "loading" | "error";

export function VideoContent({ message }: { message: Message }) {
  const path = useAppStore((state) => state.mediaPaths[message.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [state, setState] = useState<LoadState>("idle");
  const [open, setOpen] = useState(false);
  const viewOnce = isViewOnce(message);
  const [viewed, setViewed] = useState(
    () => viewOnce && isViewOnceViewed(message.id),
  );
  const autoDownloadOff = isAutoDownloadDisabled();

  const openVideo = () => {
    if (path) {
      setOpen(true);
      return;
    }
    setState("loading");
    downloadMedia(message.id, message.kind)
      .then(() => {
        setState("idle");
        setOpen(true);
      })
      .catch(() => setState("error"));
  };

  // View-once reveal: download when needed, then open the player exactly once
  // and persist the viewed marker so it cannot be replayed.
  const revealOnce = () => {
    if (viewed || state === "loading") return;
    const reveal = () => {
      markViewOnceViewed(message.id);
      setViewed(true);
      setOpen(true);
    };
    if (path) {
      reveal();
      return;
    }
    setState("loading");
    downloadMedia(message.id, message.kind)
      .then(() => {
        setState("idle");
        reveal();
      })
      .catch(() => setState("error"));
  };

  const lightbox =
    open && path
      ? createPortal(
          <Lightbox
            kind="video"
            src={avatarSrc(path)}
            caption={message.text}
            onClose={() => setOpen(false)}
          />,
          document.body,
        )
      : null;

  const caption = message.text ? (
    <div className="media-caption">{message.text}</div>
  ) : null;

  if (viewOnce && viewed) {
    return (
      <>
        <div
          className="view-once-card video viewed"
          aria-label="View-once video, already opened"
        >
          <span className="view-once-badge" aria-hidden="true">
            1
          </span>
          <span className="view-once-state">
            <EyeOff size={22} />
            <span>Opened</span>
          </span>
        </div>
        {caption}
        {lightbox}
      </>
    );
  }

  if (viewOnce) {
    return (
      <>
        <button
          type="button"
          className={`view-once-card video${
            state === "error" ? " error" : ""
          }`}
          title={state === "error" ? "Retry download" : "Tap to view once"}
          onClick={state === "loading" ? undefined : revealOnce}
        >
          <span className="view-once-veil" aria-hidden="true" />
          <span className="view-once-badge" aria-hidden="true">
            1
          </span>
          <span className="view-once-state">
            {state === "loading" ? (
              <LoaderCircle size={24} className="spin" />
            ) : state === "error" ? (
              <RotateCw size={24} />
            ) : (
              <Eye size={24} />
            )}
            <span>
              {state === "loading"
                ? "Downloading…"
                : state === "error"
                  ? "Download failed — tap to retry"
                  : "Tap to view once"}
            </span>
            {state === "idle" && autoDownloadOff ? (
              <span className="media-hint">Auto-download is off</span>
            ) : null}
          </span>
        </button>
        {caption}
        {lightbox}
      </>
    );
  }

  return (
    <>
      <button
        type="button"
        className={`video-tile${path ? "" : " empty"}${
          state === "error" ? " error" : ""
        }`}
        title={path ? "Play video" : "Download video"}
        onClick={state === "loading" ? undefined : openVideo}
      >
        {path ? (
          <>
            <video src={avatarSrc(path)} muted playsInline preload="metadata" />
            <span className="video-glyph">
              <span>
                <Play size={24} fill="currentColor" />
              </span>
            </span>
          </>
        ) : state === "loading" ? (
          <span className="video-hint">
            <LoaderCircle size={28} className="spin" />
            <span>Downloading…</span>
          </span>
        ) : state === "error" ? (
          <span className="video-hint error">
            <RotateCw size={24} />
            <span>Download failed — tap to retry</span>
          </span>
        ) : (
          <span className="video-hint">
            <Play size={30} />
            <span>Video</span>
            {autoDownloadOff ? (
              <span className="media-hint">Auto-download is off</span>
            ) : null}
          </span>
        )}
      </button>
      {caption}
      {lightbox}
    </>
  );
}
