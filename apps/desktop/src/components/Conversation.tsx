import { useEffect, useRef, useState, type ReactNode } from "react";
import { initials } from "../lib/names";
import { formatBubbleTime, formatDateDivider } from "../lib/time";
import type { ChatSummary, Message, MessageStatus } from "../lib/types";
import { useAppStore } from "../store/app";
import {
  Check,
  CheckCheck,
  Clock,
  EllipsisVertical,
  MessageCircle,
  Mic,
  Paperclip,
  Phone,
  Search,
  Send,
  Smile,
  Video,
} from "./icons";

const EMPTY_MESSAGES: Message[] = [];

interface ConversationProps {
  chat: ChatSummary;
}

export function Conversation({ chat }: ConversationProps) {
  const messages = useAppStore(
    (state) => state.messages[chat.id] ?? EMPTY_MESSAGES,
  );
  const sendText = useAppStore((state) => state.sendText);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages.length, chat.id]);

  return (
    <section className="conversation">
      <ConversationHeader chat={chat} />
      <div className="messages" ref={scrollRef}>
        {renderMessages(messages)}
      </div>
      <Composer onSend={(text) => sendText(chat.id, text)} />
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
    </section>
  );
}

function ConversationHeader({ chat }: { chat: ChatSummary }) {
  return (
    <header className="conversation-header" data-tauri-drag-region>
      <div className="avatar small">{initials(chat.name)}</div>
      <div className="conversation-title">
        <span className="conversation-name">{chat.name}</span>
        <span className="conversation-subtitle">
          {chat.isGroup ? "Group" : "online"}
        </span>
      </div>
      <div className="header-actions no-drag">
        <button type="button" className="icon-button" title="Search">
          <Search size={22} />
        </button>
        <button type="button" className="icon-button" title="Voice call">
          <Phone size={22} />
        </button>
        <button type="button" className="icon-button" title="Video call">
          <Video size={22} />
        </button>
        <button type="button" className="icon-button" title="Menu">
          <EllipsisVertical size={22} />
        </button>
      </div>
    </header>
  );
}

function renderMessages(messages: Message[]): ReactNode[] {
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
        tail={isLastOfGroup}
      />,
    );
  });

  return nodes;
}

function MessageBubble({ message, tail }: { message: Message; tail: boolean }) {
  const classes = ["bubble"];
  if (message.fromMe) classes.push("out");
  if (tail) classes.push(message.fromMe ? "tail-out" : "tail-in");

  return (
    <div className={`message-row${message.fromMe ? " out" : ""}`}>
      <div className={classes.join(" ")}>
        {message.text}
        <span className="bubble-meta">
          {formatBubbleTime(message.timestamp)}
          {message.fromMe && <StatusTick status={message.status} />}
        </span>
      </div>
    </div>
  );
}

function StatusTick({ status }: { status: MessageStatus }) {
  switch (status) {
    case "pending":
      return <Clock size={14} />;
    case "sent":
      return <Check size={15} />;
    case "failed":
      return <Clock size={14} style={{ color: "var(--danger)" }} />;
    case "read":
    case "played":
      return <CheckCheck size={15} className="tick-read" />;
    default:
      return <CheckCheck size={15} />;
  }
}

function Composer({ onSend }: { onSend: (text: string) => void }) {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const resize = () => {
    const element = textareaRef.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${Math.min(element.scrollHeight, 120)}px`;
  };

  const submit = () => {
    const value = text.trim();
    if (!value) return;
    onSend(value);
    setText("");
    requestAnimationFrame(resize);
  };

  return (
    <footer className="composer">
      <button type="button" className="icon-button" title="Emoji">
        <Smile size={24} />
      </button>
      <button type="button" className="icon-button" title="Attach">
        <Paperclip size={24} />
      </button>

      <textarea
        ref={textareaRef}
        className="composer-input"
        rows={1}
        placeholder="Type a message"
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          resize();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            submit();
          }
        }}
      />

      {text.trim().length > 0 ? (
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
    </footer>
  );
}
