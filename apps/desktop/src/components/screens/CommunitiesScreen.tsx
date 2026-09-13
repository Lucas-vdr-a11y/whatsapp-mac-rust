import { ChevronRight, Plus, Users } from "lucide-react";
import { initials } from "../../lib/names";
import { ScreenHeader } from "./shared";

interface CommunityGroup {
  id: string;
  name: string;
  unreadCount: number;
}

interface Community {
  id: string;
  name: string;
  members: string;
  groups: CommunityGroup[];
}

const COMMUNITIES: Community[] = [
  {
    id: "community-1",
    name: "Designers Academy",
    members: "1,284",
    groups: [
      { id: "group-1", name: "Announcements", unreadCount: 2 },
      { id: "group-2", name: "Feedback & critique", unreadCount: 0 },
      { id: "group-3", name: "Jobs board", unreadCount: 5 },
    ],
  },
  {
    id: "community-2",
    name: "Klimmuur Amsterdam",
    members: "376",
    groups: [
      { id: "group-4", name: "Sessions", unreadCount: 0 },
      { id: "group-5", name: "Gear swap", unreadCount: 1 },
    ],
  },
];

export function CommunitiesScreen() {
  return (
    <section className="chat-list screen">
      <ScreenHeader title="Communities">
        <button type="button" className="icon-button" title="New community">
          <Plus size={24} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <button type="button" className="screen-entry">
          <span className="screen-entry-icon">
            <Users size={22} />
          </span>
          <span className="screen-entry-body">
            <span className="screen-entry-title">New community</span>
            <span className="screen-entry-hint">
              Bring related groups together in one place
            </span>
          </span>
        </button>

        <div className="screen-section-label">Your communities</div>
        {COMMUNITIES.map((community) => (
          <CommunityCard key={community.id} community={community} />
        ))}
      </div>
    </section>
  );
}

function CommunityCard({ community }: { community: Community }) {
  return (
    <div className="community-card">
      <button type="button" className="community-card-header">
        <span className="avatar community-avatar">
          {initials(community.name)}
        </span>
        <span className="community-card-body">
          <span className="community-name">{community.name}</span>
          <span className="community-meta">{community.members} members</span>
        </span>
        <ChevronRight size={18} className="community-chevron" />
      </button>

      <div className="community-groups">
        {community.groups.map((group) => (
          <div className="community-group" key={group.id}>
            <span className="avatar community-group-avatar">
              {initials(group.name)}
            </span>
            <span className="community-group-name">{group.name}</span>
            {group.unreadCount > 0 && (
              <span className="chat-item-badge">{group.unreadCount}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
