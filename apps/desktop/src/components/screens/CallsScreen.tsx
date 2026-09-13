import {
  Link,
  Phone,
  PhoneIncoming,
  PhoneMissed,
  PhoneOutgoing,
  Plus,
  Video,
} from "lucide-react";
import { initials } from "../../lib/names";
import { EmptyState, ScreenHeader } from "./shared";

type CallDirection = "incoming" | "outgoing" | "missed";

interface CallLogEntry {
  id: string;
  name: string;
  direction: CallDirection;
  video: boolean;
  at: string;
}

const CALL_LOG: CallLogEntry[] = [
  {
    id: "call-1",
    name: "Maya de Vries",
    direction: "missed",
    video: true,
    at: "9:41 AM",
  },
  {
    id: "call-2",
    name: "Tom Bakker",
    direction: "outgoing",
    video: false,
    at: "Yesterday",
  },
  {
    id: "call-3",
    name: "Design Weekly",
    direction: "incoming",
    video: true,
    at: "Yesterday",
  },
  {
    id: "call-4",
    name: "Sanne & Bas",
    direction: "outgoing",
    video: false,
    at: "Tuesday",
  },
  {
    id: "call-5",
    name: "Priya Nair",
    direction: "missed",
    video: false,
    at: "Monday",
  },
];

const directionLabels: Record<CallDirection, string> = {
  missed: "Missed",
  outgoing: "Outgoing",
  incoming: "Incoming",
};

export function CallsScreen() {
  return (
    <section className="chat-list screen">
      <ScreenHeader title="Calls">
        <button type="button" className="icon-button" title="New call">
          <Plus size={24} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <button type="button" className="screen-entry">
          <span className="screen-entry-icon">
            <Link size={22} />
          </span>
          <span className="screen-entry-body">
            <span className="screen-entry-title">Start new call</span>
            <span className="screen-entry-hint">
              Share a call link with anyone you message
            </span>
          </span>
        </button>

        {CALL_LOG.length > 0 ? (
          <>
            <div className="screen-section-label">Recent</div>
            {CALL_LOG.map((call) => (
              <CallRow key={call.id} call={call} />
            ))}
          </>
        ) : (
          <EmptyState
            icon={<Phone size={26} strokeWidth={1.5} />}
            title="No calls yet"
            hint="Voice and video calls you make or receive will show up here."
          />
        )}
      </div>
    </section>
  );
}

function CallRow({ call }: { call: CallLogEntry }) {
  const DirectionIcon =
    call.direction === "missed"
      ? PhoneMissed
      : call.direction === "outgoing"
        ? PhoneOutgoing
        : PhoneIncoming;

  return (
    <div className="call-item">
      <div className="avatar">{initials(call.name)}</div>

      <div className="call-item-body">
        <div className="call-item-top">
          <span className="call-item-name">{call.name}</span>
          <span className="call-item-time">{call.at}</span>
        </div>
        <div className="call-item-bottom">
          <span className={`call-status ${call.direction}`}>
            <DirectionIcon size={15} />
            <span className="call-status-text">
              {directionLabels[call.direction]}{" "}
              {call.video ? "video" : "voice"} call
            </span>
          </span>
          <span className="call-item-actions">
            <button
              type="button"
              className="icon-button"
              title={`Voice call ${call.name}`}
            >
              <Phone size={18} />
            </button>
            <button
              type="button"
              className="icon-button"
              title={`Video call ${call.name}`}
            >
              <Video size={18} />
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}
