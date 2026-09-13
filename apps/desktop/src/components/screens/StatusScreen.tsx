import {
  BellOff,
  Camera,
  ChevronRight,
  Pencil,
  Plus,
  UserRound,
} from "lucide-react";
import { initials } from "../../lib/names";
import { ScreenHeader } from "./shared";

interface StatusUpdate {
  id: string;
  name: string;
  at: string;
  viewed: boolean;
}

const UPDATES: StatusUpdate[] = [
  { id: "status-1", name: "Maya de Vries", at: "Today, 9:41 AM", viewed: false },
  { id: "status-2", name: "Design Weekly", at: "Today, 8:15 AM", viewed: false },
  { id: "status-3", name: "Tom Bakker", at: "Yesterday, 9:03 PM", viewed: false },
  { id: "status-4", name: "Priya Nair", at: "Yesterday, 6:22 PM", viewed: true },
  { id: "status-5", name: "Sanne & Bas", at: "Yesterday, 12:40 PM", viewed: true },
];

const MUTED_UPDATES = 3;

export function StatusScreen() {
  const recent = UPDATES.filter((update) => !update.viewed);
  const viewed = UPDATES.filter((update) => update.viewed);

  return (
    <section className="chat-list screen">
      <ScreenHeader title="Status">
        <button type="button" className="icon-button" title="Text status">
          <Pencil size={22} />
        </button>
        <button type="button" className="icon-button" title="Add to my status">
          <Camera size={22} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <div className="status-item">
          <span className="status-ring dashed">
            <span className="avatar">
              <UserRound size={24} strokeWidth={1.6} />
            </span>
            <span className="status-add-badge">
              <Plus size={13} strokeWidth={3} />
            </span>
          </span>
          <span className="status-item-body">
            <span className="status-item-name">My status</span>
            <span className="status-item-preview">
              Click to add status update
            </span>
          </span>
        </div>

        <div className="screen-section-label accent">Recent</div>
        {recent.map((update) => (
          <StatusRow key={update.id} update={update} />
        ))}

        <div className="screen-section-label">Viewed</div>
        {viewed.map((update) => (
          <StatusRow key={update.id} update={update} />
        ))}

        <button type="button" className="status-muted-header">
          <BellOff size={16} />
          <span>Muted updates</span>
          <span className="status-muted-count">{MUTED_UPDATES}</span>
          <ChevronRight size={16} className="status-muted-chevron" />
        </button>
      </div>
    </section>
  );
}

function StatusRow({ update }: { update: StatusUpdate }) {
  return (
    <div className="status-item">
      <span className={`status-ring${update.viewed ? " viewed" : ""}`}>
        <span className="avatar">{initials(update.name)}</span>
      </span>
      <span className="status-item-body">
        <span className="status-item-name">{update.name}</span>
        <span className="status-item-preview">{update.at}</span>
      </span>
    </div>
  );
}
