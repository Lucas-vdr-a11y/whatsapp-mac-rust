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
//! - [`CallLogStore`] and the pure row conversions
//!   ([`CallLogEntry::to_record`], [`record_from_entry`], [`entry_from_record`])
//!   that persist those records in the `call_log` table (schema 005). The
//!   SQLite implementation is the `store.rs` follow-up documented on the trait;
//!   [`WaClient::import_call_log`] and [`WaClient::list_call_log`] report the
//!   unwired hook until then.
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

impl CallLogResult {
    /// Stable lowercase key, matching the serde wire form. Used for the
    /// `call_log` outcome column and for hashing id-less records.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Missed => "missed",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::AcceptedElsewhere => "acceptedElsewhere",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
            Self::Upcoming => "upcoming",
            Self::Abandoned => "abandoned",
            Self::Ongoing => "ongoing",
            Self::Invalid => "invalid",
            Self::Unknown => "unknown",
        }
    }
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

/// Coarse call outcome persisted in `call_log.outcome`.
///
/// The database schema deliberately stores one of five UI-friendly values
/// instead of the full [`CallLogResult`]; [`CallOutcome::from_result`] is the
/// lossy projection and [`CallOutcome::to_result`] the best-effort inverse for
/// the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CallOutcome {
    /// Nobody answered, or the caller gave up before the call connected.
    Missed,
    /// The call connected (possibly on another linked device).
    Answered,
    /// The callee rejected the call.
    Declined,
    /// The call failed to establish.
    Failed,
    /// The call is still in progress, or scheduled and not started yet.
    Ongoing,
}

impl CallOutcome {
    /// The exact string stored in `call_log.outcome`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missed => "missed",
            Self::Answered => "answered",
            Self::Declined => "declined",
            Self::Failed => "failed",
            Self::Ongoing => "ongoing",
        }
    }

    /// Project a full [`CallLogResult`] onto the coarse persisted set.
    ///
    /// `AcceptedElsewhere` counts as answered; `Cancelled`, `Unavailable`,
    /// `Abandoned` and `Unknown` collapse into missed; `Invalid` is a failed
    /// record and `Upcoming` is not terminal yet, so it stays ongoing.
    pub fn from_result(result: CallLogResult) -> Self {
        match result {
            CallLogResult::Connected | CallLogResult::AcceptedElsewhere => Self::Answered,
            CallLogResult::Rejected => Self::Declined,
            CallLogResult::Failed | CallLogResult::Invalid => Self::Failed,
            CallLogResult::Ongoing | CallLogResult::Upcoming => Self::Ongoing,
            CallLogResult::Missed
            | CallLogResult::Cancelled
            | CallLogResult::Unavailable
            | CallLogResult::Abandoned
            | CallLogResult::Unknown => Self::Missed,
        }
    }

    /// Best-effort inverse of [`CallOutcome::from_result`] for the UI.
    pub fn to_result(self) -> CallLogResult {
        match self {
            Self::Missed => CallLogResult::Missed,
            Self::Answered => CallLogResult::Connected,
            Self::Declined => CallLogResult::Rejected,
            Self::Failed => CallLogResult::Failed,
            Self::Ongoing => CallLogResult::Ongoing,
        }
    }
}

impl From<&str> for CallOutcome {
    /// Parse the persisted form; anything unrecognized is treated as missed,
    /// which is the safe default for a call that may have been unanswered.
    fn from(value: &str) -> Self {
        match value {
            "answered" => Self::Answered,
            "declined" => Self::Declined,
            "failed" => Self::Failed,
            "ongoing" => Self::Ongoing,
            _ => Self::Missed,
        }
    }
}

/// One persisted `call_log` row (schema 005).
///
/// Store-independent: `store.rs` reads and writes these rows through
/// [`CallLogStore`], while [`CallLogEntry`] stays the IPC-facing shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallLogRecord {
    /// Stable key: the server call id, or `sha256:<hex>` for id-less records.
    pub id: String,
    /// Chat/peer JID the call belongs to, when known.
    pub chat_id: Option<Jid>,
    /// True when the call was placed from this account. Records without
    /// direction default to incoming (`false`).
    pub from_me: bool,
    /// True for video calls.
    pub video: bool,
    /// Coarse outcome stored in the `outcome` column.
    pub outcome: CallOutcome,
    /// Unix seconds the call started; `0` when the source did not report one.
    pub started_at: u64,
    /// Talk duration in seconds, when reported.
    pub duration_secs: Option<u64>,
    /// Serialized source record for debugging and future re-processing, when
    /// available. [`import_call_log_into`] fills it with the normalized entry
    /// JSON; the proto record itself lives only in the ingest hook.
    pub raw: Option<Vec<u8>>,
}

/// The persistence surface for the synced call log.
///
/// `store.rs` owns the SQLite connection and its migrations, so this module
/// programs against this trait; the one-time follow-up adds the schema and an
/// `impl CallLogStore for Store`. The exact implementation:
///
/// ```ignore
/// // store.rs, next to the media import:
/// use crate::calls::{CallLogRecord, CallLogStore, CallOutcome};
///
/// impl CallLogStore for Store {
///     fn upsert_call_log(&self, record: &CallLogRecord) -> Result<()> {
///         let connection = self.lock()?;
///         connection
///             .execute(
///                 "INSERT INTO call_log (id, chat_id, from_me, video, outcome,
///                                       started_at, duration_secs, raw)
///                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
///                  ON CONFLICT(id) DO UPDATE SET
///                      chat_id = excluded.chat_id,
///                      from_me = excluded.from_me,
///                      video = excluded.video,
///                      outcome = excluded.outcome,
///                      started_at = excluded.started_at,
///                      duration_secs = excluded.duration_secs,
///                      raw = excluded.raw",
///                 params![
///                     &record.id,
///                     record.chat_id.as_ref().map(Jid::as_str),
///                     i64::from(record.from_me),
///                     i64::from(record.video),
///                     record.outcome.as_str(),
///                     as_i64(record.started_at),
///                     record.duration_secs.map(as_i64),
///                     record.raw.as_deref(),
///                 ],
///             )
///             .map_err(storage_error)?;
///         Ok(())
///     }
///
///     fn list_call_log(&self, limit: u32) -> Result<Vec<CallLogRecord>> {
///         let connection = self.lock()?;
///         let mut statement = connection
///             .prepare(
///                 "SELECT id, chat_id, from_me, video, outcome, started_at,
///                         duration_secs, raw
///                  FROM call_log
///                  ORDER BY started_at DESC, rowid DESC
///                  LIMIT ?1",
///             )
///             .map_err(storage_error)?;
///         let rows = statement
///             .query_map(params![limit], |row| {
///                 Ok(CallLogRecord {
///                     id: row.get(0)?,
///                     chat_id: row.get::<_, Option<String>>(1)?.map(Jid::new),
///                     from_me: row.get::<_, i64>(2)? != 0,
///                     video: row.get::<_, i64>(3)? != 0,
///                     outcome: CallOutcome::from(row.get::<_, String>(4)?.as_str()),
///                     started_at: row.get::<_, i64>(5)?.max(0) as u64,
///                     duration_secs: row
///                         .get::<_, Option<i64>>(6)?
///                         .map(|value| value.max(0) as u64),
///                     raw: row.get(7)?,
///                 })
///             })
///             .map_err(storage_error)?;
///         rows.collect::<std::result::Result<Vec<_>, _>>()
///             .map_err(storage_error)
///     }
/// }
/// ```
///
/// With that in place, replace the two `WaClient` call-log bodies at the
/// bottom of this module with their documented one-liners.
pub trait CallLogStore: Send + Sync {
    /// Insert or update one `call_log` row, keyed by [`CallLogRecord::id`].
    fn upsert_call_log(&self, record: &CallLogRecord) -> Result<()>;

    /// The most recent rows, newest first, capped at `limit`.
    fn list_call_log(&self, limit: u32) -> Result<Vec<CallLogRecord>>;
}

impl CallLogEntry {
    /// Project this entry onto the persistable [`CallLogRecord`].
    ///
    /// `raw` stays `None`: it is reserved for the serialized source record,
    /// which only the caller that parsed the protobuf still has.
    pub fn to_record(&self) -> CallLogRecord {
        record_from_entry(self)
    }
}

/// Project one normalized entry onto the `call_log` row it persists as.
///
/// The chat id is the group JID for group calls, otherwise the first
/// participant. An entry without a server call id gets a deterministic
/// `sha256:<hex>` key, so re-importing the same record merges instead of
/// duplicating. Direction is only "outgoing" when the source said so; records
/// that do not carry direction default to incoming.
pub fn record_from_entry(entry: &CallLogEntry) -> CallLogRecord {
    CallLogRecord {
        id: entry.call_id.clone().unwrap_or_else(|| {
            format!("sha256:{}", sha256_hex(&record_key_bytes(entry)))
        }),
        chat_id: entry
            .group_jid
            .clone()
            .or_else(|| entry.participants.first().cloned()),
        from_me: entry.incoming == Some(false),
        video: entry.video,
        outcome: CallOutcome::from_result(entry.result),
        started_at: entry.started_at_unix.unwrap_or(0).max(0) as u64,
        duration_secs: entry.duration_secs,
        raw: None,
    }
}

/// Canonical byte serialization of an entry, used for the SHA-256 fallback id.
///
/// Every field is tag-prefixed and NUL-terminated, so distinct records cannot
/// collide by concatenation. The `call-log-v1` prefix lets the format evolve
/// without reinterpreting old ids.
fn record_key_bytes(entry: &CallLogEntry) -> Vec<u8> {
    fn push(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    }

    let mut bytes = Vec::new();
    push(&mut bytes, "call-log-v1");
    push(&mut bytes, "incoming");
    push(
        &mut bytes,
        match entry.incoming {
            Some(true) => "yes",
            Some(false) => "no",
            None => "unknown",
        },
    );
    push(&mut bytes, "video");
    push(&mut bytes, if entry.video { "yes" } else { "no" });
    push(&mut bytes, "link");
    push(&mut bytes, if entry.call_link { "yes" } else { "no" });
    push(&mut bytes, "result");
    push(&mut bytes, entry.result.as_str());
    push(&mut bytes, "duration");
    push(
        &mut bytes,
        &entry
            .duration_secs
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    push(&mut bytes, "started");
    push(
        &mut bytes,
        &entry
            .started_at_unix
            .map(|value| value.to_string())
            .unwrap_or_default(),
    );
    if let Some(group) = &entry.group_jid {
        push(&mut bytes, "group");
        push(&mut bytes, group.as_str());
    }
    if let Some(creator) = &entry.call_creator {
        push(&mut bytes, "creator");
        push(&mut bytes, creator.as_str());
    }
    push(&mut bytes, "participants");
    for participant in &entry.participants {
        push(&mut bytes, participant.as_str());
    }
    bytes
}

/// Rebuild the UI-facing entry from a stored row.
///
/// The row keeps only the derived chat id, so the reconstruction is lossy by
/// design: the chat id becomes the sole participant (or the group JID) and the
/// stable row id is carried in `call_id` so clients always have a unique key.
pub fn entry_from_record(record: &CallLogRecord) -> CallLogEntry {
    let group = record.chat_id.as_ref().filter(|chat_id| chat_id.is_group());
    CallLogEntry {
        call_id: Some(record.id.clone()),
        participants: match (group, &record.chat_id) {
            (None, Some(chat_id)) => vec![chat_id.clone()],
            _ => Vec::new(),
        },
        group_jid: group.cloned(),
        call_creator: None,
        incoming: Some(!record.from_me),
        video: record.video,
        call_link: false,
        result: record.outcome.to_result(),
        duration_secs: record.duration_secs,
        started_at_unix: Some(i64::try_from(record.started_at).unwrap_or(i64::MAX)),
    }
}

/// Like [`call_log_entry_from_message`], but records the envelope direction.
///
/// Individual `CallLogMessage` items do not carry direction; the hosting
/// message's `from_me` flag does. Use this variant for live imports so the row
/// can be rendered as incoming or outgoing.
pub fn call_log_entry_from_message_with_direction(
    message: &wa::Message,
    from_me: bool,
) -> Option<CallLogEntry> {
    let mut entry = call_log_entry_from_message(message)?;
    entry.incoming = Some(!from_me);
    Some(entry)
}

/// Persist `entries` through a [`CallLogStore`], returning how many rows were
/// written. Rows missing a start time get "now" so the UI can order them;
/// re-imports merge on the stable row id. This is the body
/// [`WaClient::import_call_log`] gets once `Store` implements the trait.
pub fn import_call_log_into(store: &dyn CallLogStore, entries: &[CallLogEntry]) -> Result<usize> {
    let now = now_unix();
    let mut imported = 0usize;
    for entry in entries {
        let mut record = entry.to_record();
        if record.started_at == 0 {
            record.started_at = now;
        }
        if record.raw.is_none() {
            record.raw = whatsapp_rust::serde_json::to_vec(entry).ok();
        }
        store.upsert_call_log(&record)?;
        imported += 1;
    }
    Ok(imported)
}

/// Read rows back through a [`CallLogStore`], newest first and mapped back to
/// the IPC-facing [`CallLogEntry`]. This is the body [`WaClient::list_call_log`]
/// gets once `Store` implements the trait.
pub fn list_call_log_from(store: &dyn CallLogStore, limit: u32) -> Result<Vec<CallLogEntry>> {
    Ok(store
        .list_call_log(limit)?
        .iter()
        .map(entry_from_record)
        .collect())
}

/// SHA-256 of `data` as lowercase hex (FIPS 180-4).
///
/// Used only to derive a stable id for call-log records without a server call
/// id. Kept dependency-free because the core crate has no hash dependency;
/// verified against the NIST test vectors in this module's tests.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1,
        0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
        0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786,
        0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
        0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147,
        0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
        0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
        0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
        0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a,
        0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
        0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
    ];

    let mut state: [u32; 8] = [
        0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c,
        0x1f83_d9ab, 0x5be0_cd19,
    ];

    // Pad to a multiple of 64 bytes: 0x80, zeroes, then the 64-bit bit length.
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut schedule = [0u32; 64];
        for (index, word) in schedule.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for index in 16..64 {
            let x = schedule[index - 15];
            let y = schedule[index - 2];
            let gamma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let gamma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(gamma0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(gamma1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sigma0.wrapping_add(majority);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut hex = String::with_capacity(64);
    for word in state {
        hex.push_str(&format!("{word:08x}"));
    }
    hex
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

impl WaClient {
    /// Persist call-log entries from history sync or a live `CallLogMessage`;
    /// returns the number of rows written.
    ///
    /// The SQL lives behind [`CallLogStore`]. Until `store.rs` implements the
    /// trait this reports the missing hook instead of silently dropping
    /// history; afterwards the body is the documented one-liner:
    ///
    /// ```ignore
    /// import_call_log_into(&*self.store(), &entries)
    /// ```
    pub fn import_call_log(&self, entries: Vec<CallLogEntry>) -> Result<usize> {
        import_call_log_into(&*self.store(), &entries)
    }

    /// Phone-synced call-log rows, newest first.
    ///
    /// Same wiring as [`WaClient::import_call_log`]; the body becomes:
    ///
    /// ```ignore
    /// list_call_log_from(&*self.store(), limit)
    /// ```
    pub fn list_call_log(&self, limit: u32) -> Result<Vec<CallLogEntry>> {
        list_call_log_from(&*self.store(), limit)
    }
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

    /// In-memory [`CallLogStore`] proving the pure conversions and the
    /// `import`/`list` helpers without the SQLite wiring.
    #[derive(Default)]
    struct FakeCallLogStore {
        rows: std::sync::Mutex<Vec<CallLogRecord>>,
    }

    impl CallLogStore for FakeCallLogStore {
        fn upsert_call_log(&self, record: &CallLogRecord) -> Result<()> {
            let mut rows = self.rows.lock().expect("fake store lock");
            match rows.iter_mut().find(|row| row.id == record.id) {
                Some(existing) => *existing = record.clone(),
                None => rows.push(record.clone()),
            }
            Ok(())
        }

        fn list_call_log(&self, limit: u32) -> Result<Vec<CallLogRecord>> {
            let rows = self.rows.lock().expect("fake store lock");
            let mut rows = rows.clone();
            rows.sort_by_key(|row| std::cmp::Reverse(row.started_at));
            rows.truncate(limit as usize);
            Ok(rows)
        }
    }

    fn connected_entry() -> CallLogEntry {
        CallLogEntry {
            call_id: Some("CALL-7".to_owned()),
            participants: vec![Jid::new("bob@s.whatsapp.net")],
            group_jid: None,
            call_creator: None,
            incoming: Some(false),
            video: true,
            call_link: false,
            result: CallLogResult::Connected,
            duration_secs: Some(125),
            started_at_unix: Some(1_700_000_500),
        }
    }

    #[test]
    fn hashes_to_known_sha256_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn projects_entries_onto_rows() {
        let record = connected_entry().to_record();
        assert_eq!(record.id, "CALL-7");
        assert_eq!(record.chat_id, Some(Jid::new("bob@s.whatsapp.net")));
        assert!(record.from_me);
        assert!(record.video);
        assert_eq!(record.outcome, CallOutcome::Answered);
        assert_eq!(record.started_at, 1_700_000_500);
        assert_eq!(record.duration_secs, Some(125));
        assert_eq!(record.raw, None);

        let restored = entry_from_record(&record);
        assert_eq!(restored.call_id.as_deref(), Some("CALL-7"));
        assert_eq!(restored.participants, vec![Jid::new("bob@s.whatsapp.net")]);
        assert_eq!(restored.group_jid, None);
        assert_eq!(restored.incoming, Some(false));
        assert!(restored.video);
        assert_eq!(restored.result, CallLogResult::Connected);
        assert_eq!(restored.duration_secs, Some(125));
        assert_eq!(restored.started_at_unix, Some(1_700_000_500));
    }

    #[test]
    fn derives_a_sha256_id_when_the_record_has_no_call_id() {
        let mut entry = connected_entry();
        entry.call_id = None;

        let first = record_from_entry(&entry);
        let second = record_from_entry(&entry);
        assert_eq!(first.id, second.id);
        assert!(first.id.starts_with("sha256:"));
        assert_eq!(first.id.len(), "sha256:".len() + 64);

        // A different payload produces a different stable id.
        entry.duration_secs = Some(126);
        assert_ne!(record_from_entry(&entry).id, first.id);
    }

    #[test]
    fn persists_groups_by_their_group_jid() {
        let mut entry = connected_entry();
        entry.participants = vec![Jid::new("alice@s.whatsapp.net")];
        entry.group_jid = Some(Jid::new("1234567890-123@g.us"));

        let record = record_from_entry(&entry);
        assert_eq!(record.chat_id, Some(Jid::new("1234567890-123@g.us")));

        let restored = entry_from_record(&record);
        assert_eq!(restored.group_jid, Some(Jid::new("1234567890-123@g.us")));
        assert!(restored.participants.is_empty());
    }

    #[test]
    fn outcomes_round_trip_through_the_coarse_set() {
        for result in [
            CallLogResult::Connected,
            CallLogResult::Rejected,
            CallLogResult::Failed,
            CallLogResult::Ongoing,
            CallLogResult::Missed,
        ] {
            assert_eq!(CallOutcome::from_result(result).to_result(), result);
        }
        assert_eq!(
            CallOutcome::from_result(CallLogResult::AcceptedElsewhere),
            CallOutcome::Answered
        );
        assert_eq!(
            CallOutcome::from_result(CallLogResult::Cancelled),
            CallOutcome::Missed
        );
        assert_eq!(
            CallOutcome::from_result(CallLogResult::Upcoming),
            CallOutcome::Ongoing
        );
        assert_eq!(CallOutcome::from("declined").as_str(), "declined");
        assert_eq!(CallOutcome::from("nonsense"), CallOutcome::Missed);
    }

    #[test]
    fn imports_and_lists_through_the_trait() {
        let store = FakeCallLogStore::default();
        let mut live = connected_entry();
        live.call_id = None;
        live.started_at_unix = None;
        live.result = CallLogResult::Missed;

        let entries = vec![connected_entry(), live];
        assert_eq!(import_call_log_into(&store, &entries).expect("import"), 2);
        // The stored row carries a JSON snapshot now that the source is gone.
        assert!(store.rows.lock().expect("lock").iter().all(|row| row.raw.is_some()));

        // Re-importing merges instead of duplicating.
        assert_eq!(import_call_log_into(&store, &entries).expect("re-import"), 2);
        assert_eq!(store.rows.lock().expect("lock").len(), 2);

        let listed = list_call_log_from(&store, 10).expect("list");
        assert_eq!(listed.len(), 2);
        // The id-less, start-less record got "now" and sorts first.
        assert_eq!(listed[0].result, CallLogResult::Missed);
        assert!(listed[0].started_at_unix.is_some_and(|value| value > 0));
        assert!(
            listed[0]
                .call_id
                .as_deref()
                .is_some_and(|id| id.starts_with("sha256:"))
        );
        assert_eq!(listed[1].call_id.as_deref(), Some("CALL-7"));

        // The limit is honored.
        assert_eq!(list_call_log_from(&store, 1).expect("list one").len(), 1);
    }

    #[test]
    fn fills_direction_from_the_hosting_message_envelope() {
        let mut message = wa::Message::default();
        message.call_log_messsage =
            whatsapp_rust::buffa::MessageField::some(wa::message::CallLogMessage {
                is_video: Some(false),
                call_outcome: Some(wa::message::call_log_message::CallOutcome::CONNECTED),
                duration_secs: Some(12),
                ..Default::default()
            });

        let outgoing =
            call_log_entry_from_message_with_direction(&message, true).expect("outgoing entry");
        assert_eq!(outgoing.incoming, Some(false));
        assert_eq!(outgoing.result, CallLogResult::Connected);

        let incoming =
            call_log_entry_from_message_with_direction(&message, false).expect("incoming entry");
        assert_eq!(incoming.incoming, Some(true));

        assert!(call_log_entry_from_message_with_direction(&wa::Message::text("hi"), true).is_none());
    }
}
