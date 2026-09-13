import {
  Phone,
  PhoneIncoming,
  PhoneMissed,
  PhoneOutgoing,
  Video,
} from "lucide-react";
import { initials } from "../../lib/names";
import { formatListTime } from "../../lib/time";
import type { ChatSummary } from "../../lib/types";
import { useAppStore, type CallHistoryEntry } from "../../store/app";
import { EmptyState, ScreenHeader } from "./shared";

const directionLabels = {
  missed: "Missed",
  outgoing: "Outgoing",
  incoming: "Incoming",
} as const;

/** Session call history, newest first. Phone-synced history lands later. */
export function CallsScreen() {
  const callHistory = useAppStore((state) => state.callHistory);
  const chats = useAppStore((state) => state.chats);

  return (
    <section className="chat-list screen">
      <ScreenHeader title="Calls" />

      <div className="screen-body">
        {callHistory.length > 0 ? (
          <>
            <div className="screen-section-label">Recent</div>
            {callHistory.map((call) => (
              <CallRow
                key={call.id}
                call={call}
                name={resolveName(call.chatId, chats)}
              />
            ))}
            <p className="screen-notice calls-sync-note">
              Only calls from this session are listed for now — syncing your
              phone's call history lands in a later build.
            </p>
          </>
        ) : (
          <EmptyState
            icon={<Phone size={26} strokeWidth={1.5} />}
            title="No calls yet"
            hint="Calls you make or receive will appear here. Once call-log sync ships, your phone's history will show up too."
          />
        )}
      </div>
    </section>
  );
}

function CallRow({ call, name }: { call: CallHistoryEntry; name: string }) {
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
              {directionLabels[call.direction]}{" "}
              {call.video ? "video" : "voice"} call
              {call.durationSecs !== null
                ? ` · ${formatDuration(call.durationSecs)}`
                : ""}
            </span>
          </span>
          <span className="call-item-actions">
            <button
              type="button"
              className="icon-button"
              title={`Voice call ${name}`}
              onClick={() => startCall(call.chatId, false)}
            >
              <Phone size={18} />
            </button>
            <button
              type="button"
              className="icon-button"
              title={`Video call ${name}`}
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

/** Chat name when the chat is known, otherwise the JID's user part. */
function resolveName(chatId: string, chats: ChatSummary[]): string {
  return (
    chats.find((chat) => chat.id === chatId)?.name ??
    chatId.split("@")[0] ??
    chatId
  );
}

function formatDuration(total: number): string {
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) return `${hours} hr ${minutes} min`;
  if (minutes > 0) return `${minutes} min ${seconds} sec`;
  return `${seconds} sec`;
}
