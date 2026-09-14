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
import { t, useTranslation } from "../../lib/i18n";
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
    preview: chat.lastMessagePreview ?? t("channels.noUpdates"),
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
    return t("channels.needsSession");
  }
  if (/not implemented|unknown command|unrecognized/i.test(raw)) {
    return t("channels.unavailable");
  }
  return raw;
}

export function ChannelsScreen() {
  const { t } = useTranslation();
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
      setListError(friendlyError(error, t("channels.loadError")));
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
      setFollowError(t("channels.invalidInvite"));
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
                  name: t("channels.demoChannel"),
                  followers: null,
                  preview: t("channels.followedRowPreview"),
                  at: t("channels.justNow"),
                },
                ...current,
              ],
        );
        setNotice(t("channels.followedPreview"));
      } else {
        const jid = await invokeCore<Jid>("channels_follow", { inviteUrl });
        setNotice(t("channels.followed"));
        await loadChannels();
        // `list_chats` can lag behind the follow; keep the row visible.
        setChannels((current) =>
          current.some((channel) => channel.id === jid)
            ? current
            : [
                {
                  id: jid,
                  name: t("channels.newChannel"),
                  followers: null,
                  preview: t("channels.newChannelPreview"),
                  at: t("channels.justNow"),
                },
                ...current,
              ],
        );
      }
      setInvite("");
      setFindOpen(false);
    } catch (error) {
      setFollowError(friendlyError(error, t("channels.followError")));
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
      setNotice(t("channels.unfollowed", { name: pendingUnfollow.name }));
      setPendingUnfollow(null);
    } catch (error) {
      setUnfollowError(
        friendlyError(error, t("channels.unfollowError")),
      );
    } finally {
      setUnfollowing(false);
    }
  };

  const menuItems: ContextMenuEntry[] = menu
    ? [
        {
          id: "unfollow",
          label: t("channels.unfollow"),
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
      <ScreenHeader title={t("channels.title")}>
        <button
          type="button"
          className="icon-button"
          title={t("channels.find")}
          aria-expanded={findOpen}
          onClick={toggleFind}
        >
          <Compass size={24} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <p className="channels-lede">{t("channels.lede")}</p>

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
            <span className="screen-entry-title">{t("channels.find")}</span>
            <span className="screen-entry-hint">
              {findOpen
                ? t("channels.findHintOpen")
                : t("channels.findHintClosed")}
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
                aria-label={t("channels.inviteAria")}
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
                {following ? t("channels.following") : t("channels.follow")}
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
              {t("common.retry")}
            </button>
          </div>
        ) : null}

        <div className="screen-section-label">
          {t("channels.followedChannels")}
        </div>

        {loading && channels.length === 0 ? (
          <p className="screen-loading">{t("channels.loading")}</p>
        ) : channels.length === 0 && !listError ? (
          <EmptyState
            icon={<RadioTower size={26} strokeWidth={1.5} />}
            title={t("channels.emptyTitle")}
            hint={t("channels.emptyHint")}
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
                title={t("channels.moreOptions", { name: channel.name })}
                aria-label={t("channels.moreOptions", { name: channel.name })}
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
        title={t("channels.unfollowTitle")}
        body={
          pendingUnfollow
            ? t("channels.unfollowBody", { name: pendingUnfollow.name })
            : ""
        }
        confirmLabel={t("channels.unfollowConfirm")}
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
