import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Archive, Bell, BellOff, CheckCheck, Info, Trash2 } from "lucide-react";
import { avatarSrc } from "../lib/avatar";
import { t, useTranslation } from "../lib/i18n";
import { invokeCore, isTauri } from "../lib/ipc";
import { initials } from "../lib/names";
import { formatBubbleTime, formatDateDivider } from "../lib/time";
import type { ChatSummary, Jid, Message } from "../lib/types";
import { useAppStore, type MessageQuote } from "../store/app";
import { AttachmentMenu, type AttachmentKind } from "./AttachmentMenu";
import { PollComposer } from "./message/PollComposer";
import { CallOverlay } from "./calls/CallOverlay";
import { ContextMenu, type ContextMenuEntry } from "./ContextMenu";
import { DRAFT_SAVE_DELAY_MS, readDraft, writeDraft } from "./composer/drafts";
import { MentionMenu } from "./composer/MentionMenu";
import { GroupInfoPanel } from "./groups/GroupInfoPanel";
import { ConfirmDialog } from "./settings/ConfirmDialog";
import {
  deriveGroupParticipants,
  findMentionTrigger,
  matchParticipants,
  mentionJids,
  pruneMentions,
  type MentionParticipant,
  type MentionRef,
  type MentionTrigger,
} from "./composer/mentions";
import { EmojiPicker } from "./EmojiPicker";
import { MessageBubble } from "./message/MessageBubble";
import {
  Check,
  EllipsisVertical,
  MessageCircle,
  Mic,
  Paperclip,
  Phone,
  Search,
  Send,
  Smile,
  Video,
  X,
} from "./icons";

const EMPTY_MESSAGES: Message[] = [];
const EMPTY_PARTICIPANTS: MentionParticipant[] = [];

/** Channels are `@newsletter` JIDs; only they use the channel post path. */
function isChannelChat(chatId: Jid): boolean {
  return chatId.endsWith("@newsletter");
}

/**
 * Post to a channel through `channels_send`, keeping the store's optimistic
 * echo behavior: the bubble appears as "pending" immediately and is reconciled
 * with the stored message once the core acks. The store's `sendText` action
 * only knows `send_text`, hence this dedicated path.
 */
function sendChannelText(chatId: Jid, text: string): void {
  const trimmed = text.trim();
  if (!trimmed) return;

  const optimistic: Message = {
    id: `local-${crypto.randomUUID()}`,
    chatId,
    senderId: "me",
    fromMe: true,
    timestamp: Math.floor(Date.now() / 1000),
    kind: "text",
    text: trimmed,
    status: "pending",
  };
  useAppStore.getState().appendMessage(optimistic);

  if (!isTauri()) return;

  void invokeCore<Message>("channels_send", { chatId, text: trimmed })
    .then((stored) => {
      // Swap the optimistic row for the stored one. The core also emits the
      // stored message on the event bus, which may land before or after this
      // ack; dropping both ids first keeps exactly one copy.
      useAppStore.setState((state) => {
        const current = state.messages[chatId] ?? [];
        const rest = current.filter(
          (message) =>
            message.id !== optimistic.id && message.id !== stored.id,
        );
        return {
          messages: { ...state.messages, [chatId]: [...rest, stored] },
        };
      });
    })
    .catch((error: unknown) => {
      console.error("channel send failed", error);
      useAppStore.getState().setMessageStatus(chatId, optimistic.id, "failed");
    });
}

interface ConversationProps {
  chat: ChatSummary;
  /** Optional host hook for a contact-info panel; the header menu enables its
   * "Contact info" item only when this is provided (direct chats). */
  onOpenContactInfo?: (chat: ChatSummary) => void;
}

export function Conversation({ chat, onOpenContactInfo }: ConversationProps) {
  const messages = useAppStore(
    (state) => state.messages[chat.id] ?? EMPTY_MESSAGES,
  );
  const contactNames = useAppStore((state) => state.contactNames);
  const sendText = useAppStore((state) => state.sendText);
  const loadMessages = useAppStore((state) => state.loadMessages);
  const sendTyping = useAppStore((state) => state.sendTyping);
  const markRead = useAppStore((state) => state.markRead);
  const replyTo = useAppStore((state) => state.replyTo);
  const setReplyTo = useAppStore((state) => state.setReplyTo);
  const editMessage = useAppStore((state) => state.editMessage);
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastTypingSentAt = useRef(0);
  const typingActive = useRef(false);
  const isChannel = isChannelChat(chat.id);

  // Mention targets for group chats: senders seen in the loaded history,
  // labelled with the names the core's chat list provides.
  const participants = useMemo(
    () =>
      chat.isGroup
        ? deriveGroupParticipants(messages, contactNames)
        : EMPTY_PARTICIPANTS,
    [chat.isGroup, messages, contactNames],
  );

  // Inline edit target for the composer (own text messages only).
  const [editing, setEditing] = useState<{
    messageId: string;
    text: string;
  } | null>(null);

  const activeReply = replyTo && replyTo.chatId === chat.id ? replyTo : null;

  useEffect(() => {
    loadMessages(chat.id);
  }, [chat.id, loadMessages]);

  // Switching chats abandons any inline edit.
  useEffect(() => {
    setEditing(null);
  }, [chat.id]);

  // Opening a visible conversation marks it as read.
  useEffect(() => {
    if (chat.unreadCount > 0 && document.visibilityState === "visible") {
      markRead(chat.id);
    }
  }, [chat.id, chat.unreadCount, markRead]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages.length, chat.id]);

  // Throttled outgoing typing state: signal at most every 7 s while typing and
  // signal "paused" as soon as the composer empties or the message goes out.
  const handleTyping = (hasText: boolean) => {
    // Channels do not carry typing indicators.
    if (isChannel) return;
    const now = Date.now();
    if (hasText) {
      if (!typingActive.current || now - lastTypingSentAt.current > 7000) {
        typingActive.current = true;
        lastTypingSentAt.current = now;
        sendTyping(chat.id, true);
      }
    } else if (typingActive.current) {
      typingActive.current = false;
      sendTyping(chat.id, false);
    }
  };

  const startEditing = (message: Message) =>
    setEditing({ messageId: message.id, text: message.text ?? "" });

  const saveEdit = (text: string) => {
    if (!editing) return;
    editMessage(chat.id, editing.messageId, text);
    setEditing(null);
  };

  return (
    <section className="conversation">
      <ConversationHeader
        chat={chat}
        onOpenContactInfo={onOpenContactInfo}
      />
      <div className="messages" ref={scrollRef}>
        {renderMessages(messages, chat, startEditing)}
      </div>
      <Composer
        chatId={chat.id}
        isChannel={isChannel}
        participants={participants}
        onSend={(text, mentions) => {
          if (isChannel) {
            // `channels_send` only carries text: drop the reply target the
            // way `sendText` would, so the quote banner cannot linger.
            setReplyTo(null);
            sendChannelText(chat.id, text);
            return;
          }
          sendText(chat.id, text, mentions);
        }}
        onTyping={handleTyping}
        replyTo={activeReply}
        onCancelReply={() => setReplyTo(null)}
        editing={editing}
        onCancelEdit={() => setEditing(null)}
        onSaveEdit={saveEdit}
      />
      <CallOverlay />
    </section>
  );
}

export function EmptyConversation() {
  const { t } = useTranslation();

  return (
    <section className="conversation" style={{ position: "relative" }}>
      <div className="empty-conversation">
        <div className="empty-logo">
          <MessageCircle size={44} strokeWidth={1.2} />
        </div>
        <h2>RustWA</h2>
        <p>{t("conversation.emptyLede")}</p>
      </div>
      <div className="empty-footer">
        <span>🔒</span>
        <span>{t("common.e2eEncrypted")}</span>
      </div>
      <CallOverlay />
    </section>
  );
}

function ConversationHeader({
  chat,
  onOpenContactInfo,
}: {
  chat: ChatSummary;
  onOpenContactInfo?: (chat: ChatSummary) => void;
}) {
  const { t } = useTranslation();
  const typing = useAppStore(
    (state) => state.typingByChat[chat.id] ?? false,
  );
  const startCall = useAppStore((state) => state.startCall);
  const presence = useAppStore((state) => state.presenceByChat[chat.id]);
  const toggleMuted = useAppStore((state) => state.toggleMuted);
  const archiveChat = useAppStore((state) => state.archiveChat);
  const unarchiveChat = useAppStore((state) => state.unarchiveChat);
  const markRead = useAppStore((state) => state.markRead);
  const clearChat = useAppStore((state) => state.clearChat);

  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [infoOpen, setInfoOpen] = useState(false);
  const [clearOpen, setClearOpen] = useState(false);
  const [clearBusy, setClearBusy] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);

  // Switching chats closes any overlay that belongs to the previous one.
  useEffect(() => {
    setMenu(null);
    setInfoOpen(false);
    setClearOpen(false);
    setClearBusy(false);
    setClearError(null);
  }, [chat.id]);

  const menuItems: ContextMenuEntry[] = [
    chat.isGroup
      ? {
          id: "group-info",
          label: t("groups.info"),
          icon: Info,
          onSelect: () => setInfoOpen(true),
        }
      : onOpenContactInfo
        ? {
            id: "contact-info",
            label: t("conversation.menu.contactInfo"),
            icon: Info,
            onSelect: () => onOpenContactInfo(chat),
          }
        : {
            id: "contact-info",
            label: t("conversation.menu.contactInfoUnavailable"),
            icon: Info,
            disabled: true,
            onSelect: () => {},
          },
    {
      id: "mute",
      label: chat.muted ? t("chats.menu.unmute") : t("chats.menu.mute"),
      icon: chat.muted ? Bell : BellOff,
      onSelect: () => toggleMuted(chat.id),
    },
    {
      id: "archive",
      label: chat.isArchived
        ? t("chats.menu.unarchive")
        : t("chats.menu.archive"),
      icon: Archive,
      onSelect: () =>
        chat.isArchived ? unarchiveChat(chat.id) : archiveChat(chat.id),
    },
    {
      id: "read",
      label: t("chats.menu.markRead"),
      icon: CheckCheck,
      disabled: chat.unreadCount === 0,
      onSelect: () => markRead(chat.id),
    },
    { kind: "separator", id: "menu-separator" },
    {
      id: "clear",
      label: t("conversation.menu.clearChat"),
      icon: Trash2,
      danger: true,
      onSelect: () => {
        setClearError(null);
        setClearOpen(true);
      },
    },
  ];

  const confirmClear = () => {
    setClearBusy(true);
    setClearError(null);
    void clearChat(chat.id)
      .then(() => setClearOpen(false))
      .catch((cause: unknown) => setClearError(clearErrorMessage(cause)))
      .finally(() => setClearBusy(false));
  };

  const subtitle = typing
    ? t("conversation.typing")
    : isChannelChat(chat.id)
      ? t("conversation.channel")
      : chat.isGroup
        ? t("conversation.group")
        : presence?.online
          ? t("conversation.online")
          : presence?.lastSeenTs
            ? t("conversation.lastSeen", {
                time: formatBubbleTime(presence.lastSeenTs),
              })
            : null;

  return (
    <>
      <header className="conversation-header" data-tauri-drag-region>
        <ConversationAvatar chat={chat} />
        <div className="conversation-title" data-tauri-drag-region>
          <span className="conversation-name">{chat.name}</span>
          {subtitle && <span className="conversation-subtitle">{subtitle}</span>}
        </div>
        <div className="header-actions no-drag">
          <button
            type="button"
            className="icon-button"
            title={t("common.search")}
          >
            <Search size={22} />
          </button>
          <button
            type="button"
            className="icon-button"
            title={t("conversation.voiceCall")}
            onClick={() => startCall(chat.id, false)}
          >
            <Phone size={22} />
          </button>
          <button
            type="button"
            className="icon-button"
            title={t("conversation.videoCall")}
            onClick={() => startCall(chat.id, true)}
          >
            <Video size={22} />
          </button>
          <button
            type="button"
            className="icon-button"
            title={t("common.menu")}
            aria-haspopup="menu"
            aria-expanded={menu !== null}
            onClick={(event) =>
              setMenu({ x: event.clientX, y: event.clientY })
            }
          >
            <EllipsisVertical size={22} />
          </button>
        </div>
      </header>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={() => setMenu(null)}
        />
      )}

      {infoOpen && chat.isGroup ? (
        <div className="conversation-info-drawer">
          <GroupInfoPanel
            chatId={chat.id}
            onClose={() => setInfoOpen(false)}
          />
        </div>
      ) : null}

      <ConfirmDialog
        open={clearOpen}
        title={t("conversation.clearTitle")}
        body={t("conversation.clearBody")}
        confirmLabel={t("conversation.menu.clearChat")}
        danger
        busy={clearBusy}
        error={clearError}
        onCancel={() => {
          if (clearBusy) return;
          setClearOpen(false);
          setClearError(null);
        }}
        onConfirm={confirmClear}
      />
    </>
  );
}

/** Human copy for a failed `chat_clear` (`clearChat` rejects with raw IPC errors). */
function clearErrorMessage(cause: unknown): string {
  const raw =
    cause instanceof Error
      ? cause.message
      : typeof cause === "string"
        ? cause
        : "";
  if (/not connected|not linked|not paired|disconnected/i.test(raw)) {
    return t("conversation.clearNeedsConnection");
  }
  return raw || t("conversation.clearError");
}

function ConversationAvatar({ chat }: { chat: ChatSummary }) {
  const avatar = useAppStore((state) => state.avatars[chat.id]);
  const loadAvatar = useAppStore((state) => state.loadAvatar);

  useEffect(() => {
    loadAvatar(chat.id);
  }, [chat.id, loadAvatar]);

  return (
    <div className="avatar small">
      {avatar ? <img src={avatarSrc(avatar)} alt="" /> : initials(chat.name)}
    </div>
  );
}

function renderMessages(
  messages: Message[],
  chat: ChatSummary,
  onEdit: (message: Message) => void,
): ReactNode[] {
  const nodes: ReactNode[] = [];
  let previousDay = "";

  messages.forEach((message, index) => {
    const day = new Date(message.timestamp * 1000).toDateString();
    if (day !== previousDay) {
      previousDay = day;
      nodes.push(
        <div className="date-divider" key={`day-${day}`}>
          {formatDateDivider(message.timestamp)}
        </div>,
      );
    }

    const next = messages[index + 1];
    const isLastOfGroup =
      !next ||
      next.senderId !== message.senderId ||
      new Date(next.timestamp * 1000).toDateString() !== day;

    nodes.push(
      <MessageBubble
        key={message.id}
        message={message}
        chat={chat}
        tail={isLastOfGroup}
        onEdit={onEdit}
      />,
    );
  });

  return nodes;
}

interface ComposerProps {
  chatId: Jid;
  /** True for `@newsletter` chats, which post text only. */
  isChannel: boolean;
  /** Group participants the mention menu offers; empty for direct chats. */
  participants: MentionParticipant[];
  onSend: (text: string, mentions: Jid[]) => void;
  onTyping: (hasText: boolean) => void;
  replyTo: MessageQuote | null;
  onCancelReply: () => void;
  editing: { messageId: string; text: string } | null;
  onCancelEdit: () => void;
  onSaveEdit: (text: string) => void;
}

function Composer({
  chatId,
  isChannel,
  participants,
  onSend,
  onTyping,
  replyTo,
  onCancelReply,
  editing,
  onCancelEdit,
  onSaveEdit,
}: ComposerProps) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  // Mentions inserted via the autocomplete, tracked as spans in `text`.
  const [mentions, setMentions] = useState<MentionRef[]>([]);
  // The in-progress `@query` before the caret, if the menu is open.
  const [mentionTrigger, setMentionTrigger] = useState<MentionTrigger | null>(
    null,
  );
  const [mentionIndex, setMentionIndex] = useState(0);
  const [emojiOpen, setEmojiOpen] = useState(false);
  const [attachOpen, setAttachOpen] = useState(false);
  const [pollOpen, setPollOpen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const composerRef = useRef<HTMLElement>(null);
  const draftTimer = useRef<number | null>(null);
  const pendingDraft = useRef<{ chatId: Jid; text: string } | null>(null);

  const resize = () => {
    const element = textareaRef.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${Math.min(element.scrollHeight, 120)}px`;
  };

  /** Write any debounced draft immediately (chat switch / unmount). */
  const flushDraft = () => {
    if (draftTimer.current !== null) {
      window.clearTimeout(draftTimer.current);
      draftTimer.current = null;
    }
    const pending = pendingDraft.current;
    pendingDraft.current = null;
    if (pending) writeDraft(pending.chatId, pending.text);
  };

  const scheduleDraft = (value: string) => {
    if (editing) return;
    pendingDraft.current = { chatId, text: value };
    if (draftTimer.current !== null) window.clearTimeout(draftTimer.current);
    draftTimer.current = window.setTimeout(() => {
      draftTimer.current = null;
      const pending = pendingDraft.current;
      pendingDraft.current = null;
      if (pending) writeDraft(pending.chatId, pending.text);
    }, DRAFT_SAVE_DELAY_MS);
  };

  // Switching chats persists the previous chat's draft and restores this one
  // (also on first mount, so a reload brings the draft back).
  useEffect(() => {
    flushDraft();
    setText(readDraft(chatId));
    setMentions([]);
    setMentionTrigger(null);
    setMentionIndex(0);
    requestAnimationFrame(resize);
  }, [chatId]);

  // Best-effort flush when the conversation unmounts mid-debounce.
  useEffect(() => () => flushDraft(), []);

  // Clicking outside the composer dismisses whichever panel is open.
  const mentionOpen =
    !editing && mentionTrigger !== null && participants.length > 0;
  useEffect(() => {
    if (!emojiOpen && !attachOpen && !mentionOpen) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!composerRef.current?.contains(event.target as Node)) {
        setEmojiOpen(false);
        setAttachOpen(false);
        setMentionTrigger(null);
      }
    };
    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [emojiOpen, attachOpen, mentionOpen]);

  // Entering edit mode loads the message into the composer.
  useEffect(() => {
    if (!editing) return;
    setEmojiOpen(false);
    setAttachOpen(false);
    setMentions([]);
    setMentionTrigger(null);
    setText(editing.text);
    requestAnimationFrame(() => {
      const element = textareaRef.current;
      element?.focus();
      element?.setSelectionRange(element.value.length, element.value.length);
      resize();
    });
  }, [editing]);

  // Picking Reply moves focus straight to the input.
  useEffect(() => {
    if (replyTo) textareaRef.current?.focus();
  }, [replyTo]);

  const resetPanels = () => {
    setEmojiOpen(false);
    setAttachOpen(false);
    requestAnimationFrame(resize);
  };

  const submit = () => {
    const value = text.trim();
    if (!value) return;
    if (editing) {
      onSaveEdit(value);
    } else {
      // Validate spans against the raw text, then send the trimmed message.
      onSend(value, mentionJids(text, mentions));
    }
    flushDraft();
    writeDraft(chatId, "");
    setText("");
    setMentions([]);
    setMentionTrigger(null);
    setMentionIndex(0);
    onTyping(false);
    resetPanels();
  };

  const cancelEdit = () => {
    onCancelEdit();
    // Flush any draft typed before the edit started, then bring it back.
    flushDraft();
    setMentions([]);
    setMentionTrigger(null);
    setText(readDraft(chatId));
    onTyping(false);
    resetPanels();
  };

  const cancelReply = () => {
    onCancelReply();
    textareaRef.current?.focus();
  };

  const insertEmoji = (emoji: string) => {
    const element = textareaRef.current;
    const start = element?.selectionStart ?? text.length;
    const end = element?.selectionEnd ?? text.length;
    const next = `${text.slice(0, start)}${emoji}${text.slice(end)}`;
    setText(next);
    setMentions((current) => pruneMentions(next, current));
    setMentionTrigger(null);
    scheduleDraft(next);
    onTyping(true);
    requestAnimationFrame(() => {
      const cursor = start + emoji.length;
      element?.focus();
      element?.setSelectionRange(cursor, cursor);
      resize();
    });
  };

  const toggleEmoji = () => {
    setEmojiOpen((open) => !open);
    setAttachOpen(false);
    setMentionTrigger(null);
  };

  const toggleAttach = () => {
    setAttachOpen((open) => !open);
    setEmojiOpen(false);
    setMentionTrigger(null);
  };

  // The actual attachment flows land with a later milestone; for now the
  // menu only reports the chosen kind and dismisses itself.
  const handleAttach = (kind: AttachmentKind) => {
    setAttachOpen(false);
    if (kind === "poll") {
      setPollOpen(true);
    }
  };

  /** Recompute the active `@query` from the text and caret position. */
  const updateMentionTrigger = (value: string, caret: number) => {
    const next =
      editing || participants.length === 0
        ? null
        : findMentionTrigger(value, caret);
    setMentionTrigger((current) =>
      current?.start === next?.start && current?.query === next?.query
        ? current
        : next,
    );
  };

  /** Replace the active `@query` with `@Name ` and remember its span. */
  const insertMention = (participant: MentionParticipant) => {
    const element = textareaRef.current;
    if (!mentionTrigger || !element) return;

    const caret = element.selectionStart ?? text.length;
    const selectionEnd = element.selectionEnd ?? caret;
    const inserted = `@${participant.name} `;
    const next =
      text.slice(0, mentionTrigger.start) + inserted + text.slice(selectionEnd);
    const mention: MentionRef = {
      jid: participant.id,
      name: participant.name,
      start: mentionTrigger.start,
      end: mentionTrigger.start + inserted.length,
    };

    setText(next);
    setMentions((current) => [...pruneMentions(next, current), mention]);
    setMentionTrigger(null);
    setMentionIndex(0);
    scheduleDraft(next);

    const cursor = mentionTrigger.start + inserted.length;
    requestAnimationFrame(() => {
      element.focus();
      element.setSelectionRange(cursor, cursor);
      resize();
    });
  };

  const mentionMatches = mentionTrigger
    ? matchParticipants(participants, mentionTrigger.query)
    : EMPTY_PARTICIPANTS;
  const safeMentionIndex =
    mentionMatches.length === 0
      ? 0
      : Math.min(mentionIndex, mentionMatches.length - 1);

  const hasText = text.trim().length > 0;

  return (
    <footer className="composer" ref={composerRef}>
      {emojiOpen && (
        <EmojiPicker
          onPick={insertEmoji}
          onClose={() => setEmojiOpen(false)}
        />
      )}
      {!isChannel && attachOpen && (
        <AttachmentMenu
          onPick={handleAttach}
          onClose={() => setAttachOpen(false)}
        />
      )}
      {pollOpen && (
        <PollComposer chatId={chatId} onClose={() => setPollOpen(false)} />
      )}
      {mentionOpen && (
        <MentionMenu
          participants={mentionMatches}
          activeIndex={safeMentionIndex}
          onHover={setMentionIndex}
          onPick={insertMention}
        />
      )}

      {editing ? (
        <div className="composer-context edit">
          <div className="composer-context-body">
            <span className="composer-context-title">
              {t("conversation.editing")}
            </span>
            <span className="composer-context-text">{editing.text}</span>
          </div>
          <button
            type="button"
            className="icon-button"
            title={t("conversation.cancelEdit")}
            onClick={cancelEdit}
          >
            <X size={20} />
          </button>
        </div>
      ) : replyTo ? (
        <div className="composer-context">
          <div className="composer-context-body">
            <span className="composer-context-title">
              {replyTo.senderName ?? t("common.reply")}
            </span>
            <span className="composer-context-text">{replyTo.preview}</span>
          </div>
          <button
            type="button"
            className="icon-button"
            title={t("conversation.cancelReply")}
            onClick={cancelReply}
          >
            <X size={20} />
          </button>
        </div>
      ) : null}

      <div className="composer-row">
        <button
          type="button"
          className="icon-button"
          title={t("conversation.emoji")}
          aria-expanded={emojiOpen}
          onClick={toggleEmoji}
        >
          <Smile size={24} />
        </button>
        {!isChannel && (
          <button
            type="button"
            className="icon-button"
            title={t("conversation.attach")}
            aria-expanded={attachOpen}
            onClick={toggleAttach}
          >
            <Paperclip size={24} />
          </button>
        )}

        <textarea
          ref={textareaRef}
          className="composer-input"
          rows={1}
          placeholder={
            editing
              ? t("conversation.editPlaceholder")
              : t("conversation.typePlaceholder")
          }
          value={text}
          onChange={(event) => {
            const value = event.target.value;
            setText(value);
            setMentions((current) => pruneMentions(value, current));
            onTyping(value.trim().length > 0);
            scheduleDraft(value);
            setMentionIndex(0);
            updateMentionTrigger(
              value,
              event.target.selectionStart ?? value.length,
            );
            resize();
          }}
          onSelect={(event) => {
            updateMentionTrigger(
              event.currentTarget.value,
              event.currentTarget.selectionStart ?? 0,
            );
          }}
          onKeyDown={(event) => {
            if (mentionOpen) {
              if (event.key === "ArrowDown" && mentionMatches.length > 0) {
                event.preventDefault();
                setMentionIndex(
                  (index) => (index + 1) % mentionMatches.length,
                );
                return;
              }
              if (event.key === "ArrowUp" && mentionMatches.length > 0) {
                event.preventDefault();
                setMentionIndex(
                  (index) =>
                    (index - 1 + mentionMatches.length) %
                    mentionMatches.length,
                );
                return;
              }
              if (
                (event.key === "Enter" || event.key === "Tab") &&
                mentionMatches.length > 0
              ) {
                event.preventDefault();
                const participant = mentionMatches[safeMentionIndex];
                if (participant) insertMention(participant);
                return;
              }
              if (event.key === "Escape") {
                event.preventDefault();
                setMentionTrigger(null);
                return;
              }
            }
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              submit();
              return;
            }
            if (event.key === "Escape" && (editing || replyTo)) {
              event.preventDefault();
              if (editing) cancelEdit();
              else cancelReply();
            }
          }}
        />

        {editing ? (
          <button
            type="button"
            className="icon-button"
            title={t("conversation.saveEdit")}
            disabled={!hasText}
            onClick={submit}
          >
            <Check size={22} />
          </button>
        ) : hasText ? (
          <button
            type="button"
            className="icon-button"
            title={t("conversation.send")}
            onClick={submit}
          >
            <Send size={24} />
          </button>
        ) : (
          <button
            type="button"
            className="icon-button"
            title={t("conversation.voiceMessage")}
          >
            <Mic size={24} />
          </button>
        )}
      </div>
    </footer>
  );
}
