import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { avatarSrc } from "../lib/avatar";
import { initials } from "../lib/names";
import { formatDateDivider } from "../lib/time";
import type { ChatSummary, Jid, Message } from "../lib/types";
import { useAppStore, type MessageQuote } from "../store/app";
import { AttachmentMenu, type AttachmentKind } from "./AttachmentMenu";
import { CallOverlay } from "./calls/CallOverlay";
import { DRAFT_SAVE_DELAY_MS, readDraft, writeDraft } from "./composer/drafts";
import { MentionMenu } from "./composer/MentionMenu";
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

interface ConversationProps {
  chat: ChatSummary;
}

export function Conversation({ chat }: ConversationProps) {
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
      <ConversationHeader chat={chat} />
      <div className="messages" ref={scrollRef}>
        {renderMessages(messages, chat, startEditing)}
      </div>
      <Composer
        chatId={chat.id}
        participants={participants}
        onSend={(text, mentions) => sendText(chat.id, text, mentions)}
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
  return (
    <section className="conversation" style={{ position: "relative" }}>
      <div className="empty-conversation">
        <div className="empty-logo">
          <MessageCircle size={44} strokeWidth={1.2} />
        </div>
        <h2>RustWA</h2>
        <p>
          The unofficial WhatsApp client for macOS, built from scratch in Rust.
          Pair this device to start messaging.
        </p>
      </div>
      <div className="empty-footer">
        <span>🔒</span>
        <span>End-to-end encrypted</span>
      </div>
      <CallOverlay />
    </section>
  );
}

function ConversationHeader({ chat }: { chat: ChatSummary }) {
  const typing = useAppStore(
    (state) => state.typingByChat[chat.id] ?? false,
  );
  const startCall = useAppStore((state) => state.startCall);

  return (
    <header className="conversation-header" data-tauri-drag-region>
      <ConversationAvatar chat={chat} />
      <div className="conversation-title" data-tauri-drag-region>
        <span className="conversation-name">{chat.name}</span>
        <span className="conversation-subtitle">
          {typing ? "typing…" : chat.isGroup ? "Group" : "online"}
        </span>
      </div>
      <div className="header-actions no-drag">
        <button type="button" className="icon-button" title="Search">
          <Search size={22} />
        </button>
        <button
          type="button"
          className="icon-button"
          title="Voice call"
          onClick={() => startCall(chat.id, false)}
        >
          <Phone size={22} />
        </button>
        <button
          type="button"
          className="icon-button"
          title="Video call"
          onClick={() => startCall(chat.id, true)}
        >
          <Video size={22} />
        </button>
        <button type="button" className="icon-button" title="Menu">
          <EllipsisVertical size={22} />
        </button>
      </div>
    </header>
  );
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
  participants,
  onSend,
  onTyping,
  replyTo,
  onCancelReply,
  editing,
  onCancelEdit,
  onSaveEdit,
}: ComposerProps) {
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
  const handleAttach = (_kind: AttachmentKind) => {
    setAttachOpen(false);
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
      {attachOpen && (
        <AttachmentMenu
          onPick={handleAttach}
          onClose={() => setAttachOpen(false)}
        />
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
            <span className="composer-context-title">Editing message</span>
            <span className="composer-context-text">{editing.text}</span>
          </div>
          <button
            type="button"
            className="icon-button"
            title="Cancel edit"
            onClick={cancelEdit}
          >
            <X size={20} />
          </button>
        </div>
      ) : replyTo ? (
        <div className="composer-context">
          <div className="composer-context-body">
            <span className="composer-context-title">
              {replyTo.senderName ?? "Reply"}
            </span>
            <span className="composer-context-text">{replyTo.preview}</span>
          </div>
          <button
            type="button"
            className="icon-button"
            title="Cancel reply"
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
          title="Emoji"
          aria-expanded={emojiOpen}
          onClick={toggleEmoji}
        >
          <Smile size={24} />
        </button>
        <button
          type="button"
          className="icon-button"
          title="Attach"
          aria-expanded={attachOpen}
          onClick={toggleAttach}
        >
          <Paperclip size={24} />
        </button>

        <textarea
          ref={textareaRef}
          className="composer-input"
          rows={1}
          placeholder={editing ? "Edit message" : "Type a message"}
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
            title="Save edit"
            disabled={!hasText}
            onClick={submit}
          >
            <Check size={22} />
          </button>
        ) : hasText ? (
          <button
            type="button"
            className="icon-button"
            title="Send"
            onClick={submit}
          >
            <Send size={24} />
          </button>
        ) : (
          <button type="button" className="icon-button" title="Voice message">
            <Mic size={24} />
          </button>
        )}
      </div>
    </footer>
  );
}
