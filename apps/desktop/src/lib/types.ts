/** TypeScript mirror of the `whatsapp-core` domain types (camelCase over IPC). */

export type Jid = string;

export interface ChatSummary {
  id: Jid;
  name: string;
  lastMessagePreview: string | null;
  /** Kind of the newest message, when the core knows it. */
  lastMessageKind?: MessageKind | null;
  /** True when the newest message was sent by this account. */
  lastFromMe?: boolean;
  /** Delivery status of the newest message (colors the list tick). */
  lastStatus?: MessageStatus | null;
  lastActivityTs: number;
  unreadCount: number;
  muted: boolean;
  pinned: boolean;
  isGroup: boolean;
  isArchived: boolean;
}

export type MessageKind =
  | "text"
  | "image"
  | "video"
  | "audio"
  | "voiceNote"
  | "document"
  | "sticker"
  | "gif"
  | "location"
  | "contact"
  | "poll"
  | "system"
  | "unsupported";

export type MessageStatus =
  | "pending"
  | "sent"
  | "delivered"
  | "read"
  | "played"
  | "failed";

export interface Message {
  id: string;
  chatId: Jid;
  senderId: Jid;
  fromMe: boolean;
  timestamp: number;
  kind: MessageKind;
  text: string | null;
  status: MessageStatus;
}

export type ConnectionState = "disconnected" | "connecting" | "connected";

/** Events emitted by the Rust core on the `core://event` channel. */
export type CoreEvent =
  | {
      type: "connection";
      payload: { state: ConnectionState; reason: string | null };
    }
  | {
      type: "pairing";
      payload:
        | { kind: "qrCode"; code: string; timeoutSecs: number }
        | { kind: "qrCodesExhausted" }
        | { kind: "pairCode"; code: string }
        | { kind: "pairSuccess"; jid: Jid }
        | { kind: "pairFailure"; reason: string };
    }
  | { type: "message"; payload: Message }
  | {
      type: "messageStatusChanged";
      payload: { chatId: Jid; messageId: string; status: MessageStatus };
    }
  | { type: "chatUpdated"; payload: { chatId: Jid } }
  | {
      type: "reaction";
      payload: { chatId: Jid; messageId: string; reactor: Jid; emoji: string };
    }
  | { type: "messageRevoked"; payload: { chatId: Jid; messageId: string } }
  | {
      type: "messageEdited";
      payload: { chatId: Jid; messageId: string; text: string };
    }
  | {
      type: "typing";
      payload: { chatId: Jid; senderId: Jid; isTyping: boolean };
    }
  | {
      type: "presence";
      payload: { jid: Jid; online: boolean; lastSeenTs: number | null };
    }
  | { type: "error"; payload: { code: string; message: string } };
