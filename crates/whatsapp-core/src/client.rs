//! WhatsApp protocol client.
//!
//! [`WaClient`] wraps the upstream `whatsapp-rust` bot: it owns the connection
//! lifecycle, translates upstream events into the stable [`CoreEvent`] bus,
//! persists chats/messages through [`Store`], and exposes the command surface
//! used by the desktop host.
//!
//! Threading: handlers run on the upstream runtime's tasks. Our handlers do no
//! `await` before they finish, so they stay cheap; SQLite writes are short
//! transactions (a follow-up will move them to a blocking pool if profiling
//! shows jank).

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::{Mutex, broadcast};
use whatsapp_rust::chrono::{DateTime, Utc};
use whatsapp_rust::prelude::*;
use whatsapp_rust::wacore::types::events::LazyHistorySync;
use whatsapp_rust::wacore::types::presence::{ChatPresence, ReceiptType};
use whatsapp_rust::waproto::whatsapp as wa;

use crate::error::{CoreError, Result};
use crate::events::{
    ChatUpdatedEvent, ConnectionEvent, ConnectionState, CoreEvent, ErrorEvent,
    MessageStatusChangedEvent, PairingEvent, PresenceEvent, TypingEvent,
};
use crate::store::Store;
use crate::types::{ChatSummary, Jid, Message, MessageKind, MessageStatus};

/// Capacity of the event bus (events buffered per subscriber).
pub const EVENT_BUS_CAPACITY: usize = 1024;

/// Fallback sender id for own messages before the account JID is known.
const ME_PLACEHOLDER: &str = "me";

/// Configuration for [`WaClient`].
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// Directory holding the protocol session database and future media cache.
    pub data_dir: PathBuf,
    /// Device name shown under "Linked devices" on the phone.
    pub device_name: String,
}

impl ClientConfig {
    /// Configuration rooted at `data_dir`.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            device_name: "RustWA".to_owned(),
        }
    }
}

/// Connection lifecycle handle around the upstream bot.
pub struct WaClient {
    config: ClientConfig,
    store: Arc<Store>,
    events: broadcast::Sender<CoreEvent>,
    handle: Mutex<Option<BotHandle>>,
    own_jid: Arc<StdMutex<Option<Jid>>>,
    /// 0 = disconnected, 1 = connecting, 2 = connected. Shared with event
    /// handlers so upstream lifecycle events update the state machine.
    connection: Arc<AtomicU8>,
}

impl WaClient {
    /// Create a client. Call [`WaClient::connect`] to start pairing/linking.
    pub fn new(config: ClientConfig, store: Arc<Store>) -> Self {
        let (events, _receiver) = broadcast::channel(EVENT_BUS_CAPACITY);
        Self {
            config,
            store,
            events,
            handle: Mutex::new(None),
            own_jid: Arc::new(StdMutex::new(None)),
            connection: Arc::new(AtomicU8::new(0)),
        }
    }

    /// Subscribe to the domain event bus.
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }

    /// The persistent store, for listing chats and messages.
    pub fn store(&self) -> Arc<Store> {
        Arc::clone(&self.store)
    }

    /// Current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        match self.connection.load(Ordering::Relaxed) {
            2 => ConnectionState::Connected,
            1 => ConnectionState::Connecting,
            _ => ConnectionState::Disconnected,
        }
    }

    /// The account's own JID, once known.
    pub fn own_jid(&self) -> Option<Jid> {
        self.own_jid.lock().ok().and_then(|guard| guard.clone())
    }

    /// Create the session store and start connecting. QR codes (or pair codes)
    /// arrive on the event bus until the device is linked.
    pub async fn connect(&self) -> Result<()> {
        let mut guard = self.handle.lock().await;
        if guard.is_some() {
            // Already connecting or connected. Treat as success so callers can
            // retry idempotently (the UI's retry button, StrictMode re-runs).
            return Ok(());
        }

        std::fs::create_dir_all(&self.config.data_dir)
            .map_err(|error| CoreError::Storage(error.to_string()))?;
        let session_path = self.config.data_dir.join("session.db");
        let session_path = session_path
            .to_str()
            .ok_or_else(|| CoreError::Internal("session path is not valid UTF-8".into()))?;

        let backend = SqliteStore::new(session_path)
            .await
            .map_err(|error| CoreError::Storage(error.to_string()))?;

        self.set_connection(ConnectionState::Connecting, None);

        let bus = self.events.clone();
        let bus_qr = bus.clone();
        let bus_pair_code = bus.clone();
        let bus_lifecycle = bus.clone();
        let bus_messages = bus.clone();
        let bus_updates = bus.clone();
        let bus_history = bus.clone();
        let store_messages = Arc::clone(&self.store);
        let store_updates = Arc::clone(&self.store);
        let store_history = Arc::clone(&self.store);

        let bot = Bot::builder()
            .with_backend(backend)
            .on_qr_code(move |code, timeout| {
                let bus = bus_qr.clone();
                async move {
                    tracing::info!(
                        "pairing QR code received ({} bytes, valid {} s)",
                        code.len(),
                        timeout.as_secs()
                    );
                    let _ = bus.send(CoreEvent::Pairing(PairingEvent::QrCode {
                        code,
                        timeout_secs: timeout.as_secs(),
                    }));
                }
            })
            .on_pair_code(move |code, _timeout| {
                let bus = bus_pair_code.clone();
                async move {
                    let _ = bus.send(CoreEvent::Pairing(PairingEvent::PairCode { code }));
                }
            })
            .on_connected({
                let bus = bus.clone();
                let connection = Arc::clone(&self.connection);
                move |_client| {
                    let bus = bus.clone();
                    let connection = Arc::clone(&connection);
                    async move {
                        connection.store(2, Ordering::Relaxed);
                        tracing::info!("connected to WhatsApp");
                        let _ = bus.send(CoreEvent::Connection(ConnectionEvent {
                            state: ConnectionState::Connected,
                            reason: None,
                        }));
                    }
                }
            })
            .on_logged_out({
                let bus = bus.clone();
                let connection = Arc::clone(&self.connection);
                move |_info| {
                    let bus = bus.clone();
                    let connection = Arc::clone(&connection);
                    async move {
                        connection.store(0, Ordering::Relaxed);
                        let _ = bus.send(CoreEvent::Connection(ConnectionEvent {
                            state: ConnectionState::Disconnected,
                            reason: Some("logged out from this device".to_owned()),
                        }));
                    }
                }
            })
            .on_event_for(
                &[
                    EventKind::Disconnected,
                    EventKind::PairSuccess,
                    EventKind::PairError,
                    EventKind::PairingQrCodesExhausted,
                    EventKind::ConnectFailure,
                    EventKind::StreamReplaced,
                    EventKind::TemporaryBan,
                ],
                {
                    let connection = Arc::clone(&self.connection);
                    let own_jid = Arc::clone(&self.own_jid);
                    move |event, _client| {
                        let bus = bus_lifecycle.clone();
                        let connection = Arc::clone(&connection);
                        let own_jid = Arc::clone(&own_jid);
                        async move {
                            handle_lifecycle_event(&bus, event.as_ref(), &connection, &own_jid);
                        }
                    }
                },
            )
            .on_message(move |context| {
                let bus = bus_messages.clone();
                let store = Arc::clone(&store_messages);
                async move {
                    handle_inbound_message(&bus, &store, &context);
                }
            })
            .on_event_for(
                &[
                    EventKind::Receipt,
                    EventKind::ChatPresence,
                    EventKind::Presence,
                ],
                move |event, _client| {
                    let bus = bus_updates.clone();
                    let store = Arc::clone(&store_updates);
                    async move {
                        handle_update_event(&bus, &store, event.as_ref());
                    }
                },
            )
            .on_event_for(&[EventKind::HistorySync], move |event, _client| {
                let bus = bus_history.clone();
                let store = Arc::clone(&store_history);
                async move {
                    // Decompression and parsing are CPU-bound: keep them off
                    // the async worker threads.
                    if let Event::HistorySync(sync) = event.as_ref() {
                        let sync = sync.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            import_history_sync(&bus, &store, &sync);
                        });
                    }
                }
            })
            .build()
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        *guard = Some(bot.spawn());
        Ok(())
    }

    /// Gracefully stop the client without unlinking the device.
    pub async fn shutdown(&self) {
        let handle = self.handle.lock().await.take();
        if let Some(handle) = handle {
            handle.shutdown().await;
        }
        self.set_connection(ConnectionState::Disconnected, None);
    }

    /// Unlink this device and stop all session activity.
    pub async fn logout(&self) -> Result<()> {
        let handle = self.handle.lock().await.take();
        if let Some(handle) = handle {
            let client = handle.client();
            client.logout().await;
            handle.shutdown().await;
        }
        self.own_jid.lock().ok().map(|mut guard| guard.take());
        self.set_connection(ConnectionState::Disconnected, None);
        Ok(())
    }

    /// Stop the current connection attempt and start a fresh pairing flow.
    ///
    /// Keeps an already-linked session on disk: a paired device simply
    /// reconnects, an unpaired one gets fresh QR refs from the server.
    pub async fn restart_pairing(&self) -> Result<()> {
        self.shutdown().await;
        self.connect().await
    }

    /// Delete the local session and start a brand-new pairing flow.
    ///
    /// This is the recovery path when a previous linking attempt left the
    /// session store in a state the server rejects.
    pub async fn reset_session(&self) -> Result<()> {
        self.shutdown().await;
        let dir = self.config.data_dir.clone();
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|error| CoreError::Storage(error.to_string()))?;
        }
        self.own_jid.lock().ok().map(|mut guard| guard.take());
        self.connect().await
    }

    /// Send a plain text message. Returns the stored local echo.
    pub async fn send_text(&self, chat_id: &Jid, text: &str) -> Result<Message> {
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput("message text is empty".into()));
        }

        let client = self.client().await?;
        let to = to_upstream_jid(chat_id)?;
        let sent = client
            .send_message(&to, wa::Message::text(text))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let sender_id = self
            .own_jid
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| Jid::new(ME_PLACEHOLDER));

        let message = Message {
            id: sent.message_id,
            chat_id: chat_id.clone(),
            sender_id,
            from_me: true,
            timestamp: now_unix(),
            kind: MessageKind::Text,
            text: Some(text.to_owned()),
            status: MessageStatus::Sent,
        };

        // Chat row first: the message references it.
        self.store.record_message_activity(
            chat_id,
            &preview_for(&message),
            message.timestamp,
            None,
            false,
        )?;
        self.store.upsert_message(&message)?;
        let _ = self.events.send(CoreEvent::Message(message.clone()));
        Ok(message)
    }

    /// Chats, newest activity first (pinned first).
    pub fn list_chats(&self) -> Result<Vec<ChatSummary>> {
        self.store.list_chats()
    }

    /// Messages of one chat, oldest first.
    pub fn list_messages(&self, chat_id: &Jid, limit: u32) -> Result<Vec<Message>> {
        self.store.list_messages(chat_id, limit)
    }

    /// Pin or unpin a chat across the account's devices.
    pub async fn set_chat_pinned(&self, chat_id: &Jid, pinned: bool) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        let actions = client.chat_actions();
        let result = if pinned {
            actions.pin_chat(&jid).await
        } else {
            actions.unpin_chat(&jid).await
        };
        result.map_err(|error| CoreError::Protocol(error.to_string()))?;
        self.store.set_chat_pinned(chat_id, pinned)
    }

    /// Mute a chat indefinitely, or unmute it.
    pub async fn set_chat_muted(&self, chat_id: &Jid, muted: bool) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        let actions = client.chat_actions();
        let result = if muted {
            actions.mute_chat(&jid).await
        } else {
            actions.unmute_chat(&jid).await
        };
        result.map_err(|error| CoreError::Protocol(error.to_string()))?;
        self.store.set_chat_muted(chat_id, muted)
    }

    /// Archive or unarchive a chat.
    pub async fn set_chat_archived(&self, chat_id: &Jid, archived: bool) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        let actions = client.chat_actions();
        let result = if archived {
            actions.archive_chat(&jid, None).await
        } else {
            actions.unarchive_chat(&jid, None).await
        };
        result.map_err(|error| CoreError::Protocol(error.to_string()))?;
        self.store.set_chat_archived(chat_id, archived)
    }

    /// Mark a chat as read (clears the unread counter locally and remotely).
    pub async fn mark_chat_read(&self, chat_id: &Jid) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        client
            .chat_actions()
            .mark_chat_as_read(&jid, true, None)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        self.store.mark_chat_read(chat_id)
    }

    /// Send a typing/paused chat-state update for a chat.
    pub async fn set_typing(&self, chat_id: &Jid, typing: bool) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        let chatstate = client.chatstate();
        let result = if typing {
            chatstate.send_composing(&jid).await
        } else {
            chatstate.send_paused(&jid).await
        };
        result.map_err(|error| CoreError::Protocol(error.to_string()))
    }

    pub(crate) async fn client(&self) -> Result<Arc<Client>> {
        let guard = self.handle.lock().await;
        guard
            .as_ref()
            .map(BotHandle::client)
            .ok_or(CoreError::NotConnected)
    }

    /// Publish an event on the domain bus. Used by the message-action surface
    /// in `actions.rs` (local echo of an outgoing quoted reply).
    pub(crate) fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }

    fn set_connection(&self, state: ConnectionState, reason: Option<String>) {
        let value = match state {
            ConnectionState::Disconnected => 0,
            ConnectionState::Connecting => 1,
            ConnectionState::Connected => 2,
        };
        self.connection.store(value, Ordering::Relaxed);
        let _ = self
            .events
            .send(CoreEvent::Connection(ConnectionEvent { state, reason }));
    }
}

/// Connection-level events.
fn handle_lifecycle_event(
    bus: &broadcast::Sender<CoreEvent>,
    event: &Event,
    connection: &AtomicU8,
    own_jid: &StdMutex<Option<Jid>>,
) {
    match event {
        Event::Disconnected(_) => {
            connection.store(0, Ordering::Relaxed);
            let _ = bus.send(CoreEvent::Connection(ConnectionEvent {
                state: ConnectionState::Disconnected,
                reason: None,
            }));
        }
        Event::PairSuccess(success) => {
            if let Ok(mut guard) = own_jid.lock() {
                *guard = Some(from_upstream_jid(&success.id));
            }
            tracing::info!(jid = %success.id, "device paired");
            let _ = bus.send(CoreEvent::Pairing(PairingEvent::PairSuccess {
                jid: from_upstream_jid(&success.id),
            }));
        }
        Event::PairError(error) => {
            let _ = bus.send(CoreEvent::Pairing(PairingEvent::PairFailure {
                reason: format!("{error:?}"),
            }));
        }
        Event::PairingQrCodesExhausted(exhausted) => {
            tracing::info!(
                disconnected = exhausted.disconnected,
                "QR rotation budget exhausted; a restart is required"
            );
            let _ = bus.send(CoreEvent::Pairing(PairingEvent::QrCodesExhausted));
        }
        Event::ConnectFailure(failure) => {
            connection.store(0, Ordering::Relaxed);
            let _ = bus.send(CoreEvent::Connection(ConnectionEvent {
                state: ConnectionState::Disconnected,
                reason: Some(format!("{failure:?}")),
            }));
        }
        Event::StreamReplaced(replaced) => {
            let _ = bus.send(CoreEvent::Error(ErrorEvent {
                code: "streamReplaced".to_owned(),
                message: format!("{replaced:?}"),
            }));
        }
        Event::TemporaryBan(ban) => {
            let _ = bus.send(CoreEvent::Error(ErrorEvent {
                code: "temporaryBan".to_owned(),
                message: format!("{ban:?}"),
            }));
        }
        _ => {}
    }
}

/// Incoming messages: persist, then publish.
fn handle_inbound_message(
    bus: &broadcast::Sender<CoreEvent>,
    store: &Store,
    context: &MessageContext,
) {
    let info = &context.info;
    let message = Message {
        id: info.id.to_string(),
        chat_id: from_upstream_jid(&info.source.chat),
        sender_id: from_upstream_jid(&info.source.sender),
        from_me: info.source.is_from_me,
        timestamp: timestamp_to_unix(&info.timestamp),
        kind: classify(&context.message),
        text: context.message.text_content().map(str::to_owned),
        status: if info.source.is_from_me {
            MessageStatus::Sent
        } else {
            MessageStatus::Delivered
        },
    };

    let name_hint = (!info.push_name.trim().is_empty()).then_some(info.push_name.as_str());
    let preview = preview_for(&message);

    // The chat row must exist before the message (foreign key), so record the
    // activity first and only then insert the message.
    if let Err(error) = store.record_message_activity(
        &message.chat_id,
        &preview,
        message.timestamp,
        name_hint,
        !message.from_me,
    ) {
        tracing::warn!(%error, "failed to update chat activity");
    }
    if let Err(error) = store.upsert_message(&message) {
        tracing::warn!(%error, "failed to store inbound message");
    }

    let _ = bus.send(CoreEvent::Message(message));
}

/// Receipts, typing and presence: publish and, for receipts, persist.
fn handle_update_event(bus: &broadcast::Sender<CoreEvent>, store: &Store, event: &Event) {
    match event {
        Event::Receipt(receipt) => {
            let Some(status) = receipt_status(&receipt.r#type) else {
                return;
            };
            let chat_id = from_upstream_jid(&receipt.source.chat);
            for message_id in &receipt.message_ids {
                let message_id = message_id.to_string();
                if let Err(error) = store.set_message_status(&message_id, status) {
                    tracing::warn!(%error, "failed to update message status");
                }
                let _ = bus.send(CoreEvent::MessageStatusChanged(MessageStatusChangedEvent {
                    chat_id: chat_id.clone(),
                    message_id,
                    status,
                }));
            }
        }
        Event::ChatPresence(presence) => {
            let _ = bus.send(CoreEvent::Typing(TypingEvent {
                chat_id: from_upstream_jid(&presence.source.chat),
                sender_id: from_upstream_jid(&presence.source.sender),
                is_typing: matches!(presence.state, ChatPresence::Composing),
            }));
        }
        Event::Presence(presence) => {
            let _ = bus.send(CoreEvent::Presence(PresenceEvent {
                jid: from_upstream_jid(&presence.from),
                online: !presence.unavailable,
                last_seen_ts: presence.last_seen.map(|value| value.timestamp() as u64),
            }));
        }
        _ => {}
    }
}

/// Import a history-sync blob: conversations and their messages.
///
/// Runs on a blocking thread (the caller uses `spawn_blocking`); the stream
/// yields conversations lazily so memory stays bounded even for large blobs.
fn import_history_sync(bus: &broadcast::Sender<CoreEvent>, store: &Store, sync: &LazyHistorySync) {
    let mut stream = sync.stream();
    let mut chat_count = 0usize;
    let mut message_count = 0usize;

    loop {
        match stream.next_conversation() {
            Ok(Some(conversation)) => {
                let chat_id = Jid::new(conversation.id.clone());
                if chat_id.as_str().is_empty() {
                    continue;
                }

                let history_name = conversation.name.clone().unwrap_or_default();
                let name = if history_name.trim().is_empty() {
                    chat_id.user().to_owned()
                } else {
                    history_name
                };

                let summary = ChatSummary {
                    id: chat_id.clone(),
                    name,
                    last_message_preview: None,
                    last_activity_ts: conversation.last_msg_timestamp.unwrap_or(0) / 1_000,
                    unread_count: conversation.unread_count.unwrap_or(0),
                    muted: false,
                    pinned: false,
                    is_group: chat_id.is_group(),
                    is_archived: conversation.archived.unwrap_or(false),
                };
                if let Err(error) = store.upsert_chat(&summary) {
                    tracing::warn!(%error, chat = %chat_id, "history: chat upsert failed");
                    continue;
                }
                chat_count += 1;

                let mut newest: Option<Message> = None;
                for entry in &conversation.messages {
                    let Some(info) = entry.message.as_option() else {
                        continue;
                    };
                    let Some(message) = convert_history_message(info) else {
                        continue;
                    };
                    if let Err(error) = store.upsert_message(&message) {
                        tracing::warn!(%error, "history: message upsert failed");
                        continue;
                    }
                    message_count += 1;
                    if newest
                        .as_ref()
                        .is_none_or(|current| message.timestamp >= current.timestamp)
                    {
                        newest = Some(message);
                    }
                }

                if let Some(newest) = newest {
                    let _ = store.record_message_activity(
                        &chat_id,
                        &preview_for(&newest),
                        newest.timestamp,
                        None,
                        false,
                    );
                }

                let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
            }
            Ok(None) => break,
            Err(error) => {
                tracing::warn!(%error, "history: stream failed");
                break;
            }
        }
    }

    tracing::info!(
        chats = chat_count,
        messages = message_count,
        "history sync imported"
    );
}

/// Convert one history-sync message into our domain type.
fn convert_history_message(info: &wa::WebMessageInfo) -> Option<Message> {
    let key = info.key.as_option()?;
    let id = key.id.clone().unwrap_or_default();
    let chat = key.remote_jid.clone().unwrap_or_default();
    if id.is_empty() || chat.is_empty() {
        return None;
    }

    let chat_id = Jid::new(chat);
    let from_me = key.from_me.unwrap_or(false);
    let sender = key
        .participant
        .clone()
        .map(Jid::new)
        .unwrap_or_else(|| chat_id.clone());
    let body = info.message.as_option();

    Some(Message {
        id,
        chat_id,
        sender_id: if from_me {
            Jid::new(ME_PLACEHOLDER)
        } else {
            sender
        },
        from_me,
        timestamp: info.message_timestamp.unwrap_or(0) / 1_000,
        kind: body.map(classify).unwrap_or(MessageKind::Unsupported),
        text: body.and_then(|message| message.text_content().map(str::to_owned)),
        status: if from_me {
            MessageStatus::Sent
        } else {
            MessageStatus::Delivered
        },
    })
}

fn receipt_status(receipt: &ReceiptType) -> Option<MessageStatus> {
    match receipt {
        ReceiptType::Sent | ReceiptType::Sender => Some(MessageStatus::Sent),
        ReceiptType::Delivered => Some(MessageStatus::Delivered),
        ReceiptType::Read | ReceiptType::ReadSelf => Some(MessageStatus::Read),
        ReceiptType::Played => Some(MessageStatus::Played),
        _ => None,
    }
}

/// Best-effort classification of a protocol message into our UI kinds.
fn classify(message: &wa::Message) -> MessageKind {
    if message.conversation.is_some() || message.extended_text_message.is_set() {
        return MessageKind::Text;
    }
    if message.image_message.is_set() {
        return MessageKind::Image;
    }
    if message.video_message.is_set() {
        return MessageKind::Video;
    }
    if let Some(audio) = message.audio_message.as_option() {
        return if audio.ptt.unwrap_or(false) {
            MessageKind::VoiceNote
        } else {
            MessageKind::Audio
        };
    }
    if message.document_message.is_set() {
        return MessageKind::Document;
    }
    if message.sticker_message.is_set() {
        return MessageKind::Sticker;
    }
    MessageKind::Unsupported
}

/// Short, human-readable chat-list preview for a message.
fn preview_for(message: &Message) -> String {
    const MAX: usize = 120;
    let text = match message.kind {
        MessageKind::Text => message.text.clone().unwrap_or_default(),
        MessageKind::Image => "[Photo]".to_owned(),
        MessageKind::Video => "[Video]".to_owned(),
        MessageKind::VoiceNote => "[Voice message]".to_owned(),
        MessageKind::Audio => "[Audio]".to_owned(),
        MessageKind::Document => "[Document]".to_owned(),
        MessageKind::Sticker => "[Sticker]".to_owned(),
        _ => "[Message]".to_owned(),
    };
    text.chars().take(MAX).collect()
}

fn to_upstream_jid(jid: &Jid) -> Result<whatsapp_rust::Jid> {
    whatsapp_rust::Jid::from_str(jid.as_str())
        .map_err(|error| CoreError::InvalidInput(error.to_string()))
}

fn from_upstream_jid(jid: &whatsapp_rust::Jid) -> Jid {
    Jid::new(jid.to_string())
}

/// Shared with sibling modules (contacts, groups, …).
pub(crate) fn to_upstream(jid: &Jid) -> Result<whatsapp_rust::Jid> {
    to_upstream_jid(jid)
}

/// Shared with sibling modules (contacts, groups, …).
pub(crate) fn from_upstream(jid: &whatsapp_rust::Jid) -> Jid {
    from_upstream_jid(jid)
}

fn timestamp_to_unix(value: &DateTime<Utc>) -> u64 {
    value.timestamp().max(0) as u64
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::buffa::MessageField;

    fn sample_message() -> Message {
        Message {
            id: "msg-1".to_owned(),
            chat_id: Jid::new("alice@s.whatsapp.net"),
            sender_id: Jid::new("alice@s.whatsapp.net"),
            from_me: false,
            timestamp: 1_700_000_000,
            kind: MessageKind::Text,
            text: Some("hello".to_owned()),
            status: MessageStatus::Delivered,
        }
    }

    #[test]
    fn classifies_plain_text() {
        let message = wa::Message {
            conversation: Some("hello".to_owned()),
            ..Default::default()
        };
        assert_eq!(classify(&message), MessageKind::Text);
    }

    #[test]
    fn classifies_voice_note_and_audio() {
        let mut message = wa::Message::default();
        message.audio_message = MessageField::some(wa::message::AudioMessage {
            ptt: Some(true),
            ..Default::default()
        });
        assert_eq!(classify(&message), MessageKind::VoiceNote);

        message.audio_message = MessageField::some(wa::message::AudioMessage {
            ptt: Some(false),
            ..Default::default()
        });
        assert_eq!(classify(&message), MessageKind::Audio);
    }

    #[test]
    fn classifies_unknown_as_unsupported() {
        assert_eq!(classify(&wa::Message::default()), MessageKind::Unsupported);
    }

    #[test]
    fn previews_text_and_media() {
        assert_eq!(preview_for(&sample_message()), "hello");

        let media = Message {
            kind: MessageKind::Image,
            text: None,
            ..sample_message()
        };
        assert_eq!(preview_for(&media), "[Photo]");
    }

    #[test]
    fn previews_are_truncated() {
        let long = Message {
            text: Some("x".repeat(500)),
            ..sample_message()
        };
        assert_eq!(preview_for(&long).chars().count(), 120);
    }

    #[test]
    fn converts_history_messages_from_the_sync_blob() {
        let info = wa::WebMessageInfo {
            key: MessageField::some(wa::MessageKey {
                remote_jid: Some("alice@s.whatsapp.net".to_owned()),
                from_me: Some(false),
                id: Some("abc123".to_owned()),
                participant: None,
            }),
            message: MessageField::some(wa::Message {
                conversation: Some("hi from history".to_owned()),
                ..Default::default()
            }),
            message_timestamp: Some(1_700_000_000_000),
            ..Default::default()
        };

        let message = convert_history_message(&info).expect("converts");
        assert_eq!(message.id, "abc123");
        assert_eq!(message.chat_id.as_str(), "alice@s.whatsapp.net");
        assert_eq!(message.timestamp, 1_700_000_000);
        assert_eq!(message.kind, MessageKind::Text);
        assert_eq!(message.text.as_deref(), Some("hi from history"));
        assert!(!message.from_me);
    }

    #[test]
    fn skips_history_messages_without_key_or_chat() {
        let info = wa::WebMessageInfo::default();
        assert!(convert_history_message(&info).is_none());
    }
}
