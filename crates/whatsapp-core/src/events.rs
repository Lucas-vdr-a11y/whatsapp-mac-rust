//! The domain event bus.
//!
//! Upstream protocol events are translated into this small, stable enum. The
//! desktop host forwards every variant to the webview verbatim, so the enum
//! doubles as the IPC schema: keep it serializable and additive.

#[cfg(feature = "calls")]
use crate::calls::manager::CallUpdate;
use crate::types::{Jid, Message, MessageStatus};
use serde::Serialize;

/// Every event the core can emit to the UI.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum CoreEvent {
    /// Connection state machine transitioned.
    Connection(ConnectionEvent),
    /// Pairing flow progress.
    Pairing(PairingEvent),
    /// A message was received or a local echo was stored.
    Message(Message),
    /// Delivery state of an outgoing message changed.
    MessageStatusChanged(MessageStatusChangedEvent),
    /// A chat was created or its metadata changed.
    ChatUpdated(ChatUpdatedEvent),
    /// Typing indicator.
    Typing(TypingEvent),
    /// Online/offline presence of a contact.
    Presence(PresenceEvent),
    /// A fatal or recoverable error worth surfacing to the user.
    Error(ErrorEvent),
    /// A call lifecycle update (ringing, phase changes, ended, missed).
    #[cfg(feature = "calls")]
    Call(CallUpdate),
}

/// Connection lifecycle states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    /// No socket, no session activity.
    Disconnected,
    /// Connecting or reconnecting.
    Connecting,
    /// Handshake complete, events flowing.
    Connected,
}

/// Payload of [`CoreEvent::Connection`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionEvent {
    /// New state.
    pub state: ConnectionState,
    /// Human-readable detail, when there is one.
    pub reason: Option<String>,
}

/// Pairing (linking) progress.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PairingEvent {
    /// Fresh QR payload; the UI renders it as a QR code.
    QrCode {
        /// Raw QR string to encode.
        code: String,
        /// Seconds until this payload is rotated.
        timeout_secs: u64,
    },
    /// The server's rotation budget is used up. The UI must offer a refresh;
    /// no further codes arrive on this connection attempt.
    QrCodesExhausted,
    /// Alternative 8-character pairing code.
    PairCode {
        /// The code to type on the phone.
        code: String,
    },
    /// Linking succeeded.
    PairSuccess {
        /// The account's own JID.
        jid: Jid,
    },
    /// Linking failed and must be retried.
    PairFailure {
        /// Why it failed.
        reason: String,
    },
}

/// Payload of [`CoreEvent::MessageStatusChanged`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageStatusChangedEvent {
    /// Chat containing the message.
    pub chat_id: Jid,
    /// Message id.
    pub message_id: String,
    /// New state.
    pub status: MessageStatus,
}

/// Payload of [`CoreEvent::ChatUpdated`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUpdatedEvent {
    /// Chat that changed.
    pub chat_id: Jid,
}

/// Payload of [`CoreEvent::Typing`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingEvent {
    /// Chat where typing happened.
    pub chat_id: Jid,
    /// Who is typing.
    pub sender_id: Jid,
    /// True when the sender started typing, false when they stopped.
    pub is_typing: bool,
}

/// Payload of [`CoreEvent::Presence`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceEvent {
    /// Contact whose presence changed.
    pub jid: Jid,
    /// True when online.
    pub online: bool,
    /// Unix seconds of last activity, when known.
    pub last_seen_ts: Option<u64>,
}

/// Payload of [`CoreEvent::Error`].
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    /// Stable, machine-readable code for the UI to switch on.
    pub code: String,
    /// Human-readable description.
    pub message: String,
}
