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
import { useTranslation } from "../../lib/i18n";
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
  const { t } = useTranslation();
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
          aria-label={t("media.viewOnceVideoOpened")}
        >
          <span className="view-once-badge" aria-hidden="true">
            1
          </span>
          <span className="view-once-state">
            <EyeOff size={22} />
            <span>{t("media.opened")}</span>
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
          title={
            state === "error"
              ? t("media.retryDownload")
              : t("media.tapToViewOnce")
          }
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
                ? t("media.downloading")
                : state === "error"
                  ? t("media.downloadFailed")
                  : t("media.tapToViewOnce")}
            </span>
            {state === "idle" && autoDownloadOff ? (
              <span className="media-hint">{t("media.autoDownloadOff")}</span>
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
        title={path ? t("media.playVideo") : t("media.downloadVideo")}
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
            <span>{t("media.downloading")}</span>
          </span>
        ) : state === "error" ? (
          <span className="video-hint error">
            <RotateCw size={24} />
            <span>{t("media.downloadFailed")}</span>
          </span>
        ) : (
          <span className="video-hint">
            <Play size={30} />
            <span>{t("media.video")}</span>
            {autoDownloadOff ? (
              <span className="media-hint">{t("media.autoDownloadOff")}</span>
            ) : null}
          </span>
        )}
      </button>
      {caption}
      {lightbox}
    </>
  );
}
