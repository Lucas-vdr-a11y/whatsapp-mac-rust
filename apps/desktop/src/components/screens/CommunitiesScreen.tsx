/**
 * Communities screen: create a community, join one by invite link, and manage
 * the linked groups of the communities created or joined in this session.
 *
 * `ChatSummary` carries no community flag, so communities that already exist
 * on the phone cannot be told apart from regular groups in `list_chats` yet.
 * Until the core exposes one, this screen keeps its own list (sessionStorage)
 * and says so inline.
 */

import { useCallback, useEffect, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Copy,
  Info,
  Link2,
  Plus,
  Unlink,
  Users,
  X,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { t, useTranslation } from "../../lib/i18n";
import { initials } from "../../lib/names";
import type { ChatSummary, Jid } from "../../lib/types";
import { copyToClipboard } from "../groups/api";
import { ConfirmDialog } from "../settings/ConfirmDialog";
import { EmptyState, ScreenHeader } from "./shared";

/** Mirror of the core's `CommunityLinkedGroup` (camelCase over IPC). */
interface CommunityLinkedGroup {
  id: Jid;
  name: string;
  participantCount: number | null;
  isDefaultSubGroup: boolean;
  isGeneralChat: boolean;
}

/** Mirror of the core's `CommunityInfo` (camelCase over IPC). */
interface CommunityInfo {
  id: Jid;
  name: string;
  description: string | null;
  linkedGroups: CommunityLinkedGroup[];
  participantCount: number;
}

/** One entry in the screen's session-local community list. */
interface SessionCommunity {
  id: Jid;
  name: string;
  description: string | null;
}

interface GroupOption {
  id: Jid;
  name: string;
}

const STORAGE_KEY = "rustwa.communities.session";

/** Demo rows for the browser preview; Tauri reads the stored session list. */
const DEMO_COMMUNITIES: SessionCommunity[] = [
  {
    id: "community-1@g.us",
    name: "Designers Academy",
    description: "Critique, jobs and sessions for design folks.",
  },
  {
    id: "community-2@g.us",
    name: "Klimmuur Amsterdam",
    description: null,
  },
];

const DEMO_GROUPS: GroupOption[] = [
  { id: "group-1@g.us", name: "Announcements" },
  { id: "group-2@g.us", name: "Feedback & critique" },
  { id: "group-3@g.us", name: "Jobs board" },
  { id: "group-4@g.us", name: "Sessions" },
];

const NAME_MAX_LENGTH = 100;
const DESCRIPTION_MAX_LENGTH = 2048;

function isSessionCommunity(value: unknown): value is SessionCommunity {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.id === "string" &&
    typeof record.name === "string" &&
    (record.description === null || typeof record.description === "string")
  );
}

function loadSessionCommunities(): SessionCommunity[] {
  try {
    const raw = window.sessionStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isSessionCommunity);
  } catch {
    // Storage may be unavailable (private windows, disabled webview storage).
    return [];
  }
}

/** Short, non-technical message for a failed community command. */
function friendlyError(error: unknown, fallback: string): string {
  const raw =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "";
  if (!raw) return fallback;
  if (
    /not connected|not linked|no session|session (is )?(closed|missing)|not paired|disconnected/i.test(
      raw,
    )
  ) {
    return t("communities.needsSession");
  }
  if (/not implemented|unknown command|unrecognized/i.test(raw)) {
    return t("communities.unavailable");
  }
  return raw;
}

export function CommunitiesScreen() {
  const { t } = useTranslation();
  const tauri = isTauri();

  const [communities, setCommunities] = useState<SessionCommunity[]>(() =>
    tauri ? loadSessionCommunities() : DEMO_COMMUNITIES,
  );
  const [infos, setInfos] = useState<Record<string, CommunityInfo>>({});
  const [infoLoading, setInfoLoading] = useState<Record<string, boolean>>({});
  const [infoErrors, setInfoErrors] = useState<Record<string, string>>({});
  const [actionErrors, setActionErrors] = useState<Record<string, string>>({});
  const [expanded, setExpanded] = useState<Jid | null>(null);
  const [busyId, setBusyId] = useState<Jid | null>(null);

  const [groups, setGroups] = useState<GroupOption[]>(() =>
    tauri ? [] : DEMO_GROUPS,
  );
  const [groupChoice, setGroupChoice] = useState<Record<string, string>>({});

  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  const [joinOpen, setJoinOpen] = useState(false);
  const [invite, setInvite] = useState("");
  const [joining, setJoining] = useState(false);
  const [joinError, setJoinError] = useState<string | null>(null);

  const [notice, setNotice] = useState<string | null>(null);
  const [inviteLinks, setInviteLinks] = useState<Record<string, string>>({});
  const [copiedId, setCopiedId] = useState<Jid | null>(null);

  const [deactivating, setDeactivating] = useState<SessionCommunity | null>(
    null,
  );
  const [deactivateBusy, setDeactivateBusy] = useState(false);
  const [deactivateError, setDeactivateError] = useState<string | null>(null);

  // The list survives switching sections (the screen unmounts) but not an app
  // restart: it is a session cache, not the account's community list.
  useEffect(() => {
    if (!tauri) return;
    try {
      window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(communities));
    } catch {
      // Keep the list in memory when storage is unavailable.
    }
  }, [communities, tauri]);

  const loadInfo = useCallback(async (id: Jid) => {
    if (!isTauri()) return;
    setInfoLoading((current) => ({ ...current, [id]: true }));
    setInfoErrors((current) => {
      const next = { ...current };
      delete next[id];
      return next;
    });
    try {
      const info = await invokeCore<CommunityInfo>("communities_info", {
        communityId: id,
      });
      setInfos((current) => ({ ...current, [id]: info }));
      setCommunities((current) =>
        current.map((entry) =>
          entry.id === id
            ? {
                ...entry,
                name: info.name || entry.name,
                description: info.description,
              }
            : entry,
        ),
      );
    } catch (error) {
      setInfoErrors((current) => ({
        ...current,
        [id]: friendlyError(error, t("communities.loadError")),
      }));
    } finally {
      setInfoLoading((current) => ({ ...current, [id]: false }));
    }
  }, []);

  // Hydrate the stored list once per mount. Failures stay on the card.
  useEffect(() => {
    if (!isTauri()) return;
    for (const community of loadSessionCommunities()) {
      void loadInfo(community.id);
    }
  }, [loadInfo]);

  // The group picker targets the local chat list; communities are excluded
  // when a card builds its options.
  useEffect(() => {
    if (!isTauri()) return;
    void (async () => {
      try {
        const chats = await invokeCore<ChatSummary[]>("list_chats");
        setGroups(
          chats
            .filter((chat) => chat.isGroup)
            .sort((a, b) => a.name.localeCompare(b.name))
            .map((chat) => ({ id: chat.id, name: chat.name })),
        );
      } catch {
        // The picker simply stays empty; linking is best-effort here.
      }
    })();
  }, []);

  const openCreate = () => {
    setCreateError(null);
    setCreateOpen(true);
  };

  const closeCreate = () => {
    if (creating) return;
    setCreateOpen(false);
    setCreateError(null);
  };

  useEffect(() => {
    if (!createOpen) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !creating) {
        setCreateOpen(false);
        setCreateError(null);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [createOpen, creating]);

  const toggleJoin = () => {
    setJoinOpen((open) => !open);
    setJoinError(null);
  };

  const setActionError = (id: Jid, message: string | null) => {
    setActionErrors((current) => {
      const next = { ...current };
      if (message === null) {
        delete next[id];
      } else {
        next[id] = message;
      }
      return next;
    });
  };

  const handleToggle = (community: SessionCommunity) => {
    const next = expanded === community.id ? null : community.id;
    setExpanded(next);
    if (
      next !== null &&
      isTauri() &&
      !infos[community.id] &&
      !infoLoading[community.id]
    ) {
      void loadInfo(community.id);
    }
  };

  const handleCreate = async () => {
    const trimmedName = name.trim();
    if (!trimmedName || creating) return;

    setCreating(true);
    setCreateError(null);
    try {
      const trimmedDescription = description.trim();
      let id: Jid;
      if (isTauri()) {
        id = await invokeCore<Jid>("communities_create", {
          name: trimmedName,
          description: trimmedDescription || null,
        });
      } else {
        id = `community-${Date.now()}@g.us`;
      }

      const entry: SessionCommunity = {
        id,
        name: trimmedName,
        description: trimmedDescription || null,
      };
      setCommunities((current) => [
        entry,
        ...current.filter((item) => item.id !== id),
      ]);
      setName("");
      setDescription("");
      // Close directly: `closeCreate` is guarded by `creating`, which is
      // still true until the finally block below runs.
      setCreateOpen(false);
      setCreateError(null);
      setNotice(t("communities.created", { name: trimmedName }));
      if (isTauri()) void loadInfo(id);
    } catch (error) {
      setCreateError(friendlyError(error, t("communities.createError")));
    } finally {
      setCreating(false);
    }
  };

  const handleJoin = async () => {
    const trimmed = invite.trim();
    if (!trimmed || joining) return;

    setJoining(true);
    setJoinError(null);
    try {
      let id: Jid;
      if (isTauri()) {
        id = await invokeCore<Jid>("communities_join", {
          inviteUrl: trimmed,
        });
      } else {
        id = `community-${Date.now()}@g.us`;
      }

      if (communities.some((entry) => entry.id === id)) {
        setNotice(t("communities.alreadyJoined"));
      } else {
        setCommunities((current) => [
          { id, name: t("communities.joinedName"), description: null },
          ...current,
        ]);
        setNotice(t("communities.joined"));
        if (isTauri()) void loadInfo(id);
      }
      setInvite("");
      setJoinOpen(false);
    } catch (error) {
      setJoinError(friendlyError(error, t("communities.joinError")));
    } finally {
      setJoining(false);
    }
  };

  const handleInviteLink = async (community: SessionCommunity) => {
    if (busyId) return;
    setBusyId(community.id);
    setActionError(community.id, null);
    try {
      let link: string;
      if (isTauri()) {
        link = await invokeCore<string>("communities_invite_link", {
          communityId: community.id,
        });
      } else {
        link = `https://chat.whatsapp.com/${community.id
          .split("@")[0]
          .toUpperCase()}DEMO`;
      }
      setInviteLinks((current) => ({ ...current, [community.id]: link }));
    } catch (error) {
      setActionError(
        community.id,
        friendlyError(error, t("communities.inviteError")),
      );
    } finally {
      setBusyId(null);
    }
  };

  const handleCopyLink = async (community: SessionCommunity, link: string) => {
    try {
      await copyToClipboard(link);
      setCopiedId(community.id);
      window.setTimeout(() => {
        setCopiedId((current) => (current === community.id ? null : current));
      }, 1600);
    } catch {
      setActionError(community.id, t("communities.copyError"));
    }
  };

  const handleLinkGroup = async (community: SessionCommunity) => {
    const groupId = groupChoice[community.id];
    if (!groupId || busyId) return;
    setBusyId(community.id);
    setActionError(community.id, null);
    try {
      if (isTauri()) {
        await invokeCore<void>("communities_link_group", {
          communityId: community.id,
          groupId,
        });
        await loadInfo(community.id);
      } else {
        const option = groups.find((item) => item.id === groupId);
        setInfos((current) => {
          const info = current[community.id];
          const linkedGroups = [
            ...(info?.linkedGroups ?? []),
            {
              id: groupId,
              name: option?.name ?? groupId,
              participantCount: null,
              isDefaultSubGroup: false,
              isGeneralChat: false,
            },
          ];
          return {
            ...current,
            [community.id]: {
              id: community.id,
              name: info?.name ?? community.name,
              description: info?.description ?? community.description,
              participantCount: info?.participantCount ?? 0,
              linkedGroups,
            },
          };
        });
      }
      setGroupChoice((current) => {
        const next = { ...current };
        delete next[community.id];
        return next;
      });
      setNotice(t("communities.groupLinked"));
    } catch (error) {
      setActionError(
        community.id,
        friendlyError(error, t("communities.linkGroupError")),
      );
    } finally {
      setBusyId(null);
    }
  };

  const handleUnlinkGroup = async (
    community: SessionCommunity,
    group: CommunityLinkedGroup,
  ) => {
    if (busyId) return;
    setBusyId(community.id);
    setActionError(community.id, null);
    try {
      if (isTauri()) {
        await invokeCore<void>("communities_unlink_group", {
          communityId: community.id,
          groupId: group.id,
        });
        await loadInfo(community.id);
      } else {
        setInfos((current) => {
          const info = current[community.id];
          if (!info) return current;
          return {
            ...current,
            [community.id]: {
              ...info,
              linkedGroups: info.linkedGroups.filter(
                (linked) => linked.id !== group.id,
              ),
            },
          };
        });
      }
      setNotice(t("communities.unlinked", { name: group.name || group.id }));
    } catch (error) {
      setActionError(
        community.id,
        friendlyError(error, t("communities.unlinkGroupError")),
      );
    } finally {
      setBusyId(null);
    }
  };

  const handleDeactivate = async () => {
    if (!deactivating || deactivateBusy) return;
    setDeactivateBusy(true);
    setDeactivateError(null);
    try {
      if (isTauri()) {
        await invokeCore<void>("communities_deactivate", {
          communityId: deactivating.id,
        });
      }
      setCommunities((current) =>
        current.filter((entry) => entry.id !== deactivating.id),
      );
      setNotice(t("communities.deactivated", { name: deactivating.name }));
      setDeactivating(null);
    } catch (error) {
      setDeactivateError(
        friendlyError(error, t("communities.deactivateError")),
      );
    } finally {
      setDeactivateBusy(false);
    }
  };

  const linkableGroups = (communityId: Jid): GroupOption[] => {
    const linked = new Set(
      (infos[communityId]?.linkedGroups ?? []).map((group) => group.id),
    );
    return groups.filter(
      (group) => group.id !== communityId && !linked.has(group.id),
    );
  };

  return (
    <section className="chat-list screen">
      <ScreenHeader title={t("communities.title")}>
        <button
          type="button"
          className="icon-button"
          title={t("communities.new")}
          aria-label={t("communities.new")}
          onClick={openCreate}
        >
          <Plus size={24} />
        </button>
      </ScreenHeader>

      <div className="screen-body">
        <p className="community-lede">{t("communities.lede")}</p>

        <button
          type="button"
          className="screen-entry"
          aria-expanded={joinOpen}
          onClick={toggleJoin}
        >
          <span className="screen-entry-icon">
            <Link2 size={22} />
          </span>
          <span className="screen-entry-body">
            <span className="screen-entry-title">
              {t("communities.join")}
            </span>
            <span className="screen-entry-hint">
              {joinOpen
                ? t("communities.joinHintOpen")
                : t("communities.joinHintClosed")}
            </span>
          </span>
        </button>

        {joinOpen ? (
          <form
            className="community-join"
            onSubmit={(event) => {
              event.preventDefault();
              void handleJoin();
            }}
          >
            <input
              className="community-join-input"
              type="text"
              inputMode="url"
              placeholder="https://chat.whatsapp.com/…"
              aria-label={t("communities.inviteAria")}
              value={invite}
              autoFocus
              spellCheck={false}
              autoComplete="off"
              onChange={(event) => {
                setInvite(event.target.value);
                setJoinError(null);
              }}
              onKeyDown={(event) => {
                if (event.key === "Escape") toggleJoin();
              }}
            />
            <button
              type="submit"
              className="modal-action primary"
              disabled={joining || invite.trim().length === 0}
            >
              {joining ? t("communities.joining") : t("communities.joinButton")}
            </button>
            {joinError ? (
              <p className="community-inline-error" role="alert">
                {joinError}
              </p>
            ) : null}
          </form>
        ) : null}

        {notice ? (
          <p className="screen-notice" role="status">
            {notice}
          </p>
        ) : null}

        {tauri ? (
          <p className="community-note" role="note">
            <Info size={14} aria-hidden="true" />
            <span>{t("communities.note")}</span>
          </p>
        ) : null}

        <div className="screen-section-label">{t("communities.yours")}</div>

        {communities.length === 0 ? (
          <EmptyState
            icon={<Users size={26} strokeWidth={1.5} />}
            title={t("communities.emptyTitle")}
            hint={t("communities.emptyHint")}
          />
        ) : (
          communities.map((community) => (
            <CommunityCard
              key={community.id}
              community={community}
              info={infos[community.id] ?? null}
              loading={infoLoading[community.id] ?? false}
              error={infoErrors[community.id] ?? null}
              actionError={actionErrors[community.id] ?? null}
              expanded={expanded === community.id}
              busy={busyId === community.id}
              groups={linkableGroups(community.id)}
              choice={groupChoice[community.id] ?? ""}
              inviteLink={inviteLinks[community.id] ?? null}
              copied={copiedId === community.id}
              onToggle={() => handleToggle(community)}
              onRetry={() => void loadInfo(community.id)}
              onChoice={(groupId) =>
                setGroupChoice((current) => ({
                  ...current,
                  [community.id]: groupId,
                }))
              }
              onLinkGroup={() => void handleLinkGroup(community)}
              onUnlinkGroup={(group) => void handleUnlinkGroup(community, group)}
              onInviteLink={() => void handleInviteLink(community)}
              onCopyLink={() => {
                const link = inviteLinks[community.id];
                if (link) void handleCopyLink(community, link);
              }}
              onDeactivate={() => {
                setDeactivateError(null);
                setDeactivating(community);
              }}
            />
          ))
        )}
      </div>

      {createOpen ? (
        <div className="modal-backdrop" onMouseDown={closeCreate}>
          <div
            className="community-modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("communities.new")}
            onMouseDown={(event) => event.stopPropagation()}
          >
            <header className="modal-header">
              <h2 className="modal-title">{t("communities.new")}</h2>
              <button
                type="button"
                className="icon-button"
                title={t("common.close")}
                aria-label={t("common.close")}
                onClick={closeCreate}
              >
                <X size={22} />
              </button>
            </header>

            <label className="community-modal-field">
              <span className="community-modal-label">
                {t("communities.name")}
              </span>
              <input
                className="community-modal-input"
                type="text"
                value={name}
                maxLength={NAME_MAX_LENGTH}
                autoFocus
                placeholder={t("communities.namePlaceholder")}
                onChange={(event) => {
                  setName(event.target.value);
                  setCreateError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape") closeCreate();
                }}
              />
            </label>

            <label className="community-modal-field">
              <span className="community-modal-label">
                {t("communities.description")}{" "}
                <span className="community-modal-optional">
                  {t("communities.optional")}
                </span>
              </span>
              <textarea
                className="community-modal-textarea"
                value={description}
                maxLength={DESCRIPTION_MAX_LENGTH}
                rows={3}
                placeholder={t("communities.descriptionPlaceholder")}
                onChange={(event) => {
                  setDescription(event.target.value);
                  setCreateError(null);
                }}
              />
            </label>

            {createError ? (
              <p className="community-modal-error" role="alert">
                {createError}
              </p>
            ) : null}

            <footer className="community-modal-actions">
              <button
                type="button"
                className="modal-action secondary"
                disabled={creating}
                onClick={closeCreate}
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                className="modal-action primary"
                disabled={creating || name.trim().length === 0}
                onClick={() => void handleCreate()}
              >
                {creating ? t("common.creating") : t("communities.create")}
              </button>
            </footer>
          </div>
        </div>
      ) : null}

      <ConfirmDialog
        open={deactivating !== null}
        title={t("communities.deactivateTitle")}
        body={
          deactivating ? (
            <>
              {t("communities.deactivateBodyPre")}{" "}
              <strong>{deactivating.name}</strong>{" "}
              {t("communities.deactivateBodyPost")}
            </>
          ) : (
            ""
          )
        }
        confirmLabel={t("communities.deactivateConfirm")}
        danger
        busy={deactivateBusy}
        error={deactivateError}
        onConfirm={() => void handleDeactivate()}
        onCancel={() => {
          if (deactivateBusy) return;
          setDeactivating(null);
          setDeactivateError(null);
        }}
      />
    </section>
  );
}

interface CommunityCardProps {
  community: SessionCommunity;
  info: CommunityInfo | null;
  loading: boolean;
  error: string | null;
  actionError: string | null;
  expanded: boolean;
  busy: boolean;
  groups: GroupOption[];
  choice: string;
  inviteLink: string | null;
  copied: boolean;
  onToggle: () => void;
  onRetry: () => void;
  onChoice: (groupId: string) => void;
  onLinkGroup: () => void;
  onUnlinkGroup: (group: CommunityLinkedGroup) => void;
  onInviteLink: () => void;
  onCopyLink: () => void;
  onDeactivate: () => void;
}

function CommunityCard({
  community,
  info,
  loading,
  error,
  actionError,
  expanded,
  busy,
  groups,
  choice,
  inviteLink,
  copied,
  onToggle,
  onRetry,
  onChoice,
  onLinkGroup,
  onUnlinkGroup,
  onInviteLink,
  onCopyLink,
  onDeactivate,
}: CommunityCardProps) {
  const { t } = useTranslation();
  const description = info?.description ?? community.description;
  const linkedGroups = info?.linkedGroups ?? [];
  const membersLabel = info
    ? info.participantCount === 1
      ? t("communities.member", { count: info.participantCount })
      : t("communities.members", { count: info.participantCount })
    : null;
  const groupsLabel = info
    ? linkedGroups.length === 1
      ? t("communities.linkedGroup", { count: linkedGroups.length })
      : t("communities.linkedGroups", { count: linkedGroups.length })
    : null;

  return (
    <div className="community-card">
      <button
        type="button"
        className="community-card-header"
        aria-expanded={expanded}
        onClick={onToggle}
      >
        <span className="avatar community-avatar">
          {initials(community.name)}
        </span>
        <span className="community-card-body">
          <span className="community-name">{community.name}</span>
          <span className="community-meta">
            {info && membersLabel && groupsLabel
              ? t("communities.meta", {
                  members: membersLabel,
                  groups: groupsLabel,
                })
              : loading
                ? t("common.loading")
                : t("communities.openForDetails")}
          </span>
        </span>
        {expanded ? (
          <ChevronDown size={18} className="community-chevron" />
        ) : (
          <ChevronRight size={18} className="community-chevron" />
        )}
      </button>

      {expanded ? (
        <div className="community-details">
          {description ? (
            <p className="community-description">{description}</p>
          ) : null}

          <div className="community-actions">
            <button
              type="button"
              className="modal-action secondary"
              disabled={busy}
              onClick={onInviteLink}
            >
              {busy ? t("common.working") : t("communities.inviteLink")}
            </button>
            <button
              type="button"
              className="modal-action danger"
              disabled={busy}
              onClick={onDeactivate}
            >
              {t("communities.deactivate")}
            </button>
          </div>

          {inviteLink ? (
            <div className="community-invite-row">
              <span className="community-invite-link" title={inviteLink}>
                {inviteLink}
              </span>
              <button
                type="button"
                className="icon-button"
                title={t("communities.copyInviteLink")}
                aria-label={t("communities.copyInviteLink")}
                onClick={onCopyLink}
              >
                <Copy size={16} />
              </button>
              {copied ? (
                <span className="community-copied" role="status">
                  {t("common.copied")}
                </span>
              ) : null}
            </div>
          ) : null}

          {loading && !info ? (
            <p className="community-detail-hint">
              {t("communities.loadingLinkedGroups")}
            </p>
          ) : error ? (
            <div className="community-detail-error" role="alert">
              <p>{error}</p>
              <button
                type="button"
                className="modal-action secondary"
                disabled={busy}
                onClick={onRetry}
              >
                {t("common.retry")}
              </button>
            </div>
          ) : (
            <div className="community-group-list">
              {linkedGroups.length === 0 ? (
                <p className="community-detail-hint">
                  {t("communities.noGroups")}
                </p>
              ) : (
                linkedGroups.map((group) => (
                  <div className="community-group" key={group.id}>
                    <span className="avatar community-group-avatar">
                      {initials(group.name || group.id)}
                    </span>
                    <span className="community-group-name">
                      {group.name || group.id}
                      {group.isDefaultSubGroup ? (
                        <span className="community-group-tag">
                          {t("communities.tagAnnouncements")}
                        </span>
                      ) : group.isGeneralChat ? (
                        <span className="community-group-tag">
                          {t("communities.tagGeneral")}
                        </span>
                      ) : null}
                    </span>
                    <button
                      type="button"
                      className="icon-button community-group-unlink"
                      title={t("communities.unlink", {
                        name: group.name || group.id,
                      })}
                      aria-label={t("communities.unlink", {
                        name: group.name || group.id,
                      })}
                      disabled={busy}
                      onClick={() => onUnlinkGroup(group)}
                    >
                      <Unlink size={16} />
                    </button>
                  </div>
                ))
              )}
            </div>
          )}

          <div className="community-link-row">
            <select
              className="community-link-select"
              value={choice}
              aria-label={t("communities.linkToAria", {
                name: community.name,
              })}
              disabled={busy}
              onChange={(event) => onChoice(event.target.value)}
            >
              <option value="">{t("communities.linkPlaceholder")}</option>
              {groups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              className="modal-action primary"
              disabled={busy || choice.length === 0}
              onClick={onLinkGroup}
            >
              {t("communities.link")}
            </button>
          </div>

          {actionError ? (
            <p className="community-action-error" role="alert">
              {actionError}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
