/** Media bubble for `image`, `gif` and `sticker` messages.
 *
 * Shows a placeholder with a download affordance until `media_download`
 * resolves a local path, then a bounded rounded preview that opens in the
 * lightbox. Stickers render small and without bubble chrome.
 *
 * View-once payloads render as a covered "1" card; the reveal downloads the
 * file (when needed), opens the lightbox once and persists the viewed marker
 * per message id so the media cannot be reopened. */

import { useState } from "react";
import { createPortal } from "react-dom";
import {
  Download,
  Eye,
  EyeOff,
  ImageOff,
  LoaderCircle,
  RotateCw,
} from "lucide-react";
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

interface ImageContentProps {
  message: Message;
  sticker?: boolean;
}

export function ImageContent({ message, sticker = false }: ImageContentProps) {
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

  const download = () => {
    setState("loading");
    downloadMedia(message.id, message.kind)
      .then(() => setState("idle"))
      .catch(() => setState("error"));
  };

  // View-once reveal: download when needed, then open the lightbox exactly
  // once and persist the viewed marker so it cannot be replayed.
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

  const caption = message.text ? (
    <div className="media-caption">{message.text}</div>
  ) : null;

  const lightbox =
    open && path
      ? createPortal(
          <Lightbox
            kind="image"
            src={avatarSrc(path)}
            caption={message.text}
            onClose={() => setOpen(false)}
          />,
          document.body,
        )
      : null;

  if (viewOnce && viewed) {
    return (
      <>
        <div
          className={`view-once-card viewed${sticker ? " sticker" : ""}`}
          aria-label={t("media.viewOncePhotoOpened")}
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
          className={`view-once-card${sticker ? " sticker" : ""}${
            state === "error" ? " error" : ""
          }`}
          title={
            state === "error"
              ? t("media.retryDownload")
              : t("media.tapToViewOnce")
          }
          onClick={state === "loading" ? undefined : revealOnce}
        >
          {path ? (
            <img
              className="view-once-preview"
              src={avatarSrc(path)}
              alt=""
              draggable={false}
            />
          ) : null}
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

  if (path) {
    return (
      <>
        <button
          type="button"
          className={`media-card${sticker ? " sticker" : ""}`}
          title={t("media.open")}
          onClick={() => setOpen(true)}
        >
          <img
            src={avatarSrc(path)}
            alt={message.text ?? ""}
            draggable={false}
          />
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
        className={`media-placeholder${sticker ? " sticker" : ""}${
          state === "error" ? " error" : ""
        }`}
        title={
          state === "error" ? t("media.retryDownload") : t("media.download")
        }
        onClick={state === "loading" ? undefined : download}
      >
        {state === "loading" ? (
          <LoaderCircle size={26} className="spin" />
        ) : state === "error" ? (
          <RotateCw size={24} />
        ) : sticker ? (
          <ImageOff size={26} />
        ) : (
          <Download size={26} />
        )}
        <span>
          {state === "loading"
            ? t("media.downloading")
            : state === "error"
              ? t("media.downloadFailed")
              : sticker
                ? t("media.sticker")
                : t("media.photo")}
        </span>
        {state === "idle" && autoDownloadOff ? (
          <span className="media-hint">{t("media.autoDownloadOff")}</span>
        ) : null}
      </button>
      {caption}
    </>
  );
}
