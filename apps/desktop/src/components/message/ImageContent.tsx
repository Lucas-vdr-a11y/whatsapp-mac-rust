/** Media bubble for `image`, `gif` and `sticker` messages.
 *
 * Shows a placeholder with a download affordance until `media_download`
 * resolves a local path, then a bounded rounded preview that opens in the
 * lightbox. Stickers render small and without bubble chrome. */

import { useState } from "react";
import { createPortal } from "react-dom";
import { Download, ImageOff, LoaderCircle, RotateCw } from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { Lightbox } from "./Lightbox";

type LoadState = "idle" | "loading" | "error";

interface ImageContentProps {
  message: Message;
  sticker?: boolean;
}

export function ImageContent({ message, sticker = false }: ImageContentProps) {
  const path = useAppStore((state) => state.mediaPaths[message.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [state, setState] = useState<LoadState>("idle");
  const [open, setOpen] = useState(false);

  const download = () => {
    setState("loading");
    downloadMedia(message.id, message.kind)
      .then(() => setState("idle"))
      .catch(() => setState("error"));
  };

  const caption = message.text ? (
    <div className="media-caption">{message.text}</div>
  ) : null;

  if (path) {
    return (
      <>
        <button
          type="button"
          className={`media-card${sticker ? " sticker" : ""}`}
          title="Open"
          onClick={() => setOpen(true)}
        >
          <img
            src={avatarSrc(path)}
            alt={message.text ?? ""}
            draggable={false}
          />
        </button>
        {caption}
        {open
          ? createPortal(
              <Lightbox
                kind="image"
                src={avatarSrc(path)}
                caption={message.text}
                onClose={() => setOpen(false)}
              />,
              document.body,
            )
          : null}
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
        title={state === "error" ? "Retry download" : "Download"}
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
            ? "Downloading…"
            : state === "error"
              ? "Download failed — tap to retry"
              : sticker
                ? "Sticker"
                : "Photo"}
        </span>
      </button>
      {caption}
    </>
  );
}
