/** A single message bubble: text or media, quoted context, reactions and the
 * right-click menu. Composed from the per-kind media components in this
 * folder; the date dividers, tails and lazy loading stay in Conversation. */

import { useState, type MouseEvent as ReactMouseEvent } from "react";
import {
  Copy,
  Forward,
  Pencil,
  Pin,
  PinOff,
  Reply,
  SmilePlus,
  Star,
  Trash,
} from "lucide-react";
import { invokeCore, isTauri } from "../../lib/ipc";
import { useTranslation } from "../../lib/i18n";
import { formatBubbleTime } from "../../lib/time";
import type { ChatSummary, Message, MessageStatus } from "../../lib/types";
import { useAppStore } from "../../store/app";
import { ContextMenu, type ContextMenuEntry } from "../ContextMenu";
import { Check, CheckCheck, Clock } from "../icons";
import { FileContent } from "./FileContent";
import { ForwardModal } from "./ForwardModal";
import { ImageContent } from "./ImageContent";
import { MessagePopover } from "./MessagePopover";
import { PollContent } from "./PollContent";
import { QuotePreview } from "./QuotePreview";
import { ReactionBar } from "./ReactionBar";
import { VideoContent } from "./VideoContent";
import { VoiceNoteContent } from "./VoiceNoteContent";
import { messagePreview } from "./media";
import { setMessagePinned, useMessagePinned } from "./messageLocalState";

interface MessageBubbleProps {
  message: Message;
  chat: ChatSummary;
  /** True when the next message starts a new sender/day group. */
  tail: boolean;
  /** True when the message sits between two messages of the same group. */
  middle?: boolean;
  onEdit: (message: Message) => void;
}

export function MessageBubble({
  message,
  chat,
  tail,
  middle = false,
  onEdit,
}: MessageBubbleProps) {
  const { t } = useTranslation();
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
  const pinned = useMessagePinned(message.id);

  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [forwardOpen, setForwardOpen] = useState(false);
  const [popover, setPopover] = useState<{
    kind: "react" | "delete" | "pin";
    x: number;
    y: number;
  } | null>(null);

  // Protocol chatter (receipts, key shares, sync notifications) is not user
  // content and must not render as an empty bubble.
  if (message.kind === "system") {
    return null;
  }

  const isMedia =
    message.kind === "image" ||
    message.kind === "gif" ||
    message.kind === "video" ||
    message.kind === "sticker";

  const classes = ["bubble"];
  if (message.fromMe) classes.push("out");
  if (tail) classes.push(message.fromMe ? "tail-out" : "tail-in");
  if (middle) classes.push(message.fromMe ? "mid-out" : "mid-in");
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

  // Pin state is UI-only: the core exposes no "which messages are pinned"
  // snapshot, so this is optimistic and reverts if the IPC call fails.
  const pinMessage = (days: number) => {
    setMessagePinned(message.id, true);
    if (!isTauri()) return;
    void invokeCore("message_pin", {
      chatId: message.chatId,
      messageId: message.id,
      days,
      fromMe: message.fromMe,
    }).catch((error: unknown) => {
      console.error("message_pin failed", error);
      setMessagePinned(message.id, false);
    });
  };

  const unpinMessage = () => {
    setMessagePinned(message.id, false);
    if (!isTauri()) return;
    void invokeCore("message_unpin", {
      chatId: message.chatId,
      messageId: message.id,
      fromMe: message.fromMe,
    }).catch((error: unknown) => {
      console.error("message_unpin failed", error);
      setMessagePinned(message.id, true);
    });
  };

  const menuItems: ContextMenuEntry[] = [];
  if (!deleted) {
    menuItems.push({
      id: "reply",
      label: t("message.reply"),
      icon: Reply,
      onSelect: () =>
        setReplyTo({
          chatId: message.chatId,
          messageId: message.id,
          preview: messagePreview(message),
          senderName: message.fromMe ? t("common.you") : chat.name,
        }),
    });
    menuItems.push({
      id: "react",
      label: t("message.react"),
      icon: SmilePlus,
      onSelect: () =>
        setPopover({ kind: "react", x: menu?.x ?? 0, y: menu?.y ?? 0 }),
    });
    menuItems.push({
      id: "forward",
      label: t("message.forward"),
      icon: Forward,
      onSelect: () => setForwardOpen(true),
    });
    if (message.text) {
      menuItems.push({
        id: "copy",
        label: t("message.copyText"),
        icon: Copy,
        onSelect: copyText,
      });
    }
  }
  menuItems.push({
    id: "star",
    label: starred ? t("message.unstar") : t("message.star"),
    icon: Star,
    onSelect: () => toggleStar(message.chatId, message.id, message.fromMe),
  });
  if (!deleted) {
    menuItems.push(
      pinned
        ? {
            id: "unpin",
            label: t("message.unpin"),
            icon: PinOff,
            onSelect: unpinMessage,
          }
        : {
            id: "pin",
            label: t("message.pin"),
            icon: Pin,
            onSelect: () =>
              setPopover({ kind: "pin", x: menu?.x ?? 0, y: menu?.y ?? 0 }),
          },
    );
  }
  if (!deleted && message.fromMe && message.kind === "text") {
    menuItems.push({
      id: "edit",
      label: t("message.edit"),
      icon: Pencil,
      onSelect: () => onEdit(message),
    });
  }
  menuItems.push({ kind: "separator", id: "separator" });
  menuItems.push({
    id: "delete",
    label: t("message.delete"),
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
              author={quote.senderName ?? t("common.reply")}
              text={quote.preview}
            />
          ) : null}
          {deleted ? (
            <span className="deleted-text">{t("message.deleted")}</span>
          ) : (
            <MessageContent message={message} />
          )}
          <span className="bubble-meta">
            {pinned ? (
              <Pin
                size={12}
                className="pin-glyph"
                aria-label={t("message.pinnedAria")}
              />
            ) : null}
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
          onPin={pinMessage}
        />
      ) : null}

      {forwardOpen ? (
        <ForwardModal
          message={message}
          onClose={() => setForwardOpen(false)}
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
    case "poll":
      return <PollContent message={message} />;
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
