/** Media bubble for `video` messages.
 *
 * A muted first-frame tile with a play glyph. Clicking resolves the local file
 * through `media_download` (when needed) and opens the lightbox player. */

import { useState } from "react";
import { createPortal } from "react-dom";
import { LoaderCircle, Play, RotateCw } from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { Lightbox } from "./Lightbox";

type LoadState = "idle" | "loading" | "error";

export function VideoContent({ message }: { message: Message }) {
  const path = useAppStore((state) => state.mediaPaths[message.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [state, setState] = useState<LoadState>("idle");
  const [open, setOpen] = useState(false);

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
          </span>
        )}
      </button>
      {message.text ? (
        <div className="media-caption">{message.text}</div>
      ) : null}
      {open && path
        ? createPortal(
            <Lightbox
              kind="video"
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
