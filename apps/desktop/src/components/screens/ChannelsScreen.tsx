/**
 * Channels screen: follow a channel by invite link and manage the followed
 * list. Inside the Tauri host the list is derived from `list_chats` (chat ids
 * ending in `@newsletter`); the browser preview keeps the demo list.
 */

import { useCallback, useEffect, useState } from "react";
import {
  BadgeCheck,
  Compass,
  EllipsisVertical,
  RadioTower,
  UserMinus,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { initials } from "../../lib/names";
import { formatListTime } from "../../lib/time";
import type { ChatSummary, Jid } from "../../lib/types";
import { ContextMenu, type ContextMenuEntry } from "../ContextMenu";
import { ConfirmDialog } from "../settings/ConfirmDialog";
import { EmptyState, ScreenHeader } from "./shared";

interface Channel {
  id: string;
  name: string;
  /** Follower count when known (demo rows); core-derived rows hide it. */
  followers: string | null;
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

/** Channel invite URL, with or without the scheme. */
const CHANNEL_URL_RE =
  /^(?:https?:\/\/)?(?:www\.)?whatsapp\.com\/channel\/([A-Za-z0-9_-]+)\/?$/i;
/** A bare invite code pasted without the URL wrapper. */
const CHANNEL_CODE_RE = /^[A-Za-z0-9_-]{6,}$/;

/**
 * Normalises user input to the canonical
 * `https://whatsapp.com/channel/<code>` URL. Returns null when the text is
 * neither a channel link nor a plausible raw invite code.
 */
function normalizeInvite(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;

  const urlMatch = CHANNEL_URL_RE.exec(trimmed);
  if (urlMatch) return `https://whatsapp.com/channel/${urlMatch[1]}`;

  if (CHANNEL_CODE_RE.test(trimmed)) {
    return `https://whatsapp.com/channel/${trimmed}`;
  }
  return null;
}

function chatToChannel(chat: ChatSummary): Channel {
  return {
    id: chat.id,
    name: chat.name,
    followers: null,
    preview: chat.lastMessagePreview ?? "No updates yet",
    at: chat.lastActivityTs > 0 ? formatListTime(chat.lastActivityTs) : "",
  };
}

/** Short, non-technical message for a failed channel command. */
function friendlyError(error: unknown, fallback: string): string {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "";
  if (!raw) return fallback;
  if (
    /not linked|no session|session (is )?(closed|missing)|not paired|disconnected/i.test(
      raw,
    )
  ) {
    return "Channels needs a linked WhatsApp session. Link your phone in Settings, then try again.";
  }
  if (/not implemented|unknown command|unrecognized/i.test(raw)) {
    return "Channels aren't available in this build yet.";
  }
  return raw;
}

export function ChannelsScreen() {
  const [channels, setChannels] = useState<Channel[]>(() =>
    isTauri() ? [] : FOLLOWED_CHANNELS,
  );
  const [loading, setLoading] = useState(() => isTauri());
  const [listError, setListError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [findOpen, setFindOpen] = useState(false);
  const [invite, setInvite] = useState("");
  const [following, setFollowing] = useState(false);
  const [followError, setFollowError] = useState<string | null>(null);

  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    channel: Channel;
  } | null>(null);
  const [pendingUnfollow, setPendingUnfollow] = useState<Channel | null>(null);
  const [unfollowing, setUnfollowing] = useState(false);
  const [unfollowError, setUnfollowError] = useState<string | null>(null);

  const loadChannels = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    setListError(null);
    try {
      const chats = await invokeCore<ChatSummary[]>("list_chats");
      setChannels(
        chats
          .filter((chat) => chat.id.endsWith("@newsletter"))
          .sort((a, b) => b.lastActivityTs - a.lastActivityTs)
          .map(chatToChannel),
      );
    } catch (error) {
      setListError(friendlyError(error, "Couldn't load your channels."));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadChannels();
  }, [loadChannels]);

  const toggleFind = () => {
    setFindOpen((open) => !open);
    setFollowError(null);
  };

  const handleFollow = async () => {
    const inviteUrl = normalizeInvite(invite);
    if (!inviteUrl) {
      setFollowError(
        "That doesn't look like a channel invite. Paste a whatsapp.com/channel/… link or its invite code.",
      );
      return;
    }

    setFollowing(true);
    setFollowError(null);
    setNotice(null);
    try {
      if (!isTauri()) {
        // Browser preview: show the new row without calling the core.
        const code = inviteUrl.split("/").pop() ?? "invite";
        const id = `${code}@newsletter`;
        setChannels((current) =>
          current.some((channel) => channel.id === id)
            ? current
            : [
                {
                  id,
                  name: "Demo channel",
                  followers: null,
                  preview: "Followed from an invite link (browser preview)",
                  at: "Just now",
                },
                ...current,
              ],
        );
        setNotice("Channel followed (browser preview).");
      } else {
        const jid = await invokeCore<Jid>("channels_follow", { inviteUrl });
        setNotice("Channel followed.");
        await loadChannels();
        // `list_chats` can lag behind the follow; keep the row visible.
        setChannels((current) =>
          current.some((channel) => channel.id === jid)
            ? current
            : [
                {
                  id: jid,
                  name: "New channel",
                  followers: null,
                  preview: "Followed — updates will appear here",
                  at: "Just now",
                },
                ...current,
              ],
        );
      }
      setInvite("");
      setFindOpen(false);
    } catch (error) {
      setFollowError(friendlyError(error, "Couldn't follow that channel."));
    } finally {
      setFollowing(false);
    }
  };

  const handleUnfollow = async () => {
    if (!pendingUnfollow || unfollowing) return;
    setUnfollowing(true);
    setUnfollowError(null);
    try {
      if (isTauri()) {
        await invokeCore<void>("channels_unfollow", {
          chatId: pendingUnfollow.id,
        });
      }
      setChannels((current) =>
        current.filter((channel) => channel.id !== pendingUnfollow.id),
      );
      setNotice(`Unfollowed ${pendingUnfollow.name}.`);
      setPendingUnfollow(null);
    } catch (error) {
      setUnfollowError(friendlyError(error, "Couldn't unfollow this channel."));
    } finally {
      setUnfollowing(false);
    }
  };

  const menuItems: ContextMenuEntry[] = menu
    ? [
        {
          id: "unfollow",
          label: "Unfollow channel",
          icon: UserMinus,
          danger: true,
          onSelect: () => {
            setUnfollowError(null);
            setPendingUnfollow(menu.channel);
          },
        },
      ]
    : [];

  return (
    <section className="chat-list screen">
      <ScreenHeader title="Channels">
        <button
          type="button"
          className="icon-button"
          title="Find channels"
          aria-expanded={findOpen}
          onClick={toggleFind}
        >
          <Compass size={24} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <p className="channels-lede">
          Stay updated on topics you care about. Channels are a one-way
          broadcast from people and organizations you follow.
        </p>

        <button
          type="button"
          className="screen-entry"
          aria-expanded={findOpen}
          onClick={toggleFind}
        >
          <span className="screen-entry-icon">
            <Compass size={22} />
          </span>
          <span className="screen-entry-body">
            <span className="screen-entry-title">Find channels</span>
            <span className="screen-entry-hint">
              {findOpen
                ? "Paste an invite link or code below"
                : "Follow a channel by its invite link"}
            </span>
          </span>
        </button>

        {findOpen ? (
          <form
            className="channels-find"
            onSubmit={(event) => {
              event.preventDefault();
              void handleFollow();
            }}
          >
            <div className="channels-find-row">
              <input
                className="channels-find-input"
                type="text"
                inputMode="url"
                placeholder="https://whatsapp.com/channel/…"
                aria-label="Channel invite link"
                value={invite}
                autoFocus
                spellCheck={false}
                autoComplete="off"
                onChange={(event) => {
                  setInvite(event.target.value);
                  setFollowError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape") toggleFind();
                }}
              />
              <button
                type="submit"
                className="modal-action primary"
                disabled={following || invite.trim().length === 0}
              >
                {following ? "Following…" : "Follow"}
              </button>
            </div>
            {followError ? (
              <p className="channels-find-error" role="alert">
                {followError}
              </p>
            ) : null}
          </form>
        ) : null}

        {notice ? (
          <p className="screen-notice" role="status">
            {notice}
          </p>
        ) : null}

        {listError ? (
          <div className="screen-inline-error" role="alert">
            <p>{listError}</p>
            <button
              type="button"
              className="modal-action secondary"
              onClick={() => void loadChannels()}
            >
              Retry
            </button>
          </div>
        ) : null}

        <div className="screen-section-label">Followed channels</div>

        {loading && channels.length === 0 ? (
          <p className="screen-loading">Loading channels…</p>
        ) : channels.length === 0 && !listError ? (
          <EmptyState
            icon={<RadioTower size={26} strokeWidth={1.5} />}
            title="No followed channels yet"
            hint="Follow a channel with its invite link and its updates will show up here."
          />
        ) : (
          channels.map((channel) => (
            <div
              className="channel-item"
              key={channel.id}
              onContextMenu={(event) => {
                event.preventDefault();
                setMenu({ x: event.clientX, y: event.clientY, channel });
              }}
            >
              <div className="avatar">{initials(channel.name)}</div>

              <div className="channel-body">
                <div className="channel-top">
                  <span className="channel-name">{channel.name}</span>
                  <BadgeCheck size={16} className="channel-verified" />
                  <span className="channel-time">{channel.at}</span>
                </div>
                <div className="channel-bottom">
                  {channel.followers ? (
                    <>
                      <span className="channel-followers">
                        {channel.followers}
                      </span>
                      <span className="channel-sep">·</span>
                    </>
                  ) : null}
                  <span className="channel-preview">{channel.preview}</span>
                </div>
              </div>

              <button
                type="button"
                className="icon-button channel-item-menu"
                title={`More options for ${channel.name}`}
                aria-label={`More options for ${channel.name}`}
                onClick={(event) => {
                  const rect = event.currentTarget.getBoundingClientRect();
                  setMenu({ x: rect.right, y: rect.bottom + 4, channel });
                }}
              >
                <EllipsisVertical size={18} />
              </button>
            </div>
          ))
        )}
      </div>

      {menu ? (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={() => setMenu(null)}
        />
      ) : null}

      <ConfirmDialog
        open={pendingUnfollow !== null}
        title="Unfollow channel?"
        body={
          pendingUnfollow
            ? `You will stop receiving updates from ${pendingUnfollow.name}. You can follow again later with an invite link.`
            : ""
        }
        confirmLabel="Unfollow"
        danger
        busy={unfollowing}
        error={unfollowError}
        onConfirm={() => void handleUnfollow()}
        onCancel={() => {
          if (unfollowing) return;
          setPendingUnfollow(null);
          setUnfollowError(null);
        }}
      />
    </section>
  );
}
