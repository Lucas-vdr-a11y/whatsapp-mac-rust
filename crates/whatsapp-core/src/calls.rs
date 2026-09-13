//! Calls: signalling, media and call-log surface (milestone M7).
//!
//! Upstream `whatsapp-rust` ships the whole VoIP stack behind its `voip*`
//! cargo features. `CallManager` is our adapter and is compiled only with
//! `--features calls` (which enables `whatsapp-rust/voip-mlow`); the default
//! build stays free of the media stack. The verified integration plan, the
//! feature-by-feature feasibility matrix, the upstream conformance caveats and
//! the exact `client.rs`/`events.rs` wiring still required live in
//! `docs/research/calls-plan.md`.
//!
//! What this module contains today:
//!
//! - [`CallLogEntry`] plus parsers for the call-history records carried by
//!   `HistorySync` chunks and by `CallLogMessage` chat items. This part needs
//!   no VoIP feature and compiles in the default build.
//! - [`WaClient::start_call`] / [`WaClient::end_call`] stubs that report the
//!   missing wiring instead of silently failing.
//! - Behind `--features calls`, inside the feature-gated `manager` module:
//!   `CallManager`, the real upstream-call adapter (outgoing 1:1, accept,
//!   reject, hang up, mute, incoming/missed/ended-elsewhere event mapping). It
//!   compiles against the real upstream API, but [`WaClient`] cannot own it yet
//!   because its fields are private to `client.rs`; see the plan for the one
//!   wiring commit that makes it reachable.

use serde::Serialize;
use whatsapp_rust::waproto::whatsapp as wa;

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// Normalized outcome of a call-log record.
///
/// Mirrors the proto `CallLogRecord.CallResult` enum. `Unknown` also covers
/// records that omitted the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CallLogResult {
    /// The call was answered and connected.
    Connected,
    /// Nobody answered before the call ended.
    Missed,
    /// Declined by the callee.
    Rejected,
    /// Cancelled by the caller.
    Cancelled,
    /// Answered on another linked device.
    AcceptedElsewhere,
    /// The call failed to establish.
    Failed,
    /// The callee was unavailable.
    Unavailable,
    /// A scheduled call that has not happened yet.
    Upcoming,
    /// The caller abandoned the attempt before it rang out.
    Abandoned,
    /// The call is still in progress.
    Ongoing,
    /// The record was malformed.
    Invalid,
    /// No result field was present, or it held a value this client does not know.
    Unknown,
}

/// One call-history entry, normalized for the UI.
///
/// Produced from `HistorySync.callLogRecords` chunks and from
/// `Message.callLogMesssage` items in existing chats. Serialized over IPC, so
/// additions are backwards-compatible only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallLogEntry {
    /// Call id carried by the server record, when present.
    pub call_id: Option<String>,
    /// Participant JIDs exactly as the record sent them (raw strings kept
    /// verbatim because call logs may reference LID or PN addressing).
    pub participants: Vec<Jid>,
    /// Group JID for group calls.
    pub group_jid: Option<Jid>,
    /// Who created the call (group calls and call links).
    pub call_creator: Option<Jid>,
    /// True for incoming calls. `None` when the source does not carry direction
    /// (individual `CallLogMessage` items).
    pub incoming: Option<bool>,
    /// True for video calls.
    pub video: bool,
    /// True when the call was joined or created through a call link.
    pub call_link: bool,
    /// Terminal outcome.
    pub result: CallLogResult,
    /// Talk duration in seconds, when reported.
    pub duration_secs: Option<u64>,
    /// Start time in unix seconds, when reported.
    pub started_at_unix: Option<i64>,
}

/// Extract every call-log record from a decoded `HistorySync` chunk.
///
/// `whatsapp-rust` delivers history sync as an opaque compressed blob
/// (`LazyHistorySync`); this is the supported way to read the call-log field
/// once the blob has been decoded into `wa::HistorySync`.
pub fn call_log_entries(history_sync: &wa::HistorySync) -> Vec<CallLogEntry> {
    history_sync
        .call_log_records
        .iter()
        .map(call_log_entry_from_record)
        .collect()
}

/// Normalize one proto `CallLogRecord`.
pub fn call_log_entry_from_record(record: &wa::CallLogRecord) -> CallLogEntry {
    CallLogEntry {
        call_id: non_empty(record.call_id.as_deref()),
        participants: record
            .participants
            .iter()
            .filter_map(|participant| jid_from_string(participant.user_jid.as_deref()))
            .collect(),
        group_jid: jid_from_string(record.group_jid.as_deref()),
        call_creator: jid_from_string(record.call_creator_jid.as_deref()),
        incoming: record.is_incoming,
        video: record.is_video.unwrap_or(false),
        call_link: record.is_call_link.unwrap_or(false) || record.call_link_token.is_some(),
        result: record_result(record.call_result),
        duration_secs: record.duration.and_then(|value| u64::try_from(value).ok()),
        started_at_unix: record.start_time,
    }
}

/// Read the in-chat call-log item (`Message.callLogMesssage`) from an inbound
/// message, if it has one.
pub fn call_log_entry_from_message(message: &wa::Message) -> Option<CallLogEntry> {
    let log = message.call_log_messsage.as_option()?;
    Some(CallLogEntry {
        call_id: None,
        participants: log
            .participants
            .iter()
            .filter_map(|participant| jid_from_string(participant.jid.as_deref()))
            .collect(),
        group_jid: None,
        call_creator: None,
        incoming: None,
        video: log.is_video.unwrap_or(false),
        call_link: false,
        result: message_outcome(log.call_outcome),
        duration_secs: log
            .duration_secs
            .and_then(|value| u64::try_from(value).ok()),
        started_at_unix: None,
    })
}

fn record_result(value: Option<wa::call_log_record::CallResult>) -> CallLogResult {
    match value {
        Some(wa::call_log_record::CallResult::CONNECTED) => CallLogResult::Connected,
        Some(wa::call_log_record::CallResult::MISSED) => CallLogResult::Missed,
        Some(wa::call_log_record::CallResult::REJECTED) => CallLogResult::Rejected,
        Some(wa::call_log_record::CallResult::CANCELLED) => CallLogResult::Cancelled,
        Some(wa::call_log_record::CallResult::ACCEPTEDELSEWHERE) => {
            CallLogResult::AcceptedElsewhere
        }
        Some(wa::call_log_record::CallResult::FAILED) => CallLogResult::Failed,
        Some(wa::call_log_record::CallResult::UNAVAILABLE) => CallLogResult::Unavailable,
        Some(wa::call_log_record::CallResult::UPCOMING) => CallLogResult::Upcoming,
        Some(wa::call_log_record::CallResult::ABANDONED) => CallLogResult::Abandoned,
        Some(wa::call_log_record::CallResult::ONGOING) => CallLogResult::Ongoing,
        Some(wa::call_log_record::CallResult::INVALID) => CallLogResult::Invalid,
        _ => CallLogResult::Unknown,
    }
}

fn message_outcome(value: Option<wa::message::call_log_message::CallOutcome>) -> CallLogResult {
    use wa::message::call_log_message::CallOutcome;
    match value {
        Some(CallOutcome::CONNECTED) => CallLogResult::Connected,
        Some(CallOutcome::MISSED) => CallLogResult::Missed,
        Some(CallOutcome::FAILED) => CallLogResult::Failed,
        Some(CallOutcome::REJECTED) => CallLogResult::Rejected,
        Some(CallOutcome::ACCEPTED_ELSEWHERE) => CallLogResult::AcceptedElsewhere,
        Some(CallOutcome::ONGOING) => CallLogResult::Ongoing,
        Some(CallOutcome::SILENCED_BY_DND) | Some(CallOutcome::SILENCED_UNKNOWN_CALLER) => {
            CallLogResult::Missed
        }
        _ => CallLogResult::Unknown,
    }
}

fn jid_from_string(value: Option<&str>) -> Option<Jid> {
    non_empty(value).map(Jid::new)
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(str::to_owned)
}

impl WaClient {
    /// Start an outgoing voice or video call.
    #[cfg(feature = "calls")]
    pub async fn start_call(&self, chat_id: &Jid, video: bool) -> Result<()> {
        let manager = self.call_manager().await.ok_or(CoreError::NotConnected)?;
        manager.start_call(chat_id, video).await
    }

    /// Start an outgoing voice or video call.
    #[cfg(not(feature = "calls"))]
    pub async fn start_call(&self, _chat_id: &Jid, _video: bool) -> Result<()> {
        Err(CoreError::Internal(START_CALL_UNWIRED.into()))
    }

    /// Hang up the current call.
    #[cfg(feature = "calls")]
    pub async fn end_call(&self, chat_id: &Jid) -> Result<()> {
        let manager = self.call_manager().await.ok_or(CoreError::NotConnected)?;
        manager.end_call(chat_id).await
    }

    /// Hang up the current call.
    #[cfg(not(feature = "calls"))]
    pub async fn end_call(&self, _chat_id: &Jid) -> Result<()> {
        Err(CoreError::Internal(END_CALL_UNWIRED.into()))
    }

    /// Accept a ringing incoming call.
    #[cfg(feature = "calls")]
    pub async fn answer_call(&self, call_id: &str) -> Result<()> {
        let manager = self.call_manager().await.ok_or(CoreError::NotConnected)?;
        manager.answer_call(call_id).await
    }

    /// Reject a ringing incoming call.
    #[cfg(feature = "calls")]
    pub async fn reject_call(&self, call_id: &str) -> Result<()> {
        let manager = self.call_manager().await.ok_or(CoreError::NotConnected)?;
        manager.reject_call(call_id).await
    }

    /// Mute or unmute the microphone of the active call with `chat_id`.
    #[cfg(feature = "calls")]
    pub async fn set_call_muted(&self, chat_id: &Jid, muted: bool) -> Result<()> {
        let manager = self.call_manager().await.ok_or(CoreError::NotConnected)?;
        manager.set_muted(chat_id, muted)
    }
}

#[cfg(not(feature = "calls"))]
const START_CALL_UNWIRED: &str = "calling is not compiled in: build whatsapp-core with --features calls (see docs/research/calls-plan.md)";
#[cfg(not(feature = "calls"))]
const END_CALL_UNWIRED: &str = "calling is not compiled in: build whatsapp-core with --features calls (see docs/research/calls-plan.md)";

/// The real upstream-call adapter, compiled with `--features calls`.
///
/// This module is written against `whatsapp-rust` 0.7.0 and is intentionally
/// complete: the only missing piece is that [`WaClient`] has no field for it
/// (see the wiring instructions in `docs/research/calls-plan.md`). Keeping it
/// feature-gated means the default build is unaffected while the adapter can
/// still be compile-checked and unit-tested with `--features calls`.
#[cfg(feature = "calls")]
pub mod manager {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex as StdMutex, MutexGuard};

    use serde::Serialize;
    use tokio::sync::broadcast;
    use whatsapp_rust::Client;
    use whatsapp_rust::async_channel;
    use whatsapp_rust::types::events::Event;
    use whatsapp_rust::voip::{CallEvent, CallHandle, VideoFrame};
    use whatsapp_rust::wacore::types::call::{
        CallAction, CallEndedElsewhere, ElsewhereOutcome, IncomingCall, MissedCall,
    };

    use crate::error::{CoreError, Result};
    use crate::types::Jid;

    /// Number of call updates buffered per subscriber.
    pub const CALL_BUS_CAPACITY: usize = 64;

    /// Lifecycle stage of a call, normalized for the UI.
    ///
    /// This is deliberately smaller than the upstream `CallPhase`: the media
    /// engine does not report an "answered" transition, so `Active` has to
    /// come from signaling once `client.rs` forwards it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum CallState {
        /// An incoming call is ringing on this device.
        Ringing,
        /// An outgoing offer was sent and we are waiting for the peer.
        Calling,
        /// Relay/DTLS media setup is in progress.
        Connecting,
        /// Media is flowing; the call is established.
        Active,
        /// The call is over.
        Ended,
        /// Setup or media failed.
        Failed,
    }

    /// Snapshot of one call, enough for a calls-list row or a ringing screen.
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CallInfo {
        /// Server call id.
        pub call_id: String,
        /// Chat/peer the call belongs to.
        pub chat_id: Jid,
        /// True for video calls.
        pub video: bool,
        /// Unix seconds when the call started ringing/placed.
        pub timestamp: u64,
    }

    /// Incremental call state pushed to subscribers.
    #[derive(Clone, Debug, Serialize)]
    #[serde(tag = "type", content = "payload", rename_all = "camelCase")]
    pub enum CallUpdate {
        /// An incoming call started ringing.
        Ringing(CallInfo),
        /// An unanswered incoming call ended (offline replay or caller timeout).
        Missed(CallInfo),
        /// The call was answered or declined on another linked device.
        EndedElsewhere {
            /// Server call id.
            call_id: String,
            /// Peer that was ringing.
            chat_id: Jid,
            /// True when another device answered, false when it declined.
            accepted: bool,
            /// Unix seconds of the terminal event.
            timestamp: u64,
        },
        /// Local lifecycle transition.
        Phase {
            /// Server call id.
            call_id: String,
            /// New state.
            state: CallState,
            /// Failure detail, when `state` is [`CallState::Failed`].
            reason: Option<String>,
        },
        /// The call is over locally; media tasks have been torn down.
        Ended {
            /// Server call id.
            call_id: String,
        },
    }

    /// Capture/playback endpoints for one call.
    ///
    /// The channel types implement the upstream endpoint traits directly, so a
    /// macOS capture backend only needs to own the opposite channel ends.
    pub struct CallMedia {
        /// PCM frames the engine reads: exactly 960 mono `i16` samples
        /// (60 ms at 16 kHz). Shorter/longer frames are dropped by the engine.
        pub mic: async_channel::Receiver<Vec<i16>>,
        /// Decoded peer PCM: 16 kHz mono `i16`, best effort (the engine drops
        /// a frame when this sink cannot keep up).
        pub speaker: async_channel::Sender<Vec<i16>>,
        /// H.264 Annex-B access units for outgoing video (one AU per item).
        pub video_source: Option<async_channel::Receiver<Vec<u8>>>,
        /// Reassembled peer H.264 Annex-B access units.
        pub video_sink: Option<async_channel::Sender<VideoFrame>>,
    }

    impl CallMedia {
        /// Audio-only endpoints.
        pub fn audio(
            mic: async_channel::Receiver<Vec<i16>>,
            speaker: async_channel::Sender<Vec<i16>>,
        ) -> Self {
            Self {
                mic,
                speaker,
                video_source: None,
                video_sink: None,
            }
        }

        /// Attach video endpoints (camera or screen-capture source).
        pub fn with_video(
            mut self,
            source: async_channel::Receiver<Vec<u8>>,
            sink: async_channel::Sender<VideoFrame>,
        ) -> Self {
            self.video_source = Some(source);
            self.video_sink = Some(sink);
            self
        }
    }

    /// Platform capture backend, implemented by the macOS host
    /// (AVFoundation camera/mic, ScreenCaptureKit for screen share).
    pub trait CallMediaFactory: Send + Sync + 'static {
        /// Endpoints for a call with `chat_id`. `video` asks for camera
        /// endpoints; a video call without them fails before signalling.
        fn media(&self, chat_id: &Jid, video: bool) -> Result<CallMedia>;
    }

    struct Inner {
        client: Arc<Client>,
        media: Arc<dyn CallMediaFactory>,
        updates: broadcast::Sender<CallUpdate>,
        /// Live calls keyed by chat id string; the handle is cheap to clone.
        active: StdMutex<HashMap<String, CallHandle>>,
        /// Incoming calls that are currently ringing, keyed by call id. The
        /// raw offer is retained so the UI can answer after the event handler
        /// returned (it owns the callKey material).
        ringing: StdMutex<HashMap<String, RingingCall>>,
    }

    #[derive(Clone)]
    struct RingingCall {
        info: CallInfo,
        incoming: IncomingCall,
    }

    impl Inner {
        fn publish(&self, update: CallUpdate) {
            // No subscribers is normal (nobody is watching the calls screen).
            let _ = self.updates.send(update);
        }

        fn insert_active(&self, chat_key: String, handle: CallHandle) {
            lock(&self.active).insert(chat_key, handle);
        }

        fn remove_active(&self, chat_key: &str) -> Option<CallHandle> {
            lock(&self.active).remove(chat_key)
        }

        fn take_ringing(&self, call_id: &str) -> Option<RingingCall> {
            lock(&self.ringing).remove(call_id)
        }

        fn ringing_call(&self, call_id: &str) -> Option<RingingCall> {
            lock(&self.ringing).get(call_id).cloned()
        }

        fn handle_engine_event(&self, call_id: &str, event: &CallEvent) {
            let (state, reason) = match event {
                CallEvent::RelayAllocated => (Some(CallState::Connecting), None),
                CallEvent::RelayAllocateFailed(code) => (
                    Some(CallState::Failed),
                    Some(format!("relay allocation rejected (STUN code {code})")),
                ),
                CallEvent::RelayAllocateTimedOut => (
                    Some(CallState::Failed),
                    Some("relay allocation timed out".to_owned()),
                ),
                CallEvent::RelayReconnectTimedOut => (
                    Some(CallState::Failed),
                    Some("relay reconnect timed out".to_owned()),
                ),
                CallEvent::AudioFormatMismatch {
                    expected_rate,
                    received_rates,
                } => (
                    Some(CallState::Failed),
                    Some(format!(
                        "audio format mismatch: expected {expected_rate} Hz, peer offered {received_rates:?}"
                    )),
                ),
                CallEvent::WaitingRoomHeartbeatFailed => (
                    Some(CallState::Failed),
                    Some("waiting-room heartbeat failed".to_owned()),
                ),
                _ => (None, None),
            };
            if let Some(state) = state {
                self.publish(CallUpdate::Phase {
                    call_id: call_id.to_owned(),
                    state,
                    reason,
                });
            }
        }
    }

    /// Adapter around the upstream VoIP facade.
    ///
    /// `CallManager` is cheap to clone (all state sits behind an `Arc`), so
    /// `WaClient` can hand a clone to its event handler and keep one for the
    /// command surface.
    #[derive(Clone)]
    pub struct CallManager {
        inner: Arc<Inner>,
    }

    impl CallManager {
        /// Wrap a connected upstream client and the platform capture backend.
        pub fn new(client: Arc<Client>, media: Arc<dyn CallMediaFactory>) -> Self {
            let (updates, _receiver) = broadcast::channel(CALL_BUS_CAPACITY);
            Self {
                inner: Arc::new(Inner {
                    client,
                    media,
                    updates,
                    active: StdMutex::new(HashMap::new()),
                    ringing: StdMutex::new(HashMap::new()),
                }),
            }
        }

        /// Subscribe to call state updates. This is the channel `client.rs`
        /// should forward into `CoreEvent` once the event bus grows call
        /// variants.
        pub fn subscribe(&self) -> broadcast::Receiver<CallUpdate> {
            self.inner.updates.subscribe()
        }

        /// Start an outgoing 1:1 audio (`video == false`) or video call.
        pub async fn start_call(&self, chat_id: &Jid, video: bool) -> Result<()> {
            let chat_key = chat_id.to_string();
            if lock(&self.inner.active).contains_key(&chat_key) {
                return Err(CoreError::InvalidInput(format!(
                    "a call with {chat_id} is already active"
                )));
            }
            let peer = to_upstream_jid(chat_id)?;
            let media = self.inner.media.media(chat_id, video)?;
            let CallMedia {
                mic,
                speaker,
                video_source,
                video_sink,
            } = media;

            // Bind the `Voip` handle: the builder borrows it for as long as
            // the call is being set up.
            let voip = self.inner.client.voip();
            let mut builder = voip.call(&peer).audio(mic, speaker);
            if video {
                let (source, sink) = video_endpoints(video_source, video_sink)?;
                builder = builder.video(source, sink);
            }
            let handle = builder
                .start()
                .await
                .map_err(|error| CoreError::Protocol(error.to_string()))?;

            let call_id = handle.call_id().to_owned();
            self.inner.insert_active(chat_key.clone(), handle.clone());
            self.inner.publish(CallUpdate::Phase {
                call_id: call_id.clone(),
                state: CallState::Calling,
                reason: None,
            });
            spawn_monitor(Arc::clone(&self.inner), chat_key, handle);
            Ok(())
        }

        /// Hang up the call with `chat_id`, if one is active.
        pub async fn end_call(&self, chat_id: &Jid) -> Result<()> {
            let handle = self
                .inner
                .remove_active(&chat_id.to_string())
                .ok_or_else(|| CoreError::InvalidInput(format!("no active call with {chat_id}")))?;
            handle.hangup().await;
            Ok(())
        }

        /// Answer a ringing incoming call by call id.
        ///
        /// This is the UI-facing path: [`CallManager::handle_event`] retained
        /// the raw offer (including its callKey material), so answering does
        /// not depend on the event handler's borrow.
        pub async fn answer_call(&self, call_id: &str) -> Result<()> {
            let ringing = self.inner.ringing_call(call_id).ok_or_else(|| {
                CoreError::InvalidInput(format!("no ringing call with id {call_id}"))
            })?;
            self.answer(&ringing.incoming).await
        }

        /// Decline a ringing incoming call by call id.
        pub async fn reject_call(&self, call_id: &str) -> Result<()> {
            let ringing = self.inner.ringing_call(call_id).ok_or_else(|| {
                CoreError::InvalidInput(format!("no ringing call with id {call_id}"))
            })?;
            self.reject(&ringing.incoming).await
        }

        /// Answer a ringing incoming call with the raw event payload.
        pub async fn answer(&self, incoming: &IncomingCall) -> Result<()> {
            let chat_id = Jid::new(incoming.from.to_string());
            let chat_key = chat_id.to_string();
            if lock(&self.inner.active).contains_key(&chat_key) {
                return Err(CoreError::InvalidInput(format!(
                    "a call with {chat_id} is already active"
                )));
            }
            let video = matches!(&incoming.action, CallAction::Offer { is_video: true, .. });
            let media = self.inner.media.media(&chat_id, video)?;
            let CallMedia {
                mic,
                speaker,
                video_source,
                video_sink,
            } = media;

            let voip = self.inner.client.voip();
            let mut builder = voip.accept(incoming).audio(mic, speaker);
            if video {
                // `accept` errors with VideoNotOffered when the offer was
                // audio-only; only attach when the offer advertised video.
                let (source, sink) = video_endpoints(video_source, video_sink)?;
                builder = builder.video(source, sink);
            }
            let handle = builder
                .start()
                .await
                .map_err(|error| CoreError::Protocol(error.to_string()))?;

            let call_id = handle.call_id().to_owned();
            self.inner.take_ringing(&call_id);
            self.inner.insert_active(chat_key.clone(), handle.clone());
            self.inner.publish(CallUpdate::Phase {
                call_id: call_id.clone(),
                state: CallState::Connecting,
                reason: None,
            });
            spawn_monitor(Arc::clone(&self.inner), chat_key, handle);
            Ok(())
        }

        /// Decline a ringing incoming call.
        pub async fn reject(&self, incoming: &IncomingCall) -> Result<()> {
            let call_id = incoming.action.call_id().to_owned();
            self.inner
                .client
                .voip()
                .reject(incoming)
                .await
                .map_err(|error| CoreError::Protocol(error.to_string()))?;
            self.inner.take_ringing(&call_id);
            Ok(())
        }

        /// Mute or unmute the local microphone of the call with `chat_id`.
        pub fn set_muted(&self, chat_id: &Jid, muted: bool) -> Result<()> {
            let active = lock(&self.inner.active);
            let handle = active
                .get(&chat_id.to_string())
                .ok_or_else(|| CoreError::InvalidInput(format!("no active call with {chat_id}")))?;
            handle.set_muted(muted);
            Ok(())
        }

        /// Translate an upstream event-bus event into call state.
        ///
        /// `client.rs` should call this from an `on_event_for` handler
        /// registered for [`EventKind::IncomingCall`](whatsapp_rust::types::events::EventKind::IncomingCall),
        /// `MissedCall` and `CallEndedElsewhere`; the returned update is also
        /// published on [`CallManager::subscribe`].
        pub fn handle_event(&self, event: &Event) {
            match event {
                Event::IncomingCall(incoming) => {
                    let info = CallInfo::from_incoming(incoming);
                    lock(&self.inner.ringing).insert(
                        info.call_id.clone(),
                        RingingCall {
                            info: info.clone(),
                            incoming: incoming.clone(),
                        },
                    );
                    self.inner.publish(CallUpdate::Ringing(info));
                }
                Event::MissedCall(missed) => {
                    let info = self.missed_info(missed);
                    self.inner.publish(CallUpdate::Missed(info));
                }
                Event::CallEndedElsewhere(elsewhere) => {
                    let update = self.elsewhere_info(elsewhere);
                    self.inner.publish(update);
                }
                _ => {}
            }
        }

        fn missed_info(&self, missed: &MissedCall) -> CallInfo {
            self.inner
                .take_ringing(&missed.call_id)
                .map(|ringing| ringing.info)
                .unwrap_or_else(|| CallInfo {
                    call_id: missed.call_id.clone(),
                    chat_id: Jid::new(missed.from.to_string()),
                    video: false,
                    timestamp: unix_secs(missed.timestamp.timestamp()),
                })
        }

        fn elsewhere_info(&self, elsewhere: &CallEndedElsewhere) -> CallUpdate {
            let ringing = self.inner.take_ringing(&elsewhere.call_id);
            CallUpdate::EndedElsewhere {
                call_id: elsewhere.call_id.clone(),
                chat_id: ringing
                    .map(|ringing| ringing.info.chat_id)
                    .unwrap_or_else(|| Jid::new(elsewhere.from.to_string())),
                accepted: matches!(elsewhere.outcome, ElsewhereOutcome::Accepted),
                timestamp: unix_secs(elsewhere.timestamp.timestamp()),
            }
        }
    }

    impl CallInfo {
        fn from_incoming(incoming: &IncomingCall) -> Self {
            Self {
                call_id: incoming.action.call_id().to_owned(),
                chat_id: Jid::new(incoming.from.to_string()),
                video: matches!(&incoming.action, CallAction::Offer { is_video: true, .. }),
                timestamp: unix_secs(incoming.timestamp.timestamp()),
            }
        }
    }

    /// Watch a live call: forward engine diagnostics and publish `Ended` when
    /// the driver stops (peer terminate, hangup or failure).
    fn spawn_monitor(inner: Arc<Inner>, chat_key: String, handle: CallHandle) {
        let call_id = handle.call_id().to_owned();
        tokio::spawn(async move {
            let events = handle.events();
            loop {
                tokio::select! {
                    event = events.recv() => match event {
                        Ok(event) => inner.handle_engine_event(&call_id, &event),
                        Err(_) => {
                            // No more engine diagnostics; wait for teardown.
                            handle.wait_ended().await;
                            break;
                        }
                    },
                    () = handle.wait_ended() => break,
                }
            }
            inner.remove_active(&chat_key);
            inner.publish(CallUpdate::Ended { call_id });
        });
    }

    fn video_endpoints(
        source: Option<async_channel::Receiver<Vec<u8>>>,
        sink: Option<async_channel::Sender<VideoFrame>>,
    ) -> Result<(
        async_channel::Receiver<Vec<u8>>,
        async_channel::Sender<VideoFrame>,
    )> {
        match (source, sink) {
            (Some(source), Some(sink)) => Ok((source, sink)),
            _ => Err(CoreError::InvalidInput(
                "video endpoints requested but the media backend has none".into(),
            )),
        }
    }

    fn lock<T>(mutex: &StdMutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn to_upstream_jid(jid: &Jid) -> Result<whatsapp_rust::Jid> {
        use std::str::FromStr;

        whatsapp_rust::Jid::from_str(jid.as_str())
            .map_err(|error| CoreError::InvalidInput(error.to_string()))
    }

    fn unix_secs(value: i64) -> u64 {
        value.max(0) as u64
    }
}

#[cfg(feature = "calls")]
pub use manager::{
    CALL_BUS_CAPACITY, CallInfo, CallManager, CallMedia, CallMediaFactory, CallState, CallUpdate,
};

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::prelude::MessageBuilderExt as _;

    #[test]
    fn maps_history_sync_call_log_records() {
        let record = wa::CallLogRecord {
            call_result: Some(wa::call_log_record::CallResult::MISSED),
            duration: Some(42),
            start_time: Some(1_700_000_000),
            is_incoming: Some(true),
            is_video: Some(true),
            call_id: Some("CALL-1".to_owned()),
            call_creator_jid: Some("alice@s.whatsapp.net".to_owned()),
            participants: vec![wa::call_log_record::ParticipantInfo {
                user_jid: Some("alice@s.whatsapp.net".to_owned()),
                call_result: None,
            }],
            ..Default::default()
        };
        let history_sync = wa::HistorySync {
            call_log_records: vec![record],
            ..Default::default()
        };

        let entries = call_log_entries(&history_sync);
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.call_id.as_deref(), Some("CALL-1"));
        assert_eq!(entry.result, CallLogResult::Missed);
        assert_eq!(entry.duration_secs, Some(42));
        assert_eq!(entry.started_at_unix, Some(1_700_000_000));
        assert_eq!(entry.incoming, Some(true));
        assert!(entry.video);
        assert!(!entry.call_link);
        assert_eq!(entry.participants, vec![Jid::new("alice@s.whatsapp.net")]);
        assert_eq!(entry.call_creator, Some(Jid::new("alice@s.whatsapp.net")));
    }

    #[test]
    fn call_log_record_defaults_are_honest() {
        let entry = call_log_entry_from_record(&wa::CallLogRecord::default());
        assert_eq!(entry.result, CallLogResult::Unknown);
        assert_eq!(entry.incoming, None);
        assert_eq!(entry.duration_secs, None);
        assert!(entry.participants.is_empty());
    }

    #[test]
    fn maps_call_log_message() {
        let mut message = wa::Message::default();
        message.call_log_messsage =
            whatsapp_rust::buffa::MessageField::some(wa::message::CallLogMessage {
                is_video: Some(false),
                call_outcome: Some(wa::message::call_log_message::CallOutcome::CONNECTED),
                duration_secs: Some(95),
                participants: vec![wa::message::call_log_message::CallParticipant {
                    jid: Some("15551234567@s.whatsapp.net".to_owned()),
                    call_outcome: None,
                }],
                ..Default::default()
            });

        let entry = call_log_entry_from_message(&message).expect("call log item");
        assert_eq!(entry.result, CallLogResult::Connected);
        assert_eq!(entry.duration_secs, Some(95));
        assert_eq!(entry.incoming, None);
        assert!(!entry.video);
        assert_eq!(
            entry.participants,
            vec![Jid::new("15551234567@s.whatsapp.net")]
        );
    }

    #[test]
    fn non_call_log_message_has_no_entry() {
        let message = wa::Message::text("hello");
        assert!(call_log_entry_from_message(&message).is_none());
    }
}
