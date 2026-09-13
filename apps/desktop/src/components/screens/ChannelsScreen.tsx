import { BadgeCheck, Compass } from "lucide-react";
import { initials } from "../../lib/names";
import { ScreenHeader } from "./shared";

interface Channel {
  id: string;
  name: string;
  followers: string;
  preview: string;
  at: string;
}

const FOLLOWED_CHANNELS: Channel[] = [
  {
    id: "channel-1",
    name: "Tech Nieuws",
    followers: "1.2M followers",
    preview: "New: the chips behind on-device AI",
    at: "9:12 AM",
  },
  {
    id: "channel-2",
    name: "Amsterdam Culture",
    followers: "486K followers",
    preview: "Weekend tips: six exhibitions to visit",
    at: "Yesterday",
  },
  {
    id: "channel-3",
    name: "Rust Weekly",
    followers: "218K followers",
    preview: "This week in Rust: async runtimes compared",
    at: "Yesterday",
  },
  {
    id: "channel-4",
    name: "Formule 1 NL",
    followers: "94K followers",
    preview: "Race recap: the strategy calls that decided the podium",
    at: "Sunday",
  },
];

export function ChannelsScreen() {
  return (
    <section className="chat-list screen">
      <ScreenHeader title="Channels">
        <button type="button" className="icon-button" title="Explore channels">
          <Compass size={24} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <p className="channels-lede">
          Stay updated on topics you care about. Channels are a one-way
          broadcast from people and organizations you follow.
        </p>

        <button type="button" className="screen-entry">
          <span className="screen-entry-icon">
            <Compass size={22} />
          </span>
          <span className="screen-entry-body">
            <span className="screen-entry-title">Find channels</span>
            <span className="screen-entry-hint">
              Discover channels to follow
            </span>
          </span>
        </button>

        <div className="screen-section-label">Followed channels</div>
        {FOLLOWED_CHANNELS.map((channel) => (
          <div className="channel-item" key={channel.id}>
            <div className="avatar">{initials(channel.name)}</div>

            <div className="channel-body">
              <div className="channel-top">
                <span className="channel-name">{channel.name}</span>
                <BadgeCheck size={16} className="channel-verified" />
                <span className="channel-time">{channel.at}</span>
              </div>
              <div className="channel-bottom">
                <span className="channel-followers">{channel.followers}</span>
                <span className="channel-sep">·</span>
                <span className="channel-preview">{channel.preview}</span>
              </div>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
