import { useEffect, useState, type ReactNode } from "react";
import { Mic, MicOff, Phone, PhoneOff, VideoOff } from "lucide-react";
import { avatarSrc } from "../../lib/avatar";
import { initials } from "../../lib/names";
import { useAppStore, type CallSession } from "../../store/app";

/**
 * Fullscreen call card. Rendered by `Conversation.tsx` (and its empty state),
 * so it is visible from anywhere in the app, independent of the selected chat.
 */
export function CallOverlay() {
  const call = useAppStore((state) => state.callState);
  const chats = useAppStore((state) => state.chats);
  const avatars = useAppStore((state) => state.avatars);
  const clearCall = useAppStore((state) => state.clearCall);
  const endCall = useAppStore((state) => state.endCall);
  const answerCall = useAppStore((state) => state.answerCall);
  const rejectCall = useAppStore((state) => state.rejectCall);
  const setCallMuted = useAppStore((state) => state.setCallMuted);
  const duration = useCallDuration(call.activeSince);

  // Terminal cards dismiss themselves: the core bridge clears protocol
  // ended/missed updates after ~2 s, this is the fallback for local failures
  // (e.g. calling not compiled in) and gives the copy time to be read.
  useEffect(() => {
    if (call.state !== "ended") return;
    const timer = window.setTimeout(() => clearCall(call.callId), 2600);
    return () => window.clearTimeout(timer);
  }, [call.state, call.callId, clearCall]);

  // Escape hangs up an outgoing/active call or dismisses the ended card. An
  // incoming call must be answered or declined explicitly.
  useEffect(() => {
    if (call.state === "idle" || call.state === "ringing-in") return;
    const handleKeyDown = (event: KeyboardEvent) => {
      // Let the composer's own Escape handling (cancel edit/reply) win.
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (call.state === "ended") clearCall(call.callId);
      else if (call.chatId) endCall(call.chatId);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [call.state, call.callId, call.chatId, clearCall, endCall]);

  if (call.state === "idle") return null;

  const chat = call.chatId
    ? chats.find((candidate) => candidate.id === call.chatId)
    : undefined;
  const name = chat?.name ?? call.chatId?.split("@")[0] ?? "Unknown contact";
  const avatar = call.chatId ? avatars[call.chatId] : undefined;
  const video = call.info?.video ?? false;
  const missed = call.reason === "Missed call";
  const kind = video ? "Video call" : "Voice call";

  return (
    <div
      className="call-overlay"
      role="dialog"
      aria-modal="true"
      aria-label={`${kind} with ${name}`}
    >
      <div className={`call-card${video ? " video" : ""}`}>
        {video ? (
          <div className="call-video-tile">
            <VideoOff size={40} strokeWidth={1.5} />
            <span className="call-video-note">
              Video preview isn't wired up yet — this build carries the audio
              call only.
            </span>
          </div>
        ) : (
          <div className="call-avatar">
            {avatar ? <img src={avatarSrc(avatar)} alt="" /> : initials(name)}
          </div>
        )}

        <div className="call-identity">
          <span className="call-name">{name}</span>
          <span className={`call-state${call.state === "active" ? " timer" : ""}`}>
            {callHeadline(call, duration)}
          </span>
          <span className="call-kind">
            {call.state === "ringing-in"
              ? `Incoming ${kind.toLowerCase()}`
              : kind}
          </span>
          {call.reason && !missed ? (
            <span className="call-reason" role="alert">
              {call.reason}
            </span>
          ) : null}
        </div>

        <div className="call-controls">
          {call.state === "ringing-in" ? (
            <>
              <Control
                label="Decline"
                className="hangup"
                icon={<PhoneOff size={26} />}
                onClick={() => {
                  if (call.callId) rejectCall(call.callId);
                }}
              />
              <Control
                label="Accept"
                className="accept"
                icon={<Phone size={26} />}
                onClick={() => {
                  if (call.callId) answerCall(call.callId);
                }}
              />
            </>
          ) : call.state === "ended" ? (
            <Control
              label="Close"
              className="neutral"
              icon={<PhoneOff size={26} />}
              onClick={() => clearCall(call.callId)}
            />
          ) : (
            <>
              <Control
                label={call.muted ? "Unmute" : "Mute"}
                className={`neutral${call.muted ? " on" : ""}`}
                icon={call.muted ? <MicOff size={26} /> : <Mic size={26} />}
                onClick={() => setCallMuted(!call.muted)}
              />
              <Control
                label="Hang up"
                className="hangup"
                icon={<PhoneOff size={26} />}
                onClick={() => {
                  if (call.chatId) endCall(call.chatId);
                }}
              />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/** Headline for the current phase; active calls show the ticking timer. */
function callHeadline(call: CallSession, duration: string): string {
  switch (call.state) {
    case "ringing-in":
      return "Incoming call";
    case "ringing-out":
      return "Ringing…";
    case "connecting":
      return "Connecting…";
    case "active":
      return duration;
    case "ended":
      return call.reason === "Missed call" ? "Missed call" : "Call ended";
    default:
      return "";
  }
}

interface ControlProps {
  label: string;
  className: string;
  icon: ReactNode;
  onClick: () => void;
}

/** One round call control with its caption. */
function Control({ label, className, icon, onClick }: ControlProps) {
  return (
    <div className="call-action">
      <button
        type="button"
        className={`call-control ${className}`}
        title={label}
        aria-label={label}
        onClick={onClick}
      >
        {icon}
      </button>
      <span className="call-action-label">{label}</span>
    </div>
  );
}

/** `mm:ss` (or `h:mm:ss`) since `activeSince`, ticking every second. */
function useCallDuration(activeSince: number | undefined): string {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    if (activeSince === undefined) return;
    setNow(Math.floor(Date.now() / 1000));
    const timer = window.setInterval(
      () => setNow(Math.floor(Date.now() / 1000)),
      1000,
    );
    return () => window.clearInterval(timer);
  }, [activeSince]);

  if (activeSince === undefined) return "00:00";
  const total = Math.max(0, now - activeSince);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`;
}
