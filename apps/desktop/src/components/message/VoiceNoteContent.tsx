/** Voice-note bubble: mic glyph, deterministic waveform, duration and play.
 *
 * The first play resolves the local file through `media_download`; the audio
 * element then reports the real duration, so the duration text only appears
 * once the media file is available. */

import { useEffect, useMemo, useRef, useState } from "react";
import { LoaderCircle, Mic, Pause, Play, RotateCw } from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import { useTranslation } from "../../lib/i18n";
import type { Message } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { formatDuration, waveformBars } from "./media";

type LoadState = "idle" | "loading" | "error";

export function VoiceNoteContent({ message }: { message: Message }) {
  const { t } = useTranslation();
  const path = useAppStore((state) => state.mediaPaths[message.id]);
  const downloadMedia = useAppStore((state) => state.downloadMedia);
  const [state, setState] = useState<LoadState>("idle");
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState<number | null>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const playWhenReady = useRef(false);

  const bars = useMemo(() => waveformBars(message.id), [message.id]);

  // Auto-start once a requested download has resolved and the element has a
  // source.
  useEffect(() => {
    if (!path || !playWhenReady.current) return;
    playWhenReady.current = false;
    void audioRef.current?.play().catch(() => setPlaying(false));
  }, [path]);

  const toggle = () => {
    const audio = audioRef.current;
    if (!path || !audio) {
      if (state === "loading") return;
      playWhenReady.current = true;
      setState("loading");
      downloadMedia(message.id, message.kind)
        .then(() => setState("idle"))
        .catch(() => {
          playWhenReady.current = false;
          setState("error");
        });
      return;
    }
    if (audio.paused) void audio.play().catch(() => setPlaying(false));
    else audio.pause();
  };

  return (
    <div className="voice-row">
      <button
        type="button"
        className={`voice-play${state === "error" ? " error" : ""}`}
        title={
          state === "error"
            ? t("media.retryDownload")
            : playing
              ? t("media.pause")
              : t("media.play")
        }
        onClick={toggle}
      >
        {state === "loading" ? (
          <LoaderCircle size={20} className="spin" />
        ) : state === "error" ? (
          <RotateCw size={20} />
        ) : playing ? (
          <Pause size={20} />
        ) : (
          <Play size={20} />
        )}
      </button>

      <div className="voice-main">
        <Mic size={16} className="voice-mic" />
        <div className="voice-wave" role="presentation" onClick={toggle}>
          {bars.map((height, index) => (
            <span
              key={index}
              style={{ height: `${Math.round(height * 100)}%` }}
            />
          ))}
        </div>
        <span className={`voice-duration${state === "error" ? " error" : ""}`}>
          {state === "error"
            ? t("media.retryShort")
            : duration !== null
              ? formatDuration(duration)
              : "--:--"}
        </span>
      </div>

      <audio
        ref={audioRef}
        src={path ? avatarSrc(path) : undefined}
        preload="metadata"
        onLoadedMetadata={(event) => setDuration(event.currentTarget.duration)}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
      />
    </div>
  );
}
