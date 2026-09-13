/** File-row bubble for `audio` and `document` messages.
 *
 * Shows the file name/size once `media_download` has reported metadata and a
 * neutral label before that, plus a download action with progress/retry. */

import { useState } from "react";
import {
  Check,
  Download,
  FileText,
  LoaderCircle,
  Music,
  RotateCw,
} from "lucide-react";
import { useTranslation } from "../../lib/i18n";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { formatBytes } from "./media";

type LoadState = "idle" | "loading" | "error";

export function FileContent({ message }: { message: Message }) {
  const { t } = useTranslation();
  const meta = useAppStore((state) => state.mediaMeta[message.id]);
  const path = useAppStore((state) => state.mediaPaths[message.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [state, setState] = useState<LoadState>("idle");

  const isAudio = message.kind === "audio";
  const kindLabel = isAudio ? t("media.audio") : t("media.document");
  const name =
    meta?.fileName ?? (isAudio ? t("media.audioMessage") : t("media.document"));
  const details = meta
    ? [formatBytes(meta.size), kindLabel].filter(Boolean).join(" · ")
    : kindLabel;

  const download = () => {
    setState("loading");
    downloadMedia(message.id, message.kind)
      .then(() => setState("idle"))
      .catch(() => setState("error"));
  };

  return (
    <>
      <div className="file-row">
        <span className={`file-icon${isAudio ? " audio" : ""}`}>
          {isAudio ? <Music size={20} /> : <FileText size={20} />}
        </span>
        <span className="file-info">
          <span className="file-name">{name}</span>
          <span className="file-sub">{details}</span>
        </span>
        <button
          type="button"
          className={`file-action${state === "error" ? " error" : ""}${
            path ? " done" : ""
          }`}
          title={
            path
              ? t("media.downloaded")
              : state === "error"
                ? t("media.retryDownload")
                : t("media.download")
          }
          onClick={path || state === "loading" ? undefined : download}
        >
          {path && state !== "error" ? (
            <Check size={20} />
          ) : state === "loading" ? (
            <LoaderCircle size={20} className="spin" />
          ) : state === "error" ? (
            <RotateCw size={20} />
          ) : (
            <Download size={20} />
          )}
        </button>
      </div>
      {message.text ? (
        <div className="media-caption">{message.text}</div>
      ) : null}
    </>
  );
}
