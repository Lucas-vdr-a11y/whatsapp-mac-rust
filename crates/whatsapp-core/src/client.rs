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
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::{Mutex, broadcast};
use whatsapp_rust::buffa::Message as _;
use whatsapp_rust::chrono::{DateTime, Utc};
use whatsapp_rust::prelude::*;
use whatsapp_rust::wacore::types::events::LazyHistorySync;
use whatsapp_rust::wacore::types::presence::{ChatPresence, ReceiptType};
use whatsapp_rust::waproto::whatsapp as wa;

use crate::error::{CoreError, Result};
use crate::events::{
    ChatRemovedEvent, ChatUpdatedEvent, ConnectionEvent, ConnectionState, CoreEvent, ErrorEvent,
    MessageEditedEvent, MessageRevokedEvent, MessageStatusChangedEvent, PairingEvent, PresenceEvent,
    ReactionEvent, TypingEvent,
};
use crate::media::MediaStore as _;
use crate::store::Store;
use crate::types::{ChatSummary, Jid, Message, MessageKind, MessageStatus};

/// Capacity of the event bus (events buffered per subscriber).
pub const EVENT_BUS_CAPACITY: usize = 1024;

/// Fallback sender id for own messages before the account JID is known.
const ME_PLACEHOLDER: &str = "me";

/// Cap for the group-subject resolution pass.
const GROUP_NAME_PASS_CAP: u32 = 1000;
/// Cap for the contact-name resolution pass.
const CONTACT_NAME_PASS_CAP: u32 = 1000;
/// Silence window before a requested name pass actually runs, so a
/// history-sync burst collapses into a single pass instead of hammering
/// usync/group-metadata per event.
const NAME_PASS_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(30);
/// Pause between connecting and the full app-state replay, so the connection
/// (and the incremental chat-list sync that runs first) has settled before
/// the full `regular` snapshot re-downloads every patch.
const FULL_SYNC_DELAY: std::time::Duration = std::time::Duration::from_secs(10);
/// Pause between connecting and the contact → chat backfill, so the address
/// book and any history import have landed in the store first.
const CONTACT_BACKFILL_DELAY: std::time::Duration = std::time::Duration::from_secs(12);

/// Wake-up handle for the event-driven name-resolution passes (audit S6).
///
/// The passes used to run once, clock-driven and capped, so chats imported
/// later kept raw JID names forever. They now run on demand: every
/// history-sync import and every burst of chat updates requests a run, and a
/// scheduler task debounces the requests into a single pass.
#[derive(Clone, Default)]
struct NamePassTrigger {
    notify: Arc<tokio::sync::Notify>,
}

impl NamePassTrigger {
    /// Ask for a (debounced) pass run. Bursts collapse into one.
    fn request(&self) {
        // `notify_one` stores a permit when nobody waits yet, so requests
        // that land before the scheduler awaits are never lost.
        self.notify.notify_one();
    }
}

/// One-shot gate for the full `regular` app-state replay (audit S1).
///
/// Incremental app-state syncs never re-deliver chat-list patches another
/// device already acked, which is why archive/mute/pin flags were missing.
/// Once per session — shortly after connect, independent of history sync
/// (which after initial pairing never runs again) — the `regular` collection
/// is re-fetched as a full snapshot, replaying every patch.
#[derive(Clone, Default)]
struct FullSyncGate {
    fired: Arc<AtomicBool>,
}

impl FullSyncGate {
    /// Claim the replay slot. Returns `true` only for the first caller per
    /// session, so reconnects don't repeat the snapshot download.
    fn fire(&self) -> bool {
        !self.fired.swap(true, Ordering::SeqCst)
    }
}

/// Configuration for [`WaClient`].
#[derive(Clone)]
pub struct ClientConfig {
    /// Directory holding the protocol session database and future media cache.
    pub data_dir: PathBuf,
    /// Device name shown under "Linked devices" on the phone.
    pub device_name: String,
    /// Platform capture backend for calls. `None` keeps calling disabled even
    /// when the `calls` feature is compiled in.
    #[cfg(feature = "calls")]
    pub call_media: Option<Arc<dyn crate::calls::manager::CallMediaFactory>>,
}

impl std::fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("ClientConfig");
        debug
            .field("data_dir", &self.data_dir)
            .field("device_name", &self.device_name);
        #[cfg(feature = "calls")]
        debug.field(
            "call_media",
            &self.call_media.as_ref().map(|_| "dyn CallMediaFactory"),
        );
        debug.finish()
    }
}

impl ClientConfig {
    /// Configuration rooted at `data_dir`.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            device_name: "RustWA".to_owned(),
            #[cfg(feature = "calls")]
            call_media: None,
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
    /// Guards the one-shot full `regular` app-state replay (see
    /// [`FullSyncGate`]): at most once per session.
    full_sync: FullSyncGate,
    /// Active call manager; present once connected with a media backend.
    #[cfg(feature = "calls")]
    calls: Arc<tokio::sync::Mutex<Option<crate::calls::manager::CallManager>>>,
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
            full_sync: FullSyncGate::default(),
            #[cfg(feature = "calls")]
            calls: Arc::new(tokio::sync::Mutex::new(None)),
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

    /// Root directory for session state and cached media.
    pub(crate) fn data_dir(&self) -> &std::path::Path {
        &self.config.data_dir
    }

    /// Clone of the active call manager, when calling is available.
    #[cfg(feature = "calls")]
    pub(crate) async fn call_manager(&self) -> Option<crate::calls::manager::CallManager> {
        self.calls.lock().await.clone()
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

        // One-time contact backfill for devices linked before the client
        // handled `ContactUpdate`: those app-state patches were acked and are
        // never replayed. Clearing the collection's stored version forces a
        // full re-download exactly once.
        let backfill_marker = self.config.data_dir.join("contacts-backfilled");
        if matches!(self.store.contact_count(), Ok(0)) && !backfill_marker.exists() {
            match reset_app_state_collection(&self.config.data_dir, "critical_unblock_low") {
                Ok(()) => {
                    let _ = std::fs::write(&backfill_marker, b"1");
                    tracing::info!("cleared contact app-state version for a one-time backfill");
                }
                Err(error) => {
                    tracing::warn!(%error, "could not reset the contact app-state version");
                }
            }
        }

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
        let name_trigger = NamePassTrigger::default();
        let name_trigger_messages = name_trigger.clone();
        let name_trigger_updates = name_trigger.clone();
        let name_trigger_history = name_trigger.clone();
        // The call event slot exists in every build; only the `calls`
        // feature compiles the manager that consumes it.
        #[cfg(feature = "calls")]
        let call_slot: Arc<tokio::sync::Mutex<Option<crate::calls::manager::CallManager>>> =
            Arc::clone(&self.calls);
        #[cfg(not(feature = "calls"))]
        let call_slot: Arc<()> = Arc::new(());
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
                let trigger = name_trigger_messages.clone();
                async move {
                    handle_inbound_message(&bus, &store, &trigger, &context);
                }
            })
            .on_event_for(
                &[
                    EventKind::Receipt,
                    EventKind::ChatPresence,
                    EventKind::Presence,
                    EventKind::ContactUpdate,
                    EventKind::StarUpdate,
                    // Chat-list app-state patches: archive/mute/pin flags and
                    // the read state must survive restarts and stay in sync
                    // with the phone.
                    EventKind::ArchiveUpdate,
                    EventKind::MuteUpdate,
                    EventKind::PinUpdate,
                    EventKind::MarkChatAsReadUpdate,
                    // Chat and message lifecycle patches the phone pushes
                    // (audit S8): deletes, clears, delete-for-me, push names.
                    EventKind::DeleteChatUpdate,
                    EventKind::ClearChatUpdate,
                    EventKind::DeleteMessageForMeUpdate,
                    EventKind::PushNameUpdate,
                    // Group notifications (subject changes, membership).
                    EventKind::GroupUpdate,
                    // Undecryptable payloads still occupy a timeline slot.
                    EventKind::UndecryptableMessage,
                    // Profile-picture changes and server dirty markers.
                    EventKind::PictureUpdate,
                    EventKind::DirtyState,
                ],
                move |event, _client| {
                    let bus = bus_updates.clone();
                    let store = Arc::clone(&store_updates);
                    let trigger = name_trigger_updates.clone();
                    async move {
                        handle_update_event(&bus, &store, &trigger, event.as_ref());
                    }
                },
            )
            .on_event_for(&[EventKind::HistorySync], move |event, _client| {
                let bus = bus_history.clone();
                let store = Arc::clone(&store_history);
                let trigger = name_trigger_history.clone();
                async move {
                    // Decompression and parsing are CPU-bound: keep them off
                    // the async worker threads.
                    if let Event::HistorySync(sync) = event.as_ref() {
                        let sync = sync.clone();
                        // Fire-and-forget: dropping the handle detaches the
                        // blocking task, which is what we want here.
                        drop(tokio::task::spawn_blocking(move || {
                            import_history_sync(&bus, &store, &sync, &trigger);
                            // The import may have skipped conversations that
                            // history sync never sent; named contacts whose
                            // chat row is still missing get one now.
                            backfill_chats_from_contacts(&store, &bus);
                        }));
                    }
                }
            })
            .on_event_for(
                &[
                    EventKind::IncomingCall,
                    EventKind::MissedCall,
                    EventKind::CallEndedElsewhere,
                ],
                move |event, _client| {
                    let call_slot = Arc::clone(&call_slot);
                    async move {
                        #[cfg(feature = "calls")]
                        {
                            let guard = call_slot.lock().await;
                            if let Some(manager) = guard.as_ref() {
                                manager.handle_event(event.as_ref());
                            }
                        }
                        #[cfg(not(feature = "calls"))]
                        {
                            let _ = (call_slot, event);
                        }
                    }
                },
            )
            .build()
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        #[cfg(feature = "calls")]
        if let Some(media) = self.config.call_media.clone() {
            let manager = crate::calls::manager::CallManager::new(bot.client(), media);
            let mut updates = manager.subscribe();
            let bus = self.events.clone();
            tokio::spawn(async move {
                while let Ok(update) = updates.recv().await {
                    let _ = bus.send(CoreEvent::Call(update));
                }
            });
            *self.calls.lock().await = Some(manager);
        }

        // Chat-list app-state (read/pin/mute/archive) on every connect so unread
        // counts stay in sync with the phone after the first pairing.
        {
            let client = bot.client();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                client
                    .process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                        name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::Regular,
                        full_sync: false,
                    })
                    .await;
            });
        }

        // One-time backfill of the phone's address book. Contacts live in the
        // `critical_unblock_low` app-state collection; when we have never seen
        // a contact, request full syncs so names arrive for the chat list.
        if matches!(self.store.contact_count(), Ok(0)) {
            let client = bot.client();
            let store = Arc::clone(&self.store);
            tokio::spawn(async move {
                // Give the freshly spawned connection a moment to settle.
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                for name in [
                    whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::CriticalUnblockLow,
                    whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::Regular,
                ] {
                    client
                        .process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                            name,
                            full_sync: true,
                        })
                        .await;
                }
                match store.contact_count() {
                    Ok(count) => tracing::info!(count, "address-book backfill finished"),
                    Err(error) => tracing::warn!(%error, "contact count after backfill failed"),
                }
            });
        }

        // Group subjects and contact names used to be resolved by two
        // one-shot, clock-driven, capped passes right after connect, so chats
        // imported after them kept raw JID names forever (audit S6). They now
        // run event-driven: history-sync imports, chat patches and inbound
        // messages all request a run, and this scheduler debounces the
        // requests into a single pass (no busy looping). Only a `Weak`
        // handle to the client is kept, so the loop ends when the session's
        // bot is shut down instead of pinning it alive forever.
        {
            let client = Arc::downgrade(&bot.client());
            let store = Arc::clone(&self.store);
            let bus = self.events.clone();
            let trigger = name_trigger.clone();
            tokio::spawn(async move {
                loop {
                    trigger.notify.notified().await;
                    // Debounce: wait for the burst to settle before hitting
                    // usync / group metadata.
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(NAME_PASS_DEBOUNCE) => break,
                            _ = trigger.notify.notified() => {}
                        }
                    }
                    let Some(client) = client.upgrade() else {
                        break;
                    };
                    run_name_passes(&client, &store, &bus).await;
                    drop(client);
                }
            });
        }
        // Kick a first pass shortly after connect; history imports re-trigger.
        name_trigger.request();

        // Replay the chat-list app state from a full snapshot once per
        // session (audit S1). Incremental syncs never re-deliver patches the
        // phone already acked, which left archive/mute/pin/read flags behind.
        // This used to wait for the first history-sync import, but after
        // initial pairing history sync never runs again — so the snapshot is
        // now requested on a timer after connect (verified against upstream
        // 0.7.0: consumers kick `MajorSyncTask::AppStateSync { full_sync:
        // true }` themselves; the sync worker serializes it per collection).
        {
            let client = Arc::downgrade(&bot.client());
            let gate = self.full_sync.clone();
            let trigger = name_trigger.clone();
            let store = Arc::clone(&self.store);
            let bus = self.events.clone();
            tokio::spawn(async move {
                if !gate.fire() {
                    return;
                }
                tokio::time::sleep(FULL_SYNC_DELAY).await;
                let Some(client) = client.upgrade() else {
                    return;
                };
                client
                    .process_sync_task(whatsapp_rust::sync_task::MajorSyncTask::AppStateSync {
                        name: whatsapp_rust::wacore::appstate::patch_decode::WAPatchName::Regular,
                        full_sync: true,
                    })
                    .await;
                tracing::info!("full regular app-state snapshot finished");
                // The snapshot can surface chats that were deferred or still
                // unnamed; give the name passes a shot.
                trigger.request();
                // And it can reveal chats that never existed locally at all.
                backfill_chats_from_contacts(&store, &bus);
            });
        }

        // Backfill chat rows for named contacts that history sync never
        // imported (audit S4): active direct chats were invisible because
        // their conversation was missing from every history blob. Runs after
        // connect and after every history import (see the HistorySync
        // handler).
        {
            let store = Arc::clone(&self.store);
            let bus = self.events.clone();
            tokio::spawn(async move {
                tokio::time::sleep(CONTACT_BACKFILL_DELAY).await;
                backfill_chats_from_contacts(&store, &bus);
            });
        }

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
    ///
    /// The message is persisted with status `Pending` *before* the protocol
    /// send (sharing a pre-generated message id with the wire send), so a
    /// failed send survives a reload: on success the stored row is upgraded to
    /// `Sent`, on failure it is marked `Failed` and the error is returned.
    pub async fn send_text(&self, chat_id: &Jid, text: &str) -> Result<Message> {
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidInput("message text is empty".into()));
        }

        let client = self.client().await?;
        let to = to_upstream_jid(chat_id)?;

        let sender_id = self
            .own_jid
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(|| Jid::new(ME_PLACEHOLDER));

        // Pre-generate the id so the stored Pending row and the actual send
        // carry the same identity (receipts then find the row).
        let id = client.generate_message_id();

        let mut message = Message {
            id,
            chat_id: chat_id.clone(),
            sender_id,
            from_me: true,
            timestamp: now_unix(),
            kind: MessageKind::Text,
            text: Some(text.to_owned()),
            status: MessageStatus::Pending,
            view_once: false,
        };

        // Chat row first: the message references it.
        self.store
            .set_chat_last_message_meta(chat_id, MessageKind::Text, true)
            .ok();
        self.store.record_message_activity(
            chat_id,
            &preview_for(&message),
            message.timestamp,
            None,
            false,
        )?;
        self.store.upsert_message(&message)?;
        let _ = self.events.send(CoreEvent::Message(message.clone()));

        let sent = client
            .send_message_with_options(
                to,
                wa::Message::text(text),
                whatsapp_rust::send::SendOptions::default().with_message_id(message.id.clone()),
            )
            .await;

        match sent {
            Ok(_) => {
                message.status = MessageStatus::Sent;
                if let Err(error) = self
                    .store
                    .set_message_status(&message.id, MessageStatus::Sent)
                {
                    tracing::warn!(%error, "failed to upgrade sent message status");
                }
                let _ =
                    self.events
                        .send(CoreEvent::MessageStatusChanged(MessageStatusChangedEvent {
                            chat_id: message.chat_id.clone(),
                            message_id: message.id.clone(),
                            status: MessageStatus::Sent,
                        }));
                Ok(message)
            }
            Err(error) => {
                let _ = self
                    .store
                    .set_message_status(&message.id, MessageStatus::Failed);
                let _ =
                    self.events
                        .send(CoreEvent::MessageStatusChanged(MessageStatusChangedEvent {
                            chat_id: message.chat_id.clone(),
                            message_id: message.id.clone(),
                            status: MessageStatus::Failed,
                        }));
                Err(CoreError::Protocol(error.to_string()))
            }
        }
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
        self.store.set_chat_pinned(chat_id, pinned).map(|_| ())
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
        self.store.set_chat_muted(chat_id, muted).map(|_| ())
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
        self.store.set_chat_archived(chat_id, archived).map(|_| ())
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
        self.store.mark_chat_read(chat_id).map(|_| ())
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

    /// Ask the primary phone for a page of older messages in a chat.
    ///
    /// Anchors on the oldest stored message. The reply arrives as a regular
    /// history-sync chunk, which the history handler imports and publishes as
    /// chat updates. Returns `false` when the chat has nothing to anchor on.
    pub async fn fetch_older_history(&self, chat_id: &Jid, count: u32) -> Result<bool> {
        let Some((oldest_id, oldest_from_me, oldest_ts)) = self.store.oldest_message(chat_id)?
        else {
            return Ok(false);
        };

        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        client
            .fetch_message_history(
                &jid,
                &oldest_id,
                oldest_from_me,
                (oldest_ts * 1000) as i64,
                count as i32,
            )
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(true)
    }

    /// Backfill a chat's history from the primary phone via PDO
    /// (HISTORY_SYNC_ON_DEMAND).
    ///
    /// For chats with stored messages this anchors on the oldest one (see
    /// [`WaClient::fetch_older_history`]). For chats with *no* messages at
    /// all — the ones history sync never imported (audit S4) — it sends an
    /// empty anchor (`oldest_msg_id = ""`, timestamp 0): upstream passes the
    /// anchor through verbatim and the phone answers with the newest page of
    /// the conversation. The answer flows back as a self-addressed
    /// history-sync notification (upstream `receive.rs` handles it in
    /// `handle_history_sync`), so it lands in the regular history import and
    /// needs no extra wiring here. Returns `false` when nothing was
    /// requested (not connected).
    pub async fn backfill_chat_history(&self, chat_id: &Jid, count: u32) -> Result<bool> {
        if self.store.oldest_message(chat_id)?.is_some() {
            return self.fetch_older_history(chat_id, count).await;
        }

        let client = self.client().await?;
        let jid = to_upstream_jid(chat_id)?;
        client
            .fetch_message_history(&jid, "", false, 0, count as i32)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        tracing::info!(chat = %chat_id, count, "requested on-demand history backfill");
        Ok(true)
    }

    /// Subscribe to presence (online / last seen) updates for a contact.
    pub async fn subscribe_presence(&self, jid: &Jid) -> Result<()> {
        let client = self.client().await?;
        client
            .presence()
            .subscribe(to_upstream_jid(jid)?)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Stop receiving presence updates for a contact.
    pub async fn unsubscribe_presence(&self, jid: &Jid) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream_jid(jid)?;
        client
            .presence()
            .unsubscribe(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
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

/// Run both name-resolution passes (group subjects, then contact names).
///
/// Event-driven since audit S6; each pass no-ops when nothing needs a name.
async fn run_name_passes(
    client: &whatsapp_rust::Client,
    store: &Store,
    bus: &broadcast::Sender<CoreEvent>,
) {
    run_group_name_pass(client, store, bus).await;
    run_contact_name_pass(client, store, bus).await;
}

/// Fetch subjects for group chats whose name is still a placeholder or was
/// reset by the migration backfill, politely spaced.
async fn run_group_name_pass(
    client: &whatsapp_rust::Client,
    store: &Store,
    bus: &broadcast::Sender<CoreEvent>,
) {
    let targets = match store.chats_needing_group_names(GROUP_NAME_PASS_CAP) {
        Ok(targets) => targets,
        Err(error) => {
            tracing::warn!(%error, "group name pass: listing chats failed");
            return;
        }
    };
    if targets.is_empty() {
        return;
    }
    let mut resolved = 0usize;
    let mut failed = 0usize;
    for chat_id in targets {
        let Ok(upstream) = to_upstream_jid(&chat_id) else {
            failed += 1;
            continue;
        };
        match client.groups().get_metadata(&upstream).await {
            Ok(metadata) => {
                let subject = metadata.subject.trim().to_owned();
                if !subject.is_empty() && matches!(store.rename_chat(&chat_id, &subject), Ok(true))
                {
                    resolved += 1;
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent {
                        chat_id: chat_id.clone(),
                    }));
                }
            }
            Err(error) => {
                failed += 1;
                tracing::debug!(%error, chat = %chat_id, "group metadata fetch failed");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    tracing::info!(
        resolved,
        failed,
        "group name pass finished ({} pending)",
        resolved + failed
    );
}

/// Resolve direct chats that still show a phone/LID number through usync, so
/// LID rows can be merged onto the named PN chat.
async fn run_contact_name_pass(
    client: &whatsapp_rust::Client,
    store: &Store,
    bus: &broadcast::Sender<CoreEvent>,
) {
    let targets = match store.chats_needing_contact_names(CONTACT_NAME_PASS_CAP) {
        Ok(targets) => targets,
        Err(error) => {
            tracing::warn!(%error, "contact name pass: listing chats failed");
            return;
        }
    };
    if targets.is_empty() {
        return;
    }
    let mut merged = 0usize;
    let mut named = 0usize;
    for chunk in targets.chunks(15) {
        match client
            .contacts()
            .get_user_info(
                &chunk
                    .iter()
                    .filter_map(|jid| to_upstream_jid(jid).ok())
                    .collect::<Vec<_>>(),
            )
            .await
        {
            Ok(info) => {
                for (upstream, user) in info {
                    let jid = from_upstream_jid(&upstream);
                    if let Some(lid) = user.lid.as_ref() {
                        match store.link_jids(&jid, &from_upstream_jid(lid)) {
                            Ok(Some(canonical)) => {
                                merged += 1;
                                let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent {
                                    chat_id: canonical,
                                }));
                            }
                            Ok(None) => {}
                            Err(error) => {
                                tracing::debug!(%error, chat = %jid, "contact name pass: link failed")
                            }
                        }
                    }
                    if let Some(name) = user.verified_name.and_then(|v| v.name) {
                        let trimmed = name.trim();
                        if !trimmed.is_empty()
                            && matches!(store.apply_contact_name(&jid, trimmed), Ok(true))
                        {
                            named += 1;
                            let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent {
                                chat_id: jid.clone(),
                            }));
                        }
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%error, "contact name pass: usync failed");
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    tracing::info!(merged, named, "contact name pass finished");
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
    trigger: &NamePassTrigger,
    context: &MessageContext,
) {
    let info = &context.info;

    // Protocol chatter (key distribution, receipts, revokes, edits) and
    // reactions carry their payloads inside a message envelope; they must not
    // be persisted as chat messages.
    if let Some(protocol) = context.message.protocol_message.as_option() {
        handle_protocol_message(bus, store, info, protocol);
        return;
    }
    if let Some(reaction) = context.message.reaction_message.as_option() {
        handle_reaction(bus, store, info, reaction);
        return;
    }

    // In-chat call-log items are not chat bubbles; persist them as call-log
    // rows and stop before the generic message path.
    if let Some(mut entry) = crate::calls::call_log_entry_from_message_with_direction(
        &context.message,
        info.source.is_from_me,
    ) {
        entry.started_at_unix = Some(timestamp_to_unix(&info.timestamp) as i64);
        if let Err(error) = crate::calls::import_call_log_into(store, &[entry]) {
            tracing::warn!(%error, "failed to store live call-log message");
        }
        return;
    }

    let message = Message {
        id: info.id.to_string(),
        chat_id: store
            .canonical_jid(&from_upstream_jid(&info.source.chat))
            .unwrap_or_else(|_| from_upstream_jid(&info.source.chat)),
        sender_id: from_upstream_jid(&info.source.sender),
        from_me: info.source.is_from_me,
        timestamp: timestamp_to_unix(&info.timestamp),
        kind: classify(&context.message),
        text: context
            .message
            .get_caption()
            .or_else(|| context.message.text_content())
            .map(str::to_owned),
        status: if info.source.is_from_me {
            MessageStatus::Sent
        } else {
            MessageStatus::Delivered
        },
        view_once: crate::media::is_view_once(&context.message),
    };

    // A push name is only a valid name hint for direct chats: applying it to
    // a group row permanently renames the group to whoever spoke last (audit
    // S3). The store ignores hints for group rows too; this guard keeps the
    // intent visible at the call site like the history path does.
    let name_hint = (!info.push_name.trim().is_empty() && !message.chat_id.is_group())
        .then_some(info.push_name.as_str());
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
    if let Err(error) =
        store.set_chat_last_message_meta(&message.chat_id, message.kind, message.from_me)
    {
        tracing::warn!(%error, "failed to update chat preview metadata");
    }
    if let Err(error) = store.upsert_message(&message) {
        tracing::warn!(%error, "failed to store inbound message");
    } else if let Err(error) = store.drain_pending_reactions(&message.id) {
        tracing::warn!(%error, "failed to apply reactions buffered for the message");
    }
    if let Err(error) = store.set_raw_proto(&message.id, &context.message.encode_to_vec()) {
        tracing::warn!(%error, "failed to store raw protobuf for inbound message");
    }

    // Status updates arrive as normal messages on `status@broadcast`; keep a
    // compact row for the Status screen and ignore non-status messages.
    if let Err(error) = crate::statuses::import_status_message(
        store,
        &message,
        Some(&context.message.encode_to_vec()),
    ) {
        tracing::warn!(%error, "failed to store status update");
    }

    let chat_id = message.chat_id.clone();
    let _ = bus.send(CoreEvent::Message(message));
    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
    // Live traffic can create placeholder chat rows; the name passes pick
    // them up on the next debounced run.
    trigger.request();
}

/// Revokes and edits arrive as protocol messages; apply them to the store and
/// tell the UI. Other protocol types (key shares, notification syncs) are
/// intentionally ignored.
fn handle_protocol_message(
    bus: &broadcast::Sender<CoreEvent>,
    store: &Store,
    info: &whatsapp_rust::types::message::MessageInfo,
    protocol: &wa::message::ProtocolMessage,
) {
    use wa::message::protocol_message::Type;

    let Some(key) = protocol.key.as_option() else {
        return;
    };
    let Some(message_id) = key.id.as_deref() else {
        return;
    };
    let chat_id = key
        .remote_jid
        .as_deref()
        .map(Jid::new)
        .unwrap_or_else(|| from_upstream_jid(&info.source.chat));

    match protocol.r#type {
        Some(Type::REVOKE) => {
            if let Err(error) = store.mark_message_revoked(message_id) {
                tracing::warn!(%error, "failed to tombstone revoked message");
            }
            let _ = bus.send(CoreEvent::MessageRevoked(MessageRevokedEvent {
                chat_id,
                message_id: message_id.to_owned(),
            }));
        }
        Some(Type::MESSAGE_EDIT) => {
            let text = protocol
                .edited_message
                .as_option()
                .and_then(|edited| edited.text_content())
                .map(str::to_owned);
            let Some(text) = text else {
                return;
            };
            if let Err(error) = store.update_message_text(message_id, &text) {
                tracing::warn!(%error, "failed to store edited message");
            }
            let _ = bus.send(CoreEvent::MessageEdited(MessageEditedEvent {
                chat_id,
                message_id: message_id.to_owned(),
                text,
            }));
        }
        _ => {}
    }
}

/// Reactions ride on their own envelope; persist the (message, reactor, emoji)
/// triple and let the UI update the bubble. When the target message row does
/// not exist yet, the reaction is buffered in `pending_reactions` and drained
/// once the message arrives.
fn handle_reaction(
    bus: &broadcast::Sender<CoreEvent>,
    store: &Store,
    info: &whatsapp_rust::types::message::MessageInfo,
    reaction: &wa::message::ReactionMessage,
) {
    let Some(key) = reaction.key.as_option() else {
        return;
    };
    let Some(message_id) = key.id.as_deref() else {
        return;
    };
    let chat_id = key
        .remote_jid
        .as_deref()
        .map(Jid::new)
        .unwrap_or_else(|| from_upstream_jid(&info.source.chat));
    let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
    let reactor = from_upstream_jid(&info.source.sender);
    let emoji = reaction.text.as_deref().unwrap_or("");
    let timestamp = timestamp_to_unix(&info.timestamp);

    // A reaction can beat its message (live traffic does not wait for the
    // history-sync import). Inserting it directly would fail on the foreign
    // key and lose it forever, so buffer it until the message row arrives.
    let known = match store.find_message(message_id) {
        Ok(message) => message.is_some(),
        Err(error) => {
            tracing::warn!(%error, "failed to look up reaction target");
            false
        }
    };
    if known {
        if let Err(error) = store.upsert_reaction(message_id, &reactor, emoji) {
            tracing::warn!(%error, "failed to store reaction");
        }
    } else if let Err(error) =
        store.buffer_pending_reaction(&chat_id, message_id, &reactor, emoji, timestamp)
    {
        tracing::warn!(%error, "failed to buffer reaction for a missing message");
    }
    let _ = bus.send(CoreEvent::Reaction(ReactionEvent {
        chat_id,
        message_id: message_id.to_owned(),
        reactor,
        emoji: emoji.to_owned(),
    }));
}

/// Receipts, typing and presence: publish and, for receipts, persist.
fn handle_update_event(
    bus: &broadcast::Sender<CoreEvent>,
    store: &Store,
    trigger: &NamePassTrigger,
    event: &Event,
) {
    match event {
        Event::Receipt(receipt) => {
            let Some(status) = receipt_status(&receipt.r#type) else {
                return;
            };
            // Canonicalize like every other handler: a LID-addressed receipt
            // must land on the chat row the list actually shows.
            let chat_id = from_upstream_jid(&receipt.source.chat);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
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
        Event::StarUpdate(update) => {
            let starred = update.action.starred.unwrap_or(false);
            if let Err(error) = store.set_message_starred(&update.message_id, starred) {
                tracing::warn!(%error, "failed to store star update");
            }
        }
        // App-state chat patches. Each one updates the local chat row and
        // notifies the UI so the list re-renders immediately. The update JID
        // is canonicalized first so alias rows (LID ↔ phone number) patch the
        // chat the list actually shows; a missing row is created on the fly
        // (deferred) so patches that beat history sync are not lost.
        Event::ArchiveUpdate(update) => {
            let Some(archived) = update.action.archived else {
                return;
            };
            let chat_id = from_upstream_jid(&update.jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            match store.set_chat_archived(&chat_id, archived) {
                Ok(deferred) => {
                    if deferred {
                        tracing::warn!(
                            chat = %chat_id,
                            "archive patch created a deferred chat row (history not synced yet)"
                        );
                        // A deferred row is a new placeholder row; the name
                        // passes should pick it up.
                        trigger.request();
                    }
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
                }
                Err(error) => tracing::warn!(%error, "failed to store archive update"),
            }
        }
        Event::MuteUpdate(update) => {
            let Some(muted) = update.action.muted else {
                return;
            };
            let chat_id = from_upstream_jid(&update.jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            match store.set_chat_muted(&chat_id, muted) {
                Ok(deferred) => {
                    if deferred {
                        tracing::warn!(
                            chat = %chat_id,
                            "mute patch created a deferred chat row (history not synced yet)"
                        );
                        trigger.request();
                    }
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
                }
                Err(error) => tracing::warn!(%error, "failed to store mute update"),
            }
        }
        Event::PinUpdate(update) => {
            let Some(pinned) = update.action.pinned else {
                return;
            };
            let chat_id = from_upstream_jid(&update.jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            match store.set_chat_pinned(&chat_id, pinned) {
                Ok(deferred) => {
                    if deferred {
                        tracing::warn!(
                            chat = %chat_id,
                            "pin patch created a deferred chat row (history not synced yet)"
                        );
                        trigger.request();
                    }
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
                }
                Err(error) => tracing::warn!(%error, "failed to store pin update"),
            }
        }
        Event::MarkChatAsReadUpdate(update) => {
            // `read` is absent when the patch only trims a message range;
            // treat that as "marked read" the way the official clients do.
            // `read = false` means the phone marked the chat unread.
            let read = update.action.read.unwrap_or(true);
            let chat_id = from_upstream_jid(&update.jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            let result = if read {
                store.mark_chat_read(&chat_id)
            } else {
                store.mark_chat_unread(&chat_id)
            };
            match result {
                Ok(deferred) => {
                    if deferred {
                        tracing::warn!(
                            chat = %chat_id,
                            "read patch created a deferred chat row (history not synced yet)"
                        );
                        trigger.request();
                    }
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
                }
                Err(error) => tracing::warn!(%error, "failed to store read update"),
            }
        }
        Event::ContactUpdate(update) => {
            let action = &update.action;
            let name = action
                .full_name
                .as_deref()
                .or(action.first_name.as_deref())
                .map(str::trim)
                .filter(|name| !name.is_empty());
            let Some(name) = name else {
                return;
            };

            // The action usually carries the PN and/or LID the name belongs
            // to; apply it to every identity we learn about.
            let mut targets: Vec<Jid> = vec![from_upstream_jid(&update.jid)];
            for candidate in [action.lid_jid.as_deref(), action.pn_jid.as_deref()]
                .into_iter()
                .flatten()
            {
                let jid = Jid::new(candidate);
                if !targets.contains(&jid) {
                    targets.push(jid);
                }
            }

            if targets.len() >= 2 {
                for pair in targets.windows(2) {
                    if let Err(error) = store.link_jids(&pair[0], &pair[1]) {
                        tracing::debug!(%error, "failed to link contact JIDs");
                    }
                }
            }

            for jid in targets {
                match store.apply_contact_name(&jid, name) {
                    Ok(true) => {
                        let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id: jid }));
                    }
                    Ok(false) => {}
                    Err(error) => {
                        tracing::warn!(%error, contact = %jid, "failed to store contact name");
                    }
                }
            }
        }
        Event::DeleteChatUpdate(update) => {
            let chat_id = from_upstream_jid(&update.jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            match store.delete_chat(&chat_id) {
                Ok(true) => {
                    let _ = bus.send(CoreEvent::ChatRemoved(ChatRemovedEvent {
                        chat_id: chat_id.clone(),
                    }));
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "failed to delete chat"),
            }
        }
        Event::ClearChatUpdate(update) => {
            let chat_id = from_upstream_jid(&update.jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            match store.clear_chat(&chat_id) {
                Ok(true) => {
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "failed to clear chat"),
            }
        }
        Event::DeleteMessageForMeUpdate(update) => {
            let chat_id = from_upstream_jid(&update.chat_jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            if let Err(error) = store.delete_message(&update.message_id) {
                tracing::warn!(%error, "failed to delete message for me");
            }
            // The chat-list preview may have pointed at the removed message;
            // recompute it from what is left (or blank it when the chat is
            // now empty).
            match store.list_messages(&chat_id, 1) {
                Ok(messages) => {
                    if let Some(newest) = messages.first() {
                        let _ = store.record_message_activity(
                            &chat_id,
                            &preview_for(newest),
                            newest.timestamp,
                            None,
                            false,
                        );
                    } else {
                        let _ = store.clear_chat(&chat_id);
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to refresh chat preview"),
            }
            let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
        }
        Event::PushNameUpdate(update) => {
            // A push name is only a valid display name for direct chats, and
            // only when nothing better (contact name, subject) is known.
            let jid = from_upstream_jid(&update.jid);
            match store.apply_push_name(&jid, &update.new_push_name) {
                Ok(true) => {
                    let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id: jid }));
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, contact = %jid, "failed to store push name"),
            }
        }
        Event::GroupUpdate(update) => {
            let chat_id = from_upstream_jid(&update.group_jid);
            let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);
            if let whatsapp_rust::wacore::stanza::groups::GroupNotificationAction::Subject {
                subject,
                ..
            } = &update.action
            {
                let subject = subject.trim();
                if !subject.is_empty() {
                    match store.rename_chat(&chat_id, subject) {
                        Ok(true) => {
                            let _ =
                                bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
                        }
                        Ok(false) => {}
                        Err(error) => {
                            tracing::warn!(%error, "failed to store group subject")
                        }
                    }
                }
            }
            // Participant and setting changes carry no stored state of their
            // own yet; a join/leave shows up when its system notice message
            // arrives through the regular message path. Nothing to crash on.
        }
        Event::UndecryptableMessage(update) => {
            // The payload will never render, but it occupies a slot in the
            // timeline and the unread count; storing a placeholder keeps the
            // list honest instead of silently swallowing the message.
            let info = &update.info;
            let chat_id = store
                .canonical_jid(&from_upstream_jid(&info.source.chat))
                .unwrap_or_else(|_| from_upstream_jid(&info.source.chat));
            let message = Message {
                id: info.id.to_string(),
                chat_id: chat_id.clone(),
                sender_id: from_upstream_jid(&info.source.sender),
                from_me: info.source.is_from_me,
                timestamp: timestamp_to_unix(&info.timestamp),
                kind: MessageKind::Unsupported,
                text: None,
                status: if info.source.is_from_me {
                    MessageStatus::Sent
                } else {
                    MessageStatus::Delivered
                },
                view_once: false,
            };
            let known = matches!(store.find_message(&message.id), Ok(Some(_)));
            if !known
                && let Err(error) = store.upsert_message(&message)
            {
                tracing::warn!(%error, "failed to store undecryptable placeholder");
            }
            if let Err(error) = store.record_message_activity(
                &chat_id,
                &preview_for(&message),
                message.timestamp,
                None,
                !message.from_me,
            ) {
                tracing::warn!(%error, "failed to update chat activity for placeholder");
            }
            if !known {
                let _ = bus.send(CoreEvent::Message(message));
            }
            let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
        }
        Event::PictureUpdate(update) => {
            // The event only carries the JID and the server picture id — no
            // URL or path — so there is nothing to persist beyond a log line.
            // Avatar fetching stays on-demand via `avatar_url`.
            tracing::debug!(
                jid = %update.jid,
                removed = update.removed,
                picture_id = update.picture_id.as_deref().unwrap_or(""),
                "picture update received"
            );
        }
        Event::DirtyState(update) => {
            // The upstream client performs its own clean/resync work; the
            // missing-flag repair here rides on the periodic full `regular`
            // snapshot instead.
            tracing::warn!(
                dirty_type = ?update.dirty_type,
                "server reported dirty app state; relying on the full regular snapshot to repair"
            );
        }
        _ => {}
    }
}

/// Import a history-sync blob: conversations and their messages.
///
/// Runs on a blocking thread (the caller uses `spawn_blocking`); the stream
/// yields conversations lazily so memory stays bounded even for large blobs.
///
/// One bad conversation must never void the rest of the blob: the upstream
/// stream already skips undecodable conversations (counted in
/// `skipped_conversations()`), and store-level failures per conversation are
/// logged and skipped. Only a fatal stream error (truncation, zlib failure)
/// stops the import — the stream cannot recover from those, so they are
/// logged *and* surfaced as a `CoreEvent::Error` instead of being lost
/// silently.
fn import_history_sync(
    bus: &broadcast::Sender<CoreEvent>,
    store: &Store,
    sync: &LazyHistorySync,
    trigger: &NamePassTrigger,
) {
    let mut stream = sync.stream();
    let mut chat_count = 0usize;
    let mut message_count = 0usize;
    let mut aborted = false;

    loop {
        match stream.next_conversation() {
            Ok(Some(conversation)) => {
                let chat_id = Jid::new(conversation.id.clone());
                if chat_id.as_str().is_empty() {
                    continue;
                }
                let chat_id = store.canonical_jid(&chat_id).unwrap_or(chat_id);

                let history_name = conversation.name.clone().unwrap_or_default();
                let name_is_real = !history_name.trim().is_empty();
                let fallback_name = chat_id.user().to_owned();
                let name = if name_is_real {
                    history_name
                } else {
                    fallback_name.clone()
                };

                // The server's unread counter is the source of truth. Keep
                // whether the chunk carried one: chunks without a counter
                // (older-message backfills) must not zero the local value.
                let server_unread = conversation.unread_count;

                let summary = ChatSummary {
                    id: chat_id.clone(),
                    name,
                    last_message_preview: None,
                    last_activity_ts: normalize_timestamp(
                        conversation.last_msg_timestamp.unwrap_or(0),
                    ),
                    unread_count: server_unread.unwrap_or(0),
                    muted: false,
                    pinned: false,
                    is_group: chat_id.is_group(),
                    is_archived: conversation.archived.unwrap_or(false),
                    last_message_kind: None,
                    last_from_me: false,
                    last_status: None,
                };
                if let Err(error) = store.upsert_chat_from_history(
                    &summary,
                    name_is_real,
                    &fallback_name,
                    server_unread,
                ) {
                    tracing::warn!(%error, chat = %chat_id, "history: chat upsert failed");
                    continue;
                }
                chat_count += 1;

                let mut newest: Option<Message> = None;
                let mut best_name: Option<String> = None;
                for entry in &conversation.messages {
                    let Some(info) = entry.message.as_option() else {
                        continue;
                    };
                    let Some(mut message) = convert_history_message(info) else {
                        continue;
                    };
                    message.chat_id = chat_id.clone();
                    if !message.from_me
                        && !chat_id.is_group()
                        && let Some(push) = info.push_name.as_deref()
                        && !push.trim().is_empty()
                    {
                        best_name = Some(push.trim().to_owned());
                    }
                    if let Err(error) = store.upsert_message(&message) {
                        tracing::warn!(%error, "history: message upsert failed");
                        continue;
                    }
                    // The message may resolve reactions that arrived before
                    // its row existed.
                    if let Err(error) = store.drain_pending_reactions(&message.id) {
                        tracing::warn!(%error, "history: buffered reaction drain failed");
                    }
                    if let Some(raw) = info.message.as_option()
                        && let Err(error) = store.set_raw_proto(&message.id, &raw.encode_to_vec())
                    {
                        tracing::warn!(%error, "history: raw protobuf store failed");
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
                        best_name.as_deref(),
                        false,
                    );
                }
                let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
            }
            Ok(None) => break,
            Err(error) => {
                // Fatal: the stream is truncated or the zlib window failed and
                // cannot resume at the next conversation. Log it and tell the
                // UI the import aborted so the loss is not silent.
                tracing::warn!(%error, "history: stream failed, aborting import");
                let _ = bus.send(CoreEvent::Error(ErrorEvent {
                    code: "historySyncImportFailed".to_owned(),
                    message: format!("message history import aborted: {error}"),
                }));
                aborted = true;
                break;
            }
        }
    }

    // Call-log records live in the non-conversation remainder of the blob.
    // Skip it after a fatal stream error: the walker cannot resume either.
    // (`remainder()` consumes the stream, so read the skip counter first.)
    let skipped = stream.skipped_conversations();
    let call_count = if aborted {
        0
    } else {
        match stream.remainder() {
            Ok(rest) => {
                let entries = crate::calls::call_log_entries(&rest);
                if let Err(error) = crate::calls::import_call_log_into(store, &entries) {
                    tracing::warn!(%error, "history: call-log import failed");
                }
                entries.len()
            }
            Err(error) => {
                tracing::warn!(%error, "history: call-log remainder failed");
                0
            }
        }
    };

    if skipped > 0 {
        tracing::warn!(
            skipped,
            "history: conversations skipped (undecodable entries)"
        );
    }
    tracing::info!(
        chats = chat_count,
        messages = message_count,
        calls = call_count,
        skipped,
        "history sync imported"
    );

    // Newly imported group-LID rows can duplicate an existing @g.us chat;
    // merge the twins before the name passes look at them (audit S7).
    match store.merge_group_lid_twins() {
        Ok(merged) if !merged.is_empty() => {
            tracing::info!(count = merged.len(), "history: merged group-LID twins");
            for chat_id in merged {
                let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id }));
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "history: group-LID twin merge failed"),
    }

    // The import may have created placeholder rows; the name passes pick
    // them up on the next debounced run.
    trigger.request();
}

/// Create minimal chat rows for named contacts that have none (audit S4).
///
/// History sync never re-runs after the initial pairing, so conversations
/// missing from its blobs stay invisible even when the address book knows
/// them. This inserts a `name_source = 'contact'` row per affected contact
/// and tells the UI so the list refreshes; existing rows are never touched.
fn backfill_chats_from_contacts(store: &Store, bus: &broadcast::Sender<CoreEvent>) {
    let candidates = match store.contacts_missing_chats(CONTACT_NAME_PASS_CAP) {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::warn!(%error, "contact backfill: listing contacts failed");
            return;
        }
    };
    if candidates.is_empty() {
        return;
    }
    let mut created = 0usize;
    for (jid, name) in candidates {
        match store.upsert_chat_from_contact(&jid, &name) {
            Ok(true) => {
                created += 1;
                let _ = bus.send(CoreEvent::ChatUpdated(ChatUpdatedEvent { chat_id: jid }));
            }
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, chat = %jid, "contact backfill: insert failed"),
        }
    }
    if created > 0 {
        tracing::info!(created, "contact backfill created missing chat rows");
    }
}

/// WhatsApp history timestamps are usually milliseconds, but some server
/// entries carry seconds. Normalize both to Unix seconds.
fn normalize_timestamp(raw: u64) -> u64 {
    const MILLIS_THRESHOLD: u64 = 10_000_000_000; // year 2286 in seconds
    if raw > MILLIS_THRESHOLD {
        raw / 1_000
    } else {
        raw
    }
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
        timestamp: normalize_timestamp(info.message_timestamp.unwrap_or(0)),
        kind: body.map(classify).unwrap_or(MessageKind::Unsupported),
        text: body.and_then(|message| {
            message
                .get_caption()
                .or_else(|| message.text_content())
                .map(str::to_owned)
        }),
        status: if from_me {
            MessageStatus::Sent
        } else {
            MessageStatus::Delivered
        },
        view_once: body.map(crate::media::is_view_once).unwrap_or(false),
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
    // Protocol chatter (key distribution, receipts carried in message rows,
    // reactions) is not user content and must not render as a bubble.
    if message.protocol_message.is_set()
        || message.reaction_message.is_set()
        || message.sender_key_distribution_message.is_set()
        || message.device_sent_message.is_set()
    {
        return MessageKind::System;
    }

    // View-once/ephemeral/edited wrappers hide the real payload one level
    // down; classification must look at the inner message.
    let base = message.get_base_message();
    if base.conversation.is_some() || base.extended_text_message.is_set() {
        return MessageKind::Text;
    }
    if base.image_message.is_set() {
        return MessageKind::Image;
    }
    if base.video_message.is_set() || base.ptv_message.is_set() {
        return MessageKind::Video;
    }
    if let Some(audio) = base.audio_message.as_option() {
        return if audio.ptt.unwrap_or(false) {
            MessageKind::VoiceNote
        } else {
            MessageKind::Audio
        };
    }
    if base.document_message.is_set() {
        return MessageKind::Document;
    }
    if base.sticker_message.is_set() {
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
        // Protocol chatter is not user content and must never become the
        // chat-list preview.
        MessageKind::System => String::new(),
        _ => "[Message]".to_owned(),
    };
    text.chars().take(MAX).collect()
}

/// Delete the stored sync version for one app-state collection so the next
/// sync re-downloads every patch. Used once to backfill contact names.
fn reset_app_state_collection(data_dir: &std::path::Path, collection: &str) -> Result<()> {
    let connection = rusqlite::Connection::open(data_dir.join("session.db"))
        .map_err(|error| CoreError::Storage(error.to_string()))?;
    // The table only exists after the backend has initialised once.
    let deleted = connection
        .execute(
            "DELETE FROM app_state_versions WHERE name = ?1",
            rusqlite::params![collection],
        )
        .map_err(|error| CoreError::Storage(error.to_string()))?;
    tracing::info!(
        collection,
        deleted,
        "app-state version cleared for backfill"
    );
    Ok(())
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
            view_once: false,
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
    fn classifies_protocol_messages_as_system() {
        let message = wa::Message {
            protocol_message: MessageField::some(wa::message::ProtocolMessage::default()),
            ..Default::default()
        };
        assert_eq!(classify(&message), MessageKind::System);
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

    #[test]
    fn normalizes_history_timestamps() {
        // Seconds stay seconds; milliseconds convert down.
        assert_eq!(normalize_timestamp(1_700_000_000), 1_700_000_000);
        assert_eq!(normalize_timestamp(1_700_000_000_000), 1_700_000_000);
        assert_eq!(normalize_timestamp(0), 0);
    }
}
