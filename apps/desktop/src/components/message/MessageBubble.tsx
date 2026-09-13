/** A single message bubble: text or media, quoted context, reactions and the
 * right-click menu. Composed from the per-kind media components in this
 * folder; the date dividers, tails and lazy loading stay in Conversation. */

import { useState, type MouseEvent as ReactMouseEvent } from "react";
import { Copy, Pencil, Reply, SmilePlus, Star, Trash } from "lucide-react";
import { formatBubbleTime } from "../../lib/time";
import type { ChatSummary, Message, MessageStatus } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { ContextMenu, type ContextMenuEntry } from "../ContextMenu";
import { Check, CheckCheck, Clock } from "../icons";
import { FileContent } from "./FileContent";
import { ImageContent } from "./ImageContent";
import { MessagePopover } from "./MessagePopover";
import { QuotePreview } from "./QuotePreview";
import { ReactionBar } from "./ReactionBar";
import { VideoContent } from "./VideoContent";
import { VoiceNoteContent } from "./VoiceNoteContent";
import { messagePreview } from "./media";

interface MessageBubbleProps {
  message: Message;
  chat: ChatSummary;
  /** True when the next message starts a new sender/day group. */
  tail: boolean;
  onEdit: (message: Message) => void;
}

export function MessageBubble({
  message,
  chat,
  tail,
  onEdit,
}: MessageBubbleProps) {
  const deleted = useAppStore((state) =>
    Boolean(state.deletedMessages[message.id]),
  );
  const starred = useAppStore((state) => Boolean(state.starred[message.id]));
  const reactions = useAppStore((state) => state.reactions[message.id]);
  const myReaction = useAppStore(
    (state) => state.myReactions[message.id] ?? null,
  );
  const quote = useAppStore((state) => state.quotes[message.id]);
  const setReplyTo = useAppStore((state) => state.setReplyTo);
  const toggleStar = useAppStore((state) => state.toggleStar);
  const toggleReaction = useAppStore((state) => state.toggleReaction);
  const revokeMessage = useAppStore((state) => state.revokeMessage);

  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [popover, setPopover] = useState<{
    kind: "react" | "delete";
    x: number;
    y: number;
  } | null>(null);

  const isMedia =
    message.kind === "image" ||
    message.kind === "gif" ||
    message.kind === "video" ||
    message.kind === "sticker";

  const classes = ["bubble"];
  if (message.fromMe) classes.push("out");
  if (tail) classes.push(message.fromMe ? "tail-out" : "tail-in");
  if (isMedia) classes.push("media-bubble");
  if (message.kind === "sticker") classes.push("media-plain");
  if (deleted) classes.push("deleted");

  const openMenu = (event: ReactMouseEvent) => {
    event.preventDefault();
    setPopover(null);
    setMenu({ x: event.clientX, y: event.clientY });
  };

  const copyText = () => {
    const text = message.text;
    if (!text) return;
    if (navigator.clipboard?.writeText) {
      void navigator.clipboard
        .writeText(text)
        .catch(() => fallbackCopy(text));
    } else {
      fallbackCopy(text);
    }
  };

  const menuItems: ContextMenuEntry[] = [];
  if (!deleted) {
    menuItems.push({
      id: "reply",
      label: "Reply",
      icon: Reply,
      onSelect: () =>
        setReplyTo({
          chatId: message.chatId,
          messageId: message.id,
          preview: messagePreview(message),
          senderName: message.fromMe ? "You" : chat.name,
        }),
    });
    menuItems.push({
      id: "react",
      label: "React",
      icon: SmilePlus,
      onSelect: () =>
        setPopover({ kind: "react", x: menu?.x ?? 0, y: menu?.y ?? 0 }),
    });
    if (message.text) {
      menuItems.push({
        id: "copy",
        label: "Copy text",
        icon: Copy,
        onSelect: copyText,
      });
    }
  }
  menuItems.push({
    id: "star",
    label: starred ? "Unstar message" : "Star message",
    icon: Star,
    onSelect: () => toggleStar(message.chatId, message.id, message.fromMe),
  });
  if (!deleted && message.fromMe && message.kind === "text") {
    menuItems.push({
      id: "edit",
      label: "Edit message",
      icon: Pencil,
      onSelect: () => onEdit(message),
    });
  }
  menuItems.push({ kind: "separator", id: "separator" });
  menuItems.push({
    id: "delete",
    label: "Delete message",
    icon: Trash,
    danger: true,
    onSelect: () =>
      setPopover({ kind: "delete", x: menu?.x ?? 0, y: menu?.y ?? 0 }),
  });

  const toggle = (emoji: string) =>
    toggleReaction(message.chatId, message.id, emoji, message.fromMe);

  return (
    <div className={`message-row${message.fromMe ? " out" : ""}`}>
      <div className="message-stack">
        <div className={classes.join(" ")} onContextMenu={openMenu}>
          {quote ? (
            <QuotePreview
              author={quote.senderName ?? "Reply"}
              text={quote.preview}
            />
          ) : null}
          {deleted ? (
            <span className="deleted-text">This message was deleted</span>
          ) : (
            <MessageContent message={message} />
          )}
          <span className="bubble-meta">
            {formatBubbleTime(message.timestamp)}
            {message.fromMe && <StatusTick status={message.status} />}
          </span>
        </div>
        {!deleted && reactions && Object.keys(reactions).length > 0 ? (
          <ReactionBar reactions={reactions} mine={myReaction} onToggle={toggle} />
        ) : null}
      </div>

      {menu ? (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={() => setMenu(null)}
        />
      ) : null}

      {popover ? (
        <MessagePopover
          kind={popover.kind}
          x={popover.x}
          y={popover.y}
          canDeleteForEveryone={message.fromMe}
          onClose={() => setPopover(null)}
          onPickEmoji={toggle}
          onDelete={(forEveryone) =>
            revokeMessage(message.chatId, message.id, forEveryone)
          }
        />
      ) : null}
    </div>
  );
}

function MessageContent({ message }: { message: Message }) {
  switch (message.kind) {
    case "image":
    case "gif":
      return <ImageContent message={message} />;
    case "sticker":
      return <ImageContent message={message} sticker />;
    case "video":
      return <VideoContent message={message} />;
    case "voiceNote":
      return <VoiceNoteContent message={message} />;
    case "audio":
    case "document":
      return <FileContent message={message} />;
    default:
      // text and every kind without a dedicated bubble keep the plain text
      // fallback that Conversation used before.
      return message.text ? (
        <span className="bubble-text">{message.text}</span>
      ) : null;
  }
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

/** Clipboard fallback for non-secure webview contexts. */
function fallbackCopy(text: string) {
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  try {
    document.execCommand("copy");
  } catch {
    // Clipboard access is best-effort.
  }
  document.body.removeChild(area);
}
