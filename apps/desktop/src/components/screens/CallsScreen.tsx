import { useEffect, useState } from "react";
import {
  Phone,
  PhoneIncoming,
  PhoneMissed,
  PhoneOutgoing,
  Video,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { t, useTranslation } from "../../lib/i18n";
import { initials } from "../../lib/names";
import { formatListTime } from "../../lib/time";
import type { ChatSummary } from "../../lib/types";
import { useAppStore, type CallHistoryEntry } from "../../store/app";
import { EmptyState, ScreenHeader } from "./shared";

const directionKeys = {
  missed: "calls.direction.missed",
  outgoing: "calls.direction.outgoing",
  incoming: "calls.direction.incoming",
} as const;

type Direction = keyof typeof directionKeys;

/** Result values of `calls::CallLogResult` over IPC (camelCase). */
type CallLogResult =
  | "connected"
  | "missed"
  | "rejected"
  | "cancelled"
  | "acceptedElsewhere"
  | "failed"
  | "unavailable"
  | "upcoming"
  | "abandoned"
  | "ongoing"
  | "invalid"
  | "unknown";

/** One phone-synced row (`call_log_list` → `calls::CallLogEntry`). */
interface SyncedCallLogEntry {
  callId: string | null;
  participants: string[];
  groupJid: string | null;
  callCreator: string | null;
  incoming: boolean | null;
  video: boolean;
  callLink: boolean;
  result: CallLogResult;
  durationSecs: number | null;
  startedAtUnix: number | null;
}

/**
 * Calls screen: the session's live history plus the phone's synced call log.
 * Synced rows come from `call_log_list` (degrades honestly when the core
 * reports the store hook is unwired or the app is off).
 */
export function CallsScreen() {
  const { t } = useTranslation();
  const callHistory = useAppStore((state) => state.callHistory);
  const chats = useAppStore((state) => state.chats);
  const [synced, setSynced] = useState<SyncedCallLogEntry[] | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const desktop = isTauri();

  useEffect(() => {
    if (!desktop) {
      // Browser preview has no core: don't pretend there is synced history.
      setSynced([]);
      return;
    }
    let cancelled = false;
    void invokeCore<SyncedCallLogEntry[]>("call_log_list", { limit: 200 })
      .then((calls) => {
        if (cancelled) return;
        setSynced(calls);
        setSyncError(null);
      })
      .catch((cause) => {
        if (!cancelled) setSyncError(friendlyCallLogError(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [desktop]);

  const hasSessionCalls = callHistory.length > 0;
  const hasSyncedCalls = synced !== null && synced.length > 0;
  const nothingAtAll = !hasSessionCalls && !hasSyncedCalls && !syncError && synced !== null;

  return (
    <section className="chat-list screen">
      <ScreenHeader title={t("calls.title")} />

      <div className="screen-body">
        {nothingAtAll ? (
          <EmptyState
            icon={<Phone size={26} strokeWidth={1.5} />}
            title={t("calls.emptyTitle")}
            hint={
              desktop ? t("calls.emptyHintDesktop") : t("calls.emptyHint")
            }
          />
        ) : (
          <>
            {hasSessionCalls && (
              <>
                <div className="screen-section-label">
                  {t("calls.thisSession")}
                </div>
                {callHistory.map((call) => (
                  <CallRow
                    key={call.id}
                    call={call}
                    name={resolveName(call.chatId, chats)}
                  />
                ))}
              </>
            )}

            <div className="screen-section-label">{t("calls.recent")}</div>
            {syncError ? (
              <p className="screen-inline-error">{syncError}</p>
            ) : synced === null ? (
              <p className="screen-loading">{t("calls.loading")}</p>
            ) : hasSyncedCalls ? (
              synced.map((call) => (
                <SyncedCallRow
                  key={
                    call.callId ??
                    `${call.startedAtUnix}-${call.participants[0] ?? "unknown"}`
                  }
                  call={call}
                  chats={chats}
                />
              ))
            ) : (
              <p className="screen-notice calls-sync-note">
                {desktop ? t("calls.syncEmptyDesktop") : t("calls.syncEmptyPreview")}
              </p>
            )}
          </>
        )}
      </div>
    </section>
  );
}

/** Session entry: same row as before, with call-back actions. */
function CallRow({ call, name }: { call: CallHistoryEntry; name: string }) {
  const { t } = useTranslation();
  const startCall = useAppStore((state) => state.startCall);
  const DirectionIcon =
    call.direction === "missed"
      ? PhoneMissed
      : call.direction === "outgoing"
        ? PhoneOutgoing
        : PhoneIncoming;

  return (
    <div className="call-item">
      <div className="avatar">{initials(name)}</div>

      <div className="call-item-body">
        <div className="call-item-top">
          <span className="call-item-name">{name}</span>
          <span className="call-item-time">{formatListTime(call.startedAt)}</span>
        </div>
        <div className="call-item-bottom">
          <span className={`call-status ${call.direction}`}>
            <DirectionIcon size={15} />
            <span className="call-status-text">
              {t(
                `calls.row.${call.direction}.${call.video ? "video" : "voice"}`,
              )}
              {call.durationSecs !== null
                ? ` · ${formatDuration(call.durationSecs)}`
                : ""}
            </span>
          </span>
          <span className="call-item-actions">
            <button
              type="button"
              className="icon-button"
              title={t("calls.voiceCallTitle", { name })}
              onClick={() => startCall(call.chatId, false)}
            >
              <Phone size={18} />
            </button>
            <button
              type="button"
              className="icon-button"
              title={t("calls.videoCallTitle", { name })}
              onClick={() => startCall(call.chatId, true)}
            >
              <Video size={18} />
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}

/** Phone-synced entry: opens the chat when its JID is known to the store. */
function SyncedCallRow({
  call,
  chats,
}: {
  call: SyncedCallLogEntry;
  chats: ChatSummary[];
}) {
  const { t } = useTranslation();
  const selectChat = useAppStore((state) => state.selectChat);
  const chatId = call.groupJid ?? call.participants[0] ?? null;
  const chat = chatId ? chats.find((item) => item.id === chatId) : undefined;
  const name = chat?.name ?? chatId?.split("@")[0] ?? t("calls.unknownCaller");
  const direction = directionOf(call);
  const DirectionIcon =
    direction === "missed"
      ? PhoneMissed
      : direction === "outgoing"
        ? PhoneOutgoing
        : PhoneIncoming;
  const startedAt = call.startedAtUnix ?? 0;
  const duration =
    call.durationSecs !== null && call.durationSecs > 0
      ? ` · ${formatDuration(call.durationSecs)}`
      : "";

  const open = chat
    ? () => {
        selectChat(chat.id);
      }
    : undefined;

  return (
    <div
      className="call-item"
      role={open ? "button" : undefined}
      tabIndex={open ? 0 : undefined}
      style={open ? { cursor: "pointer" } : undefined}
      title={open ? t("calls.openChatWith", { name }) : undefined}
      onClick={open}
      onKeyDown={
        open
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                open();
              }
            }
          : undefined
      }
    >
      <div className="avatar">{initials(name)}</div>

      <div className="call-item-body">
        <div className="call-item-top">
          <span className="call-item-name">{name}</span>
          <span className="call-item-time">
            {formatRelativeTime(startedAt)}
          </span>
        </div>
        <div className="call-item-bottom">
          <span className={`call-status ${direction}`}>
            <DirectionIcon size={15} />
            <span className="call-status-text">
              {describeCall(call, direction)}
              {duration}
            </span>
          </span>
        </div>
      </div>
    </div>
  );
}

/** Missed-like results always get the red missed icon. */
function directionOf(call: SyncedCallLogEntry): Direction {
  if (
    call.result === "missed" ||
    call.result === "unavailable" ||
    call.result === "abandoned"
  ) {
    return "missed";
  }
  return call.incoming === false ? "outgoing" : "incoming";
}

/** Human-readable outcome, keeping the direction icon's voice. */
function describeCall(call: SyncedCallLogEntry, direction: Direction): string {
  const kind = call.video ? "video" : "voice";
  switch (call.result) {
    case "missed":
      return direction === "outgoing"
        ? t(`calls.result.unanswered.${kind}`)
        : t(`calls.result.missed.${kind}`);
    case "rejected":
      return t(`calls.result.declined.${kind}`);
    case "cancelled":
    case "abandoned":
      return t(`calls.result.cancelled.${kind}`);
    case "failed":
      return t(`calls.result.failed.${kind}`);
    case "unavailable":
      return t(`calls.result.unavailable.${kind}`);
    case "ongoing":
      return t(`calls.result.ongoing.${kind}`);
    case "upcoming":
      return t(`calls.result.scheduled.${kind}`);
    case "acceptedElsewhere":
      return t("calls.result.answeredElsewhere");
    case "invalid":
    case "unknown":
    default:
      return t(`calls.row.${direction}.${kind}`);
  }
}

/** Chat name when the chat is known, otherwise the JID's user part. */
function resolveName(chatId: string, chats: ChatSummary[]): string {
  return (
    chats.find((chat) => chat.id === chatId)?.name ??
    chatId.split("@")[0] ??
    chatId
  );
}

/** Compact "5 min ago" stamp for synced rows; falls back to a date. */
function formatRelativeTime(unixSeconds: number): string {
  if (!unixSeconds) return "";
  const elapsed = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
  if (elapsed < 60) return t("calls.time.justNow");
  const minutes = Math.floor(elapsed / 60);
  if (minutes < 60) return t("calls.time.minutes", { minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("calls.time.hours", { hours });
  const days = Math.floor(hours / 24);
  if (days === 1) return t("calls.time.yesterday");
  if (days < 7) return t("calls.time.days", { days });
  return new Date(unixSeconds * 1000).toLocaleDateString([], {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
  });
}

/** Turn a `call_log_list` rejection into copy that doesn't leak internals. */
function friendlyCallLogError(cause: unknown): string {
  const raw =
    cause instanceof Error
      ? cause.message
      : typeof cause === "string"
        ? cause
        : "";
  const lower = raw.toLowerCase();
  if (lower.includes("not wired") || lower.includes("call-log storage")) {
    return t("calls.error.build");
  }
  if (lower.includes("not connected")) {
    return t("calls.error.connect");
  }
  return raw || t("calls.error.load");
}

function formatDuration(total: number): string {
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) return t("calls.duration.hm", { hours, minutes });
  if (minutes > 0) return t("calls.duration.ms", { minutes, seconds });
  return t("calls.duration.s", { seconds });
}
