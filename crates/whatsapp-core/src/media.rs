//! Media pipeline: download and cache of inbound media, upload of outbound
//! media. Binaries live in `data_dir/media`; the store keeps metadata.
//!
//! # Pieces
//!
//! * [`classify_media`] maps a raw protobuf message to a [`Downloadable`]
//!   reference (image, video, PTV clip, audio/voice note, document, sticker).
//! * [`cache_file_name`] derives a deterministic, collision-resistant cache
//!   path from the content hash plus the original file name.
//! * [`classify_outgoing`] maps a file path to the upload [`MediaType`], the
//!   UI kind and the MIME type to advertise.
//! * [`MediaPipeline::send_video_note`] sends a short picked video as a
//!   push-to-video ("video note") message; [`mp4_duration_seconds`] enforces
//!   the 60-second recorder cap without a media-parsing dependency.
//! * [`MediaPipeline`] wires those helpers to the upstream `Client` (download,
//!   upload, send) and to [`MediaStore`] (raw protobuf bytes + media cache
//!   rows).
//!
//! # Wiring (one small follow-up)
//!
//! `WaClient` keeps its data directory private and `store.rs` owns both the
//! SQLite connection and the schema migrations, so the two `WaClient` command
//! methods at the bottom of this module cannot reach [`MediaPipeline`] yet.
//! Everything else is ready; the follow-up is mechanical:
//!
//! 1. `store.rs`: bump `SCHEMA_VERSION` to `2` and run
//!    `include_str!("schema/002_media.sql")` when `user_version < 2`. Then
//!    implement [`MediaStore`] for `Store` (see the trait docs for the exact
//!    SQL; three of the five methods delegate to existing `Store` methods).
//! 2. `client.rs`: add `pub(crate) fn data_dir(&self) -> &std::path::Path`
//!    (or make the `config` field `pub(crate)`), and in `handle_inbound_message`
//!    persist the raw payload. `client()` is already `pub(crate)` and `emit()`
//!    already exists:
//!
//!    ```ignore
//!    use whatsapp_rust::buffa::Message as _;
//!    if let Err(error) = store.set_raw_proto(&message.id, &context.message.encode_to_vec()) {
//!        tracing::warn!(%error, "failed to store raw protobuf for inbound message");
//!    }
//!    ```
//!
//! 3. Replace the bodies of [`WaClient::download_media`] and
//!    [`WaClient::send_file`] with the pipeline delegations documented on each
//!    method. No logic moves: the pipeline already does the work, and
//!    `WaClient::send_file` emits the returned message itself.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use serde::Serialize;
use whatsapp_rust::Client;
use whatsapp_rust::buffa::{Message as _, MessageField};
use whatsapp_rust::download::{Downloadable, MediaType};
use whatsapp_rust::media::{
    AudioOptions, DocumentOptions, ImageOptions, VideoOptions, audio_message, document_message,
    image_message, video_message,
};
use whatsapp_rust::proto_helpers::MessageExt;
use whatsapp_rust::upload::UploadOptions;
use whatsapp_rust::waproto::whatsapp as wa;

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::events::CoreEvent;
use crate::types::{Jid, Message, MessageKind, MessageStatus};

/// Subdirectory of `data_dir` holding cached media binaries.
pub const MEDIA_DIR_NAME: &str = "media";

/// Fallback sender id for own messages before the account JID is known.
const ME_PLACEHOLDER: &str = "me";

/// A media file available on disk.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    /// Absolute path of the cached file.
    pub path: String,
    /// MIME type, when known.
    pub mime: Option<String>,
    /// Original file name, when known.
    pub file_name: Option<String>,
    /// Size in bytes.
    pub size: u64,
}

/// Metadata of a cached media binary, mirrored in the `media` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRecord {
    /// Message the binary belongs to.
    pub message_id: String,
    /// MIME type the sender declared, when known.
    pub mime: Option<String>,
    /// Original file name, when known.
    pub file_name: Option<String>,
    /// Size of the cached binary in bytes.
    pub size: u64,
    /// Absolute path of the cached binary.
    pub local_path: String,
    /// SHA-256 of the plaintext bytes, as carried by the protobuf message.
    pub sha256: Option<Vec<u8>>,
    /// Unix seconds of the download.
    pub downloaded_at: u64,
}

impl MediaRecord {
    /// Project the stored record onto the IPC-facing [`MediaFile`].
    pub fn to_media_file(&self) -> MediaFile {
        MediaFile {
            path: self.local_path.clone(),
            mime: self.mime.clone(),
            file_name: self.file_name.clone(),
            size: self.size,
        }
    }
}

/// The persistence surface the media pipeline needs.
///
/// `store.rs` owns the SQLite connection and its migrations, so the pipeline
/// programs against this trait; the one-time follow-up adds the schema and an
/// `impl MediaStore for Store`. The exact implementation:
///
/// ```ignore
/// // store.rs, inside `impl Store` or as a free impl block:
/// use crate::media::{MediaRecord, MediaStore};
///
/// impl MediaStore for Store {
///     fn raw_proto(&self, message_id: &str) -> Result<Option<Vec<u8>>> {
///         let connection = self.lock()?;
///         connection
///             .query_row(
///                 "SELECT raw_proto FROM messages WHERE id = ?1",
///                 params![message_id],
///                 |row| row.get::<_, Option<Vec<u8>>>(0),
///             )
///             .optional()
///             .map(Option::flatten)
///             .map_err(storage_error)
///     }
///
///     fn set_raw_proto(&self, message_id: &str, raw_proto: &[u8]) -> Result<()> {
///         let connection = self.lock()?;
///         connection
///             .execute(
///                 "UPDATE messages SET raw_proto = ?1 WHERE id = ?2",
///                 params![raw_proto, message_id],
///             )
///             .map_err(storage_error)?;
///         Ok(())
///     }
///
///     fn media_record(&self, message_id: &str) -> Result<Option<MediaRecord>> {
///         let connection = self.lock()?;
///         connection
///             .query_row(
///                 "SELECT message_id, mime, file_name, size, local_path, sha256,
///                         downloaded_at
///                  FROM media WHERE message_id = ?1",
///                 params![message_id],
///                 |row| {
///                     Ok(MediaRecord {
///                         message_id: row.get(0)?,
///                         mime: row.get(1)?,
///                         file_name: row.get(2)?,
///                         size: row.get::<_, i64>(3)? as u64,
///                         local_path: row.get(4)?,
///                         sha256: row.get(5)?,
///                         downloaded_at: row.get::<_, i64>(6)? as u64,
///                     })
///                 },
///             )
///             .optional()
///             .map_err(storage_error)
///     }
///
///     fn upsert_media_record(&self, record: &MediaRecord) -> Result<()> {
///         let connection = self.lock()?;
///         connection
///             .execute(
///                 "INSERT INTO media (message_id, mime, file_name, size,
///                                     local_path, sha256, downloaded_at)
///                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
///                  ON CONFLICT(message_id) DO UPDATE SET
///                      mime = excluded.mime,
///                      file_name = excluded.file_name,
///                      size = excluded.size,
///                      local_path = excluded.local_path,
///                      sha256 = excluded.sha256,
///                      downloaded_at = excluded.downloaded_at",
///                 params![
///                     &record.message_id,
///                     record.mime.as_deref(),
///                     record.file_name.as_deref(),
///                     as_i64(record.size),
///                     &record.local_path,
///                     record.sha256.as_deref(),
///                     as_i64(record.downloaded_at),
///                 ],
///             )
///             .map_err(storage_error)?;
///         Ok(())
///     }
///
///     fn save_outgoing_message(
///         &self,
///         chat_id: &Jid,
///         message: &Message,
///         preview: &str,
///     ) -> Result<()> {
///         // Same order as `WaClient::send_text`: the chat row must exist
///         // before the message references it.
///         self.record_message_activity(chat_id, preview, message.timestamp, None, false)?;
///         self.upsert_message(message)
///     }
/// }
/// ```
pub trait MediaStore: Send + Sync {
    /// Serialized `wa::Message` stored for `message_id`, if any.
    fn raw_proto(&self, message_id: &str) -> Result<Option<Vec<u8>>>;

    /// Persist the serialized `wa::Message` for an already stored message row.
    fn set_raw_proto(&self, message_id: &str, raw_proto: &[u8]) -> Result<()>;

    /// Cached binary metadata for `message_id`, if any.
    fn media_record(&self, message_id: &str) -> Result<Option<MediaRecord>>;

    /// Insert or update cached binary metadata.
    fn upsert_media_record(&self, record: &MediaRecord) -> Result<()>;

    /// Persist an outgoing message and its chat activity, exactly like
    /// `WaClient::send_text` does.
    fn save_outgoing_message(&self, chat_id: &Jid, message: &Message, preview: &str) -> Result<()>;
}

/// A downloadable payload found in a raw protobuf message.
pub struct MediaAttachment<'a> {
    /// UI kind: image, video, GIF, audio, voice note, document or sticker.
    pub kind: MessageKind,
    /// CDN reference and decryption material.
    pub source: &'a dyn Downloadable,
    /// MIME type the sender declared, when known.
    pub mime: Option<String>,
    /// Original file name, when known (documents).
    pub file_name: Option<String>,
    /// Plaintext length in bytes, when known.
    pub size: Option<u64>,
    /// SHA-256 of the plaintext bytes, when known.
    pub sha256: Option<Vec<u8>>,
}

/// True when a raw protobuf message carries view-once content.
///
/// Covers the legacy `viewOnceMessage` / `viewOnceMessageV2` /
/// `viewOnceMessageV2Extension` envelopes — also when they are nested under
/// `deviceSentMessage` or `ephemeralMessage` — and the inline `view_once`
/// flag on modern image/video/audio payloads. [`classify_media`] unwraps the
/// same envelopes, so callers can get the inner kind *and* this flag from one
/// payload.
pub fn is_view_once(message: &wa::Message) -> bool {
    message.is_view_once()
}

/// Find the downloadable payload of a message, unwrapping
/// ephemeral/view-once/edited wrappers. Returns `None` for messages that carry
/// no media (text, location, polls, …) and for kinds this build cannot fetch.
pub fn classify_media(message: &wa::Message) -> Option<MediaAttachment<'_>> {
    let base = message.get_base_message();

    if let Some(image) = base.image_message.as_option() {
        return Some(MediaAttachment {
            kind: MessageKind::Image,
            source: image,
            mime: image.mimetype.clone(),
            file_name: None,
            size: image.file_length,
            sha256: image.file_sha256.clone(),
        });
    }
    if let Some(video) = base.video_message.as_option() {
        let kind = if video.gif_playback.unwrap_or(false) {
            MessageKind::Gif
        } else {
            MessageKind::Video
        };
        return Some(MediaAttachment {
            kind,
            source: video,
            mime: video.mimetype.clone(),
            file_name: None,
            size: video.file_length,
            sha256: video.file_sha256.clone(),
        });
    }
    if let Some(video) = base.ptv_message.as_option() {
        // Push-to-video ("instant video message"): the same payload as a
        // regular video, only the UI treatment differs, which is not modeled
        // yet.
        return Some(MediaAttachment {
            kind: MessageKind::Video,
            source: video,
            mime: video.mimetype.clone(),
            file_name: None,
            size: video.file_length,
            sha256: video.file_sha256.clone(),
        });
    }
    if let Some(audio) = base.audio_message.as_option() {
        let kind = if audio.ptt.unwrap_or(false) {
            MessageKind::VoiceNote
        } else {
            MessageKind::Audio
        };
        return Some(MediaAttachment {
            kind,
            source: audio,
            mime: audio.mimetype.clone(),
            file_name: None,
            size: audio.file_length,
            sha256: audio.file_sha256.clone(),
        });
    }
    if let Some(document) = base.document_message.as_option() {
        return Some(MediaAttachment {
            kind: MessageKind::Document,
            source: document,
            mime: document.mimetype.clone(),
            file_name: document.file_name.clone(),
            size: document.file_length,
            sha256: document.file_sha256.clone(),
        });
    }
    if let Some(sticker) = base.sticker_message.as_option() {
        return Some(MediaAttachment {
            kind: MessageKind::Sticker,
            source: sticker,
            mime: sticker.mimetype.clone(),
            file_name: None,
            size: sticker.file_length,
            sha256: sticker.file_sha256.clone(),
        });
    }
    None
}

/// How an outgoing file maps onto the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutgoingMedia {
    /// Coarse UI kind stored on the local echo.
    pub kind: MessageKind,
    /// Upload/CDN media type.
    pub media_type: MediaType,
    /// MIME type advertised to the recipient.
    pub mime: &'static str,
    /// True for image formats that must be sent as looping video.
    pub gif_playback: bool,
    /// True when the caller asked for a push-to-video ("video note") message.
    pub video_note: bool,
    /// Clip length in whole seconds, when the container declares one.
    pub duration_seconds: Option<u32>,
}

/// Caller-selected treatment of an outgoing file, on top of what its extension
/// already implies.
#[derive(Clone, Copy, Debug, Default)]
pub struct SendFileOptions {
    /// Send a short video as a push-to-video ("video note"/PTV) message: the
    /// `VideoMessage` payload moves under `ptvMessage`, and the clip must be an
    /// ISO base-media file (`.mp4`/`.m4v`/`.mov`) no longer than
    /// [`VIDEO_NOTE_MAX_SECONDS`].
    pub video_note: bool,
}

/// Longest push-to-video ("video note") clip, matching the recorder cap in the
/// official clients. Longer clips are refused rather than silently downgraded.
pub const VIDEO_NOTE_MAX_SECONDS: u32 = 60;

/// Map a file path to the upload media type. Known image/video/audio
/// extensions select their protobuf kind; everything else is sent as a
/// document, which mirrors how messengers treat arbitrary files.
pub fn classify_outgoing(path: &Path) -> OutgoingMedia {
    let extension = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let (kind, media_type, gif_playback) = match extension.as_str() {
        "jpg" | "jpeg" | "png" | "heic" | "heif" => (MessageKind::Image, MediaType::Image, false),
        "webp" => (MessageKind::Sticker, MediaType::Sticker, false),
        "gif" => (MessageKind::Gif, MediaType::Video, true),
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" => {
            (MessageKind::Video, MediaType::Video, false)
        }
        "ogg" | "opus" | "mp3" | "m4a" | "aac" | "wav" => {
            (MessageKind::Audio, MediaType::Audio, false)
        }
        _ => (MessageKind::Document, MediaType::Document, false),
    };

    OutgoingMedia {
        kind,
        media_type,
        mime: mime_from_extension(&extension).unwrap_or("application/octet-stream"),
        gif_playback,
        video_note: false,
        duration_seconds: None,
    }
}

/// MIME type for a file extension (case-insensitive). Returns `None` for
/// extensions that are not worth advertising, so callers can fall back to
/// `application/octet-stream`.
pub fn mime_from_extension(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "heic" | "heif" => Some("image/heic"),
        "mp4" | "m4v" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        "webm" => Some("video/webm"),
        "mkv" => Some("video/x-matroska"),
        "avi" => Some("video/x-msvideo"),
        "ogg" | "opus" => Some("audio/ogg; codecs=opus"),
        "mp3" => Some("audio/mpeg"),
        "m4a" => Some("audio/mp4"),
        "aac" => Some("audio/aac"),
        "wav" => Some("audio/wav"),
        "pdf" => Some("application/pdf"),
        "doc" => Some("application/msword"),
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xls" => Some("application/vnd.ms-excel"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "ppt" => Some("application/vnd.ms-powerpoint"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        "txt" => Some("text/plain"),
        "csv" => Some("text/csv"),
        "json" => Some("application/json"),
        "html" | "htm" => Some("text/html"),
        "rtf" => Some("application/rtf"),
        "zip" => Some("application/zip"),
        "apk" => Some("application/vnd.android.package-archive"),
        _ => None,
    }
}

/// Deterministic cache file name: a SHA-256 prefix plus a sanitized original
/// name (or a MIME-derived extension), so files stay recognizable while
/// identical content reuses one entry.
///
/// Passing no digest (older or malformed payloads) prefixes `nohash`, which
/// still keeps the file inside the media directory.
pub fn cache_file_name(
    sha256: Option<&[u8]>,
    file_name: Option<&str>,
    mime: Option<&str>,
) -> String {
    let prefix = sha256
        .map(|digest| hex_prefix(digest, 16))
        .unwrap_or_else(|| "nohash".to_owned());

    let mut base = file_name.map(sanitize_file_name).unwrap_or_default();
    if base.is_empty() {
        base = "media".to_owned();
    }
    if Path::new(&base).extension().is_none()
        && let Some(extension) = mime.and_then(extension_for_mime)
    {
        base.push('.');
        base.push_str(extension);
    }
    format!("{prefix}-{base}")
}

/// Canvas size of a WebP file, for `VP8 ` (lossy), `VP8L` (lossless) and
/// `VP8X` (extended) containers. `None` when the bytes are not a WebP image
/// this build understands.
pub fn webp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 16 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }
    match &data[12..16] {
        // 3-byte frame tag + 3-byte start code, then 14-bit width/height.
        b"VP8 " if data.len() >= 30 => {
            let width = u32::from(u16::from_le_bytes([data[26], data[27]]) & 0x3fff);
            let height = u32::from(u16::from_le_bytes([data[28], data[29]]) & 0x3fff);
            (width > 0 && height > 0).then_some((width, height))
        }
        // Signature byte, then 14-bit (size - 1) pairs packed little-endian.
        b"VP8L" if data.len() >= 25 => {
            let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        // Flags + 3 reserved bytes, then 24-bit (size - 1) pairs.
        b"VP8X" if data.len() >= 30 => {
            let width = u32::from_le_bytes([data[24], data[25], data[26], 0]) + 1;
            let height = u32::from_le_bytes([data[27], data[28], data[29], 0]) + 1;
            Some((width, height))
        }
        _ => None,
    }
}

/// Duration in seconds of an ISO base-media (`.mp4`/`.m4v`/`.mov`) file, read
/// from its `moov/mvhd` box.
///
/// Upstream ships no media parser and this crate avoids a codec dependency, so
/// the walk is hand-rolled: only the 8/16-byte box headers are inspected, never
/// a payload, which keeps it cheap even when `moov` trails a multi-gigabyte
/// `mdat`. Returns `None` for containers this build cannot read.
pub fn mp4_duration_seconds(data: &[u8]) -> Option<f64> {
    let moov = iso_box_payload(data, b"moov")?;
    let mvhd = iso_box_payload(moov, b"mvhd")?;
    match mvhd.first()? {
        // version + flags, creation + modification (u32 each), timescale, duration.
        0 if mvhd.len() >= 20 => {
            let timescale = u32::from_be_bytes(mvhd[12..16].try_into().ok()?) as f64;
            let duration = u32::from_be_bytes(mvhd[16..20].try_into().ok()?) as f64;
            (timescale > 0.0).then(|| duration / timescale)
        }
        // version + flags, creation + modification (u64 each), timescale, duration.
        1 if mvhd.len() >= 32 => {
            let timescale = u32::from_be_bytes(mvhd[20..24].try_into().ok()?) as f64;
            let duration = u64::from_be_bytes(mvhd[24..32].try_into().ok()?) as f64;
            (timescale > 0.0).then(|| duration / timescale)
        }
        _ => None,
    }
}

/// Payload of the first direct-child box named `wanted`, honouring 32-bit,
/// 64-bit (`size == 1`, largesize follows the type) and to-the-end (`size == 0`)
/// box sizes. Returns `None` when the bytes run out mid-header.
fn iso_box_payload<'a>(container: &'a [u8], wanted: &[u8; 4]) -> Option<&'a [u8]> {
    let mut offset = 0usize;
    while let Some(header) = container.get(offset..offset.checked_add(8)?) {
        let size32 = u32::from_be_bytes(header[0..4].try_into().ok()?) as u64;
        let kind: &[u8; 4] = header[4..8].try_into().ok()?;
        let (size, header_len) = if size32 == 1 {
            let large = container.get(offset + 8..offset + 16)?;
            (u64::from_be_bytes(large.try_into().ok()?), 16usize)
        } else if size32 == 0 {
            // Box runs to the end of the container.
            ((container.len() - offset) as u64, 8usize)
        } else {
            (size32, 8usize)
        };
        if size < header_len as u64 {
            return None;
        }
        let end = offset.checked_add(usize::try_from(size).ok()?)?;
        let payload = container.get(offset + header_len..end)?;
        if kind == wanted {
            return Some(payload);
        }
        offset = end;
    }
    None
}

/// The media download/upload engine.
///
/// Construct one per operation (it borrows nothing, so it is cheap) with the
/// upstream client, the store and the chat's data directory; the constructor
/// appends [`MEDIA_DIR_NAME`] and creates the directory lazily. The engine
/// persists the outgoing message but leaves the [`CoreEvent`] broadcast to the
/// caller, so it stays independent of the UI event bus.
///
/// [`CoreEvent`]: crate::events::CoreEvent
pub struct MediaPipeline {
    client: Arc<Client>,
    store: Arc<dyn MediaStore>,
    media_dir: PathBuf,
    own_jid: Option<Jid>,
}

impl MediaPipeline {
    /// Bundle the protocol handle, the persistent store and the account
    /// identity into a pipeline rooted at `data_dir`.
    pub fn new(
        client: Arc<Client>,
        store: Arc<dyn MediaStore>,
        data_dir: impl Into<PathBuf>,
        own_jid: Option<Jid>,
    ) -> Self {
        Self {
            client,
            store,
            media_dir: data_dir.into().join(MEDIA_DIR_NAME),
            own_jid,
        }
    }

    /// Directory the cached binaries are written to.
    pub fn media_dir(&self) -> &Path {
        &self.media_dir
    }

    /// Download (and cache) the media attached to a message.
    ///
    /// Reuses an existing cache entry when the file is still on disk,
    /// otherwise streams the decrypted plaintext through the upstream client
    /// into `data_dir/media/<sha256-prefixed name>` (a `.part` file renamed
    /// into place) and persists the local path in the `media` table.
    pub async fn download_media(&self, message_id: &str) -> Result<MediaFile> {
        if let Some(record) = self.store.media_record(message_id)?
            && is_complete_cache_entry(&record)
        {
            return Ok(record.to_media_file());
        }

        let raw_proto = self.store.raw_proto(message_id)?.ok_or_else(|| {
            CoreError::Protocol(format!(
                "message {message_id} has no stored protobuf payload; media cannot be fetched"
            ))
        })?;
        let message = wa::Message::decode_from_slice(&raw_proto).map_err(|error| {
            CoreError::Protocol(format!(
                "stored protobuf for message {message_id} is invalid: {error}"
            ))
        })?;

        let attachment = classify_media(&message).ok_or_else(|| {
            CoreError::Protocol(format!(
                "message {message_id} has no downloadable media (expected image, video, \
                 audio/voice note, document or sticker)"
            ))
        })?;

        let file_name = cache_file_name(
            attachment.sha256.as_deref(),
            attachment.file_name.as_deref(),
            attachment.mime.as_deref(),
        );
        ensure_media_dir(&self.media_dir)?;
        let path = self.media_dir.join(&file_name);

        if !cached_file_matches(&path, attachment.size) {
            self.download_to_cache(message_id, &path, &file_name, attachment.source)
                .await?;
        }

        let size = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or_else(|_| attachment.size.unwrap_or(0));
        let record = MediaRecord {
            message_id: message_id.to_owned(),
            mime: attachment.mime,
            file_name: attachment.file_name,
            size,
            local_path: path.to_string_lossy().into_owned(),
            sha256: attachment.sha256,
            downloaded_at: now_unix(),
        };
        self.store.upsert_media_record(&record)?;
        Ok(record.to_media_file())
    }

    /// Upload a file and send it to `chat_id`, persisting the local echo and
    /// the raw protobuf exactly like `WaClient::send_text` persists its text
    /// message. Returns the stored message so the caller can emit
    /// [`CoreEvent::Message`].
    ///
    /// [`CoreEvent::Message`]: crate::events::CoreEvent::Message
    pub async fn send_file(
        &self,
        chat_id: &Jid,
        path: &str,
        caption: Option<&str>,
    ) -> Result<Message> {
        self.send_file_with(chat_id, path, caption, SendFileOptions::default())
            .await
    }

    /// Send a picked video as a push-to-video ("video note") message: the same
    /// upload/encrypt/send path as [`MediaPipeline::send_file`], but the
    /// payload travels under `ptvMessage` and the clip must be an ISO
    /// base-media file (`.mp4`/`.m4v`/`.mov`) at most
    /// [`VIDEO_NOTE_MAX_SECONDS`] seconds long.
    pub async fn send_video_note(
        &self,
        chat_id: &Jid,
        path: &str,
        caption: Option<&str>,
    ) -> Result<Message> {
        self.send_file_with(chat_id, path, caption, SendFileOptions { video_note: true })
            .await
    }

    /// [`MediaPipeline::send_file`] with caller-selected treatment (see
    /// [`SendFileOptions`]).
    pub async fn send_file_with(
        &self,
        chat_id: &Jid,
        path: &str,
        caption: Option<&str>,
        options: SendFileOptions,
    ) -> Result<Message> {
        let source = Path::new(path);
        let read_path = source.to_path_buf();
        let data = tokio::task::spawn_blocking(move || std::fs::read(&read_path))
            .await
            .map_err(|error| CoreError::Internal(format!("file read task failed: {error}")))?
            .map_err(|error| CoreError::InvalidInput(format!("cannot read {path}: {error}")))?;
        if data.is_empty() {
            return Err(CoreError::InvalidInput(format!("{path} is empty")));
        }

        let mut media = classify_outgoing(source);
        if options.video_note {
            if media.kind != MessageKind::Video {
                return Err(CoreError::InvalidInput(format!(
                    "{path} is not a video; video notes must be an MP4 or MOV clip"
                )));
            }
            let seconds = mp4_duration_seconds(&data).ok_or_else(|| {
                CoreError::InvalidInput(format!(
                    "cannot read the duration of {path}; video notes must be MP4 or MOV"
                ))
            })?;
            if seconds > f64::from(VIDEO_NOTE_MAX_SECONDS) {
                return Err(CoreError::InvalidInput(format!(
                    "video notes can be at most {VIDEO_NOTE_MAX_SECONDS} seconds long \
                     (picked {seconds:.0} s)"
                )));
            }
            media.video_note = true;
            media.duration_seconds = Some(seconds.round() as u32);
        } else if media.kind == MessageKind::Video {
            // Regular clips get their duration declared too, when readable.
            media.duration_seconds =
                mp4_duration_seconds(&data).map(|seconds| seconds.round() as u32);
        }
        let sticker = (media.kind == MessageKind::Sticker).then(|| {
            webp_dimensions(&data).map(|(width, height)| StickerDetails {
                width,
                height,
                animated: whatsapp_rust::webp::is_animated(&data),
            })
        });
        if media.kind == MessageKind::Sticker && sticker.is_none() {
            return Err(CoreError::InvalidInput(format!(
                "{path} is not a readable WebP sticker"
            )));
        }

        let upload = self
            .client
            .upload(data, media.media_type, UploadOptions::new())
            .await
            .map_err(|error| CoreError::Protocol(format!("media upload failed: {error}")))?;

        let file_name = source.file_name().and_then(std::ffi::OsStr::to_str);
        let proto = build_outgoing_message(upload, &media, sticker.flatten(), file_name, caption)?;
        let raw_proto = proto.encode_to_vec();

        let to = to_upstream_jid(chat_id)?;
        let sent = self
            .client
            .send_message(&to, proto)
            .await
            .map_err(|error| CoreError::Protocol(format!("sending media failed: {error}")))?;

        // Audio messages have no caption field in the protocol, so a caption
        // must not survive into the local echo either: the recipient never
        // sees it, and the bubble would otherwise show text that was not sent.
        let text = if matches!(media.kind, MessageKind::Audio | MessageKind::VoiceNote) {
            if caption.is_some() {
                tracing::debug!("audio messages cannot carry captions; dropping the caption");
            }
            None
        } else {
            caption.map(str::to_owned)
        };

        let message = Message {
            id: sent.message_id,
            chat_id: chat_id.clone(),
            sender_id: self
                .own_jid
                .clone()
                .unwrap_or_else(|| Jid::new(ME_PLACEHOLDER)),
            from_me: true,
            timestamp: now_unix(),
            kind: media.kind,
            text,
            status: MessageStatus::Sent,
            // This build has no outgoing view-once send surface yet.
            view_once: false,
        };

        let preview = preview_for(&message);
        self.store
            .save_outgoing_message(chat_id, &message, &preview)?;
        self.store.set_raw_proto(&message.id, &raw_proto)?;
        Ok(message)
    }

    /// Stream one verified download into `path`, via a `.part` sibling so a
    /// failed attempt never leaves a plausible cache entry behind.
    async fn download_to_cache(
        &self,
        message_id: &str,
        path: &Path,
        file_name: &str,
        source: &dyn Downloadable,
    ) -> Result<()> {
        let temp = path.with_file_name(format!("{file_name}.part"));
        let file = std::fs::File::create(&temp).map_err(|error| {
            CoreError::Storage(format!("cannot create {}: {error}", temp.display()))
        })?;
        match self.client.download_to_writer(source, file).await {
            Ok(writer) => {
                drop(writer);
                std::fs::rename(&temp, path).map_err(|error| {
                    CoreError::Storage(format!(
                        "cannot move {} into place: {error}",
                        temp.display()
                    ))
                })
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                Err(CoreError::Protocol(format!(
                    "downloading media for message {message_id} failed: {error}"
                )))
            }
        }
    }
}

/// Width, height and animation flag needed for a `StickerMessage`.
#[derive(Clone, Copy, Debug)]
struct StickerDetails {
    width: u32,
    height: u32,
    animated: bool,
}

/// Re-home a built `VideoMessage` payload under the `ptvMessage` field, which
/// is what a push-to-video ("video note") message is on the wire.
fn as_video_note(message: wa::Message) -> wa::Message {
    wa::Message {
        ptv_message: message.video_message,
        ..Default::default()
    }
}

/// Build the outgoing protobuf for one uploaded file.
fn build_outgoing_message(
    upload: whatsapp_rust::upload::UploadResponse,
    media: &OutgoingMedia,
    sticker: Option<StickerDetails>,
    file_name: Option<&str>,
    caption: Option<&str>,
) -> Result<wa::Message> {
    let mime = media.mime.to_owned();
    let message = match media.kind {
        MessageKind::Image => image_message(
            upload,
            ImageOptions {
                caption: caption.map(str::to_owned),
                mimetype: Some(mime),
                ..Default::default()
            },
        ),
        MessageKind::Video | MessageKind::Gif => {
            let message = video_message(
                upload,
                VideoOptions {
                    caption: caption.map(str::to_owned),
                    mimetype: Some(mime),
                    gif_playback: media.gif_playback.then_some(true),
                    duration_seconds: media.duration_seconds,
                    ..Default::default()
                },
            );
            if media.video_note {
                // Upstream has no dedicated `ptvMessage` builder, but a video
                // note is the same `VideoMessage` payload under a different
                // field. Re-home the builder's payload, exactly like the
                // sticker arm below hand-assembles its message.
                as_video_note(message)
            } else {
                message
            }
        }
        MessageKind::Document => document_message(
            upload,
            DocumentOptions {
                mimetype: Some(mime),
                file_name: file_name.map(str::to_owned),
                caption: caption.map(str::to_owned),
                ..Default::default()
            },
        ),
        MessageKind::Audio | MessageKind::VoiceNote => audio_message(
            upload,
            AudioOptions {
                mimetype: Some(mime),
                ..Default::default()
            },
        ),
        MessageKind::Sticker => {
            let Some(sticker) = sticker else {
                return Err(CoreError::InvalidInput(
                    "sticker upload needs readable WebP dimensions".into(),
                ));
            };
            wa::Message {
                sticker_message: MessageField::some(wa::message::StickerMessage {
                    url: Some(upload.url),
                    direct_path: Some(upload.direct_path),
                    media_key: Some(upload.media_key.to_vec()),
                    file_sha256: Some(upload.file_sha256.to_vec()),
                    file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
                    file_length: Some(upload.file_length),
                    media_key_timestamp: Some(upload.media_key_timestamp),
                    mimetype: Some(mime),
                    width: Some(sticker.width),
                    height: Some(sticker.height),
                    is_animated: Some(sticker.animated),
                    ..Default::default()
                }),
                ..Default::default()
            }
        }
        other => {
            return Err(CoreError::Internal(format!(
                "no outgoing builder for message kind {other:?}"
            )));
        }
    };
    Ok(message)
}

/// Short, human-readable chat-list preview, mirroring the one in `client.rs`.
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
        MessageKind::Gif => "[GIF]".to_owned(),
        _ => "[Message]".to_owned(),
    };
    text.chars().take(MAX).collect()
}

/// The cached file is usable when it exists and (if the record knows the size)
/// still has the recorded length; a truncated file is re-downloaded.
fn is_complete_cache_entry(record: &MediaRecord) -> bool {
    std::fs::metadata(&record.local_path)
        .map(|metadata| metadata.is_file() && (record.size == 0 || metadata.len() == record.size))
        .unwrap_or(false)
}

/// The on-disk file is usable when it exists and, when the message declares a
/// plaintext length, matches it.
fn cached_file_matches(path: &Path, expected_size: Option<u64>) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    metadata.is_file() && expected_size.is_none_or(|expected| metadata.len() == expected)
}

fn ensure_media_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|error| CoreError::Storage(format!("cannot create {}: {error}", dir.display())))
}

fn to_upstream_jid(jid: &Jid) -> Result<whatsapp_rust::Jid> {
    whatsapp_rust::Jid::from_str(jid.as_str())
        .map_err(|error| CoreError::InvalidInput(error.to_string()))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Hex-encode at most `max_chars` characters of `bytes`.
fn hex_prefix(bytes: &[u8], max_chars: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out.truncate(max_chars);
    out
}

/// Keep only the final path component and characters that are safe on every
/// filesystem; anything else becomes `_`.
fn sanitize_file_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let sanitized: String = base
        .chars()
        .take(80)
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    sanitized
        .trim_matches(|ch: char| ch == '.' || ch == ' ')
        .to_owned()
}

/// Extension to append when a payload has no file name to borrow one from.
fn extension_for_mime(mime: &str) -> Option<&'static str> {
    let mime = mime.split(';').next().unwrap_or(mime).trim();
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/heic" => Some("heic"),
        "video/mp4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "video/webm" => Some("webm"),
        "audio/ogg" => Some("ogg"),
        "audio/mpeg" => Some("mp3"),
        "audio/mp4" => Some("m4a"),
        "audio/aac" => Some("aac"),
        "audio/wav" => Some("wav"),
        "application/pdf" => Some("pdf"),
        _ => None,
    }
}

impl WaClient {
    /// Download (and cache) the media attached to a message.
    ///
    /// The work lives in [`MediaPipeline::download_media`]; once the follow-up
    /// in the module docs lands, this body becomes:
    ///
    /// ```ignore
    /// let pipeline = MediaPipeline::new(
    ///     self.client().await?,
    ///     self.store(),
    ///     self.data_dir(),
    ///     self.own_jid(),
    /// );
    /// pipeline.download_media(message_id).await
    /// ```
    pub async fn download_media(&self, message_id: &str) -> Result<MediaFile> {
        let pipeline = MediaPipeline::new(
            self.client().await?,
            self.store(),
            self.data_dir(),
            self.own_jid(),
        );
        pipeline.download_media(message_id).await
    }

    /// Send a file from disk to a chat, with an optional caption.
    ///
    /// Wraps [`MediaPipeline::send_file`] and emits the stored message as
    /// [`CoreEvent::Message`], so the UI can append the local echo.
    pub async fn send_file(&self, chat_id: &Jid, path: &str, caption: Option<&str>) -> Result<()> {
        self.send_file_with(chat_id, path, caption, SendFileOptions::default())
            .await
    }

    /// Send a picked short video as a push-to-video ("video note") message.
    ///
    /// Mirrors [`WaClient::send_file`]; the pipeline rejects non-MP4/MOV input
    /// and clips longer than [`VIDEO_NOTE_MAX_SECONDS`].
    pub async fn send_video_note(
        &self,
        chat_id: &Jid,
        path: &str,
        caption: Option<&str>,
    ) -> Result<()> {
        self.send_file_with(chat_id, path, caption, SendFileOptions { video_note: true })
            .await
    }

    /// Shared body of [`WaClient::send_file`] and [`WaClient::send_video_note`].
    async fn send_file_with(
        &self,
        chat_id: &Jid,
        path: &str,
        caption: Option<&str>,
        options: SendFileOptions,
    ) -> Result<()> {
        let pipeline = MediaPipeline::new(
            self.client().await?,
            self.store(),
            self.data_dir(),
            self.own_jid(),
        );
        let message = pipeline
            .send_file_with(chat_id, path, caption, options)
            .await?;
        self.emit(CoreEvent::Message(message));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_proto() -> wa::Message {
        wa::Message {
            image_message: MessageField::some(wa::message::ImageMessage {
                direct_path: Some("/v/t62.7118-24/image".to_owned()),
                media_key: Some(vec![7u8; 32]),
                file_sha256: Some(vec![9u8; 32]),
                file_enc_sha256: Some(vec![8u8; 32]),
                file_length: Some(4096),
                mimetype: Some("image/jpeg".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn mime_detection_is_case_insensitive_and_conservative() {
        assert_eq!(mime_from_extension("jpg"), Some("image/jpeg"));
        assert_eq!(mime_from_extension("JPEG"), Some("image/jpeg"));
        assert_eq!(mime_from_extension("ogg"), Some("audio/ogg; codecs=opus"));
        assert_eq!(mime_from_extension("pdf"), Some("application/pdf"));
        assert_eq!(mime_from_extension("exe"), None);
        assert_eq!(mime_from_extension(""), None);
    }

    #[test]
    fn cache_names_are_prefixed_and_sanitized() {
        let digest = [0xabu8; 32];
        assert_eq!(
            cache_file_name(Some(&digest), Some("../../etc/passwd"), None),
            "abababababababab-passwd"
        );
        assert_eq!(
            cache_file_name(Some(&digest), Some("IMG 0421"), Some("image/jpeg")),
            "abababababababab-IMG 0421.jpg"
        );
        assert_eq!(
            cache_file_name(Some(&[0x01, 0x23]), Some("report"), Some("application/pdf")),
            "0123-report.pdf"
        );
        assert_eq!(
            cache_file_name(None, None, Some("video/mp4")),
            "nohash-media.mp4"
        );
    }

    #[test]
    fn classifies_images_and_unwraps_view_once() {
        let message = image_proto();
        let attachment = classify_media(&message).expect("image");
        assert_eq!(attachment.kind, MessageKind::Image);
        assert_eq!(attachment.source.app_info(), MediaType::Image);
        assert_eq!(
            attachment.source.direct_path(),
            Some("/v/t62.7118-24/image")
        );
        assert_eq!(attachment.mime.as_deref(), Some("image/jpeg"));
        assert_eq!(attachment.size, Some(4096));
        assert_eq!(attachment.sha256.as_deref(), Some(&[9u8; 32][..]));
        assert!(!is_view_once(&message));

        let wrapped = wa::Message {
            view_once_message: MessageField::some(wa::message::FutureProofMessage {
                message: MessageField::some(message),
            }),
            ..Default::default()
        };
        assert!(is_view_once(&wrapped));
        assert_eq!(
            classify_media(&wrapped).expect("view once image").kind,
            MessageKind::Image
        );
    }

    #[test]
    fn detects_every_view_once_wrapper() {
        let inner = image_proto();

        let wrapper = |message: wa::Message| wa::message::FutureProofMessage {
            message: MessageField::some(message),
        };

        let v1 = wa::Message {
            view_once_message: MessageField::some(wrapper(inner.clone())),
            ..Default::default()
        };
        let v2 = wa::Message {
            view_once_message_v2: MessageField::some(wrapper(inner.clone())),
            ..Default::default()
        };
        let v2_extension = wa::Message {
            view_once_message_v2_extension: MessageField::some(wrapper(inner.clone())),
            ..Default::default()
        };

        for message in [&v1, &v2, &v2_extension] {
            assert!(is_view_once(message));
            let attachment = classify_media(message).expect("view once image");
            assert_eq!(attachment.kind, MessageKind::Image);
            assert_eq!(attachment.source.app_info(), MediaType::Image);
        }

        // A view-once envelope can itself be nested under the multi-device
        // `deviceSentMessage` wrapper; detection must see through it.
        let nested = wa::Message {
            device_sent_message: MessageField::some(wa::message::DeviceSentMessage {
                message: MessageField::some(wa::Message {
                    view_once_message_v2: MessageField::some(wrapper(inner.clone())),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(is_view_once(&nested));
        assert_eq!(
            classify_media(&nested)
                .expect("nested view once image")
                .kind,
            MessageKind::Image
        );

        // Non-media messages stay view-once-free.
        let text = wa::Message {
            conversation: Some("hello".to_owned()),
            ..Default::default()
        };
        assert!(!is_view_once(&text));
    }

    #[test]
    fn detects_inline_view_once_flag_on_media_payloads() {
        // Modern clients mark the payload itself instead of wrapping it.
        let inline = wa::Message {
            image_message: MessageField::some(wa::message::ImageMessage {
                view_once: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(is_view_once(&inline));

        let plain = wa::Message {
            image_message: MessageField::some(wa::message::ImageMessage {
                view_once: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!is_view_once(&plain));
    }

    #[test]
    fn classifies_video_gif_and_ptv() {
        let video = wa::Message {
            video_message: MessageField::some(wa::message::VideoMessage {
                direct_path: Some("/v/video".to_owned()),
                media_key: Some(vec![2u8; 32]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let attachment = classify_media(&video).expect("video");
        assert_eq!(attachment.kind, MessageKind::Video);
        assert_eq!(attachment.source.app_info(), MediaType::Video);

        let gif = wa::Message {
            video_message: MessageField::some(wa::message::VideoMessage {
                direct_path: Some("/v/video".to_owned()),
                media_key: Some(vec![2u8; 32]),
                gif_playback: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(classify_media(&gif).expect("gif").kind, MessageKind::Gif);

        let ptv = wa::Message {
            ptv_message: MessageField::some(wa::message::VideoMessage {
                direct_path: Some("/v/ptv".to_owned()),
                media_key: Some(vec![3u8; 32]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let attachment = classify_media(&ptv).expect("ptv");
        assert_eq!(attachment.kind, MessageKind::Video);
        assert_eq!(attachment.source.app_info(), MediaType::Video);
    }

    #[test]
    fn classifies_voice_notes_and_regular_audio() {
        let voice_note = wa::Message {
            audio_message: MessageField::some(wa::message::AudioMessage {
                direct_path: Some("/v/audio".to_owned()),
                media_key: Some(vec![1u8; 32]),
                mimetype: Some("audio/ogg; codecs=opus".to_owned()),
                ptt: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let attachment = classify_media(&voice_note).expect("voice note");
        assert_eq!(attachment.kind, MessageKind::VoiceNote);
        assert_eq!(attachment.source.app_info(), MediaType::Audio);

        let audio = wa::Message {
            audio_message: MessageField::some(wa::message::AudioMessage {
                direct_path: Some("/v/audio".to_owned()),
                media_key: Some(vec![1u8; 32]),
                mimetype: Some("audio/ogg; codecs=opus".to_owned()),
                ptt: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            classify_media(&audio).expect("audio").kind,
            MessageKind::Audio
        );
    }

    #[test]
    fn classifies_documents_and_stickers() {
        let document = wa::Message {
            document_message: MessageField::some(wa::message::DocumentMessage {
                direct_path: Some("/v/doc".to_owned()),
                media_key: Some(vec![4u8; 32]),
                file_name: Some("notes.pdf".to_owned()),
                mimetype: Some("application/pdf".to_owned()),
                file_length: Some(128),
                ..Default::default()
            }),
            ..Default::default()
        };
        let attachment = classify_media(&document).expect("document");
        assert_eq!(attachment.kind, MessageKind::Document);
        assert_eq!(attachment.file_name.as_deref(), Some("notes.pdf"));

        let sticker = wa::Message {
            sticker_message: MessageField::some(wa::message::StickerMessage {
                direct_path: Some("/v/sticker".to_owned()),
                media_key: Some(vec![5u8; 32]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let attachment = classify_media(&sticker).expect("sticker");
        assert_eq!(attachment.kind, MessageKind::Sticker);
        assert_eq!(attachment.source.app_info(), MediaType::Sticker);
    }

    #[test]
    fn rejects_messages_without_downloadable_media() {
        assert!(classify_media(&wa::Message::default()).is_none());
        let text = wa::Message {
            conversation: Some("hello".to_owned()),
            ..Default::default()
        };
        assert!(classify_media(&text).is_none());
    }

    #[test]
    fn classifies_outgoing_files_by_extension() {
        let image = classify_outgoing(Path::new("/tmp/photo.JPG"));
        assert_eq!(image.kind, MessageKind::Image);
        assert_eq!(image.media_type, MediaType::Image);
        assert_eq!(image.mime, "image/jpeg");
        assert!(!image.gif_playback);

        let gif = classify_outgoing(Path::new("/tmp/loop.gif"));
        assert_eq!(gif.kind, MessageKind::Gif);
        assert_eq!(gif.media_type, MediaType::Video);
        assert_eq!(gif.mime, "image/gif");
        assert!(gif.gif_playback);

        let sticker = classify_outgoing(Path::new("/tmp/cat.webp"));
        assert_eq!(sticker.kind, MessageKind::Sticker);
        assert_eq!(sticker.media_type, MediaType::Sticker);

        let document = classify_outgoing(Path::new("/tmp/archive.unknownext"));
        assert_eq!(document.kind, MessageKind::Document);
        assert_eq!(document.media_type, MediaType::Document);
        assert_eq!(document.mime, "application/octet-stream");
    }

    #[test]
    fn media_record_projects_to_media_file() {
        let record = MediaRecord {
            message_id: "m1".to_owned(),
            mime: Some("image/jpeg".to_owned()),
            file_name: Some("photo.jpg".to_owned()),
            size: 42,
            local_path: "/tmp/media/ab-photo.jpg".to_owned(),
            sha256: Some(vec![0xab; 32]),
            downloaded_at: 1_700_000_000,
        };
        let file = record.to_media_file();
        assert_eq!(file.path, record.local_path);
        assert_eq!(file.mime.as_deref(), Some("image/jpeg"));
        assert_eq!(file.file_name.as_deref(), Some("photo.jpg"));
        assert_eq!(file.size, 42);
    }

    fn webp_vp8(width: u16, height: u16) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(b"VP8 ");
        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(&[0, 0, 0]);
        data.extend_from_slice(&[0x9d, 0x01, 0x2a]);
        data.extend_from_slice(&width.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data
    }

    #[test]
    fn reads_webp_dimensions_for_all_containers() {
        assert_eq!(webp_dimensions(&webp_vp8(512, 256)), Some((512, 256)));

        let mut lossless = Vec::new();
        lossless.extend_from_slice(b"RIFF");
        lossless.extend_from_slice(&[0, 0, 0, 0]);
        lossless.extend_from_slice(b"WEBP");
        lossless.extend_from_slice(b"VP8L");
        lossless.extend_from_slice(&[0, 0, 0, 0]);
        lossless.push(0x2f);
        let bits = 511u32 | (255u32 << 14);
        lossless.extend_from_slice(&bits.to_le_bytes());
        assert_eq!(webp_dimensions(&lossless), Some((512, 256)));

        let mut extended = Vec::new();
        extended.extend_from_slice(b"RIFF");
        extended.extend_from_slice(&[0, 0, 0, 0]);
        extended.extend_from_slice(b"WEBP");
        extended.extend_from_slice(b"VP8X");
        extended.extend_from_slice(&[0, 0, 0, 0]);
        extended.push(0);
        extended.extend_from_slice(&[0, 0, 0]);
        extended.extend_from_slice(&[511u32.to_le_bytes()[0], 1, 0]);
        extended.extend_from_slice(&[255u32.to_le_bytes()[0], 0, 0]);
        assert_eq!(webp_dimensions(&extended), Some((512, 256)));

        assert_eq!(webp_dimensions(b"not a webp file at all"), None);
    }

    /// Minimal ISO base-media box: 32-bit size, four-byte type, payload.
    fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut boxed = ((8 + payload.len()) as u32).to_be_bytes().to_vec();
        boxed.extend_from_slice(kind);
        boxed.extend_from_slice(payload);
        boxed
    }

    /// An `ftyp`+`moov(mvhd)` file with the given movie timescale/duration.
    fn mp4_with_mvhd(timescale: u32, duration: u64, version_one: bool) -> Vec<u8> {
        let mut mvhd = vec![if version_one { 1 } else { 0 }, 0, 0, 0];
        if version_one {
            mvhd.extend_from_slice(&[0u8; 16]); // creation + modification (u64)
            mvhd.extend_from_slice(&timescale.to_be_bytes());
            mvhd.extend_from_slice(&duration.to_be_bytes());
        } else {
            mvhd.extend_from_slice(&[0u8; 8]); // creation + modification (u32)
            mvhd.extend_from_slice(&timescale.to_be_bytes());
            mvhd.extend_from_slice(&(duration as u32).to_be_bytes());
        }
        let mut file = mp4_box(b"ftyp", b"isom");
        file.extend_from_slice(&mp4_box(b"moov", &mp4_box(b"mvhd", &mvhd)));
        file
    }

    #[test]
    fn reads_mp4_duration_from_mvhd() {
        assert_eq!(
            mp4_duration_seconds(&mp4_with_mvhd(1000, 4500, false)),
            Some(4.5)
        );
        assert_eq!(
            mp4_duration_seconds(&mp4_with_mvhd(600, 12_000, true)),
            Some(20.0)
        );
        // 64-bit (`size == 1`) boxes are walked too: rebuild `moov` that way.
        let mvhd = mp4_box(b"mvhd", &{
            let mut payload = vec![0, 0, 0, 0];
            payload.extend_from_slice(&[0u8; 8]);
            payload.extend_from_slice(&1000u32.to_be_bytes());
            payload.extend_from_slice(&90_000u32.to_be_bytes());
            payload
        });
        let mut large_moov = 1u32.to_be_bytes().to_vec();
        large_moov.extend_from_slice(b"moov");
        large_moov.extend_from_slice(&((16 + mvhd.len()) as u64).to_be_bytes());
        large_moov.extend_from_slice(&mvhd);
        assert_eq!(mp4_duration_seconds(&large_moov), Some(90.0));

        assert_eq!(mp4_duration_seconds(b"not an mp4 at all"), None);
        // A zero timescale cannot be divided.
        assert_eq!(mp4_duration_seconds(&mp4_with_mvhd(0, 4500, false)), None);
    }

    #[test]
    fn rehomes_video_payloads_under_the_ptv_field() {
        let video = wa::Message {
            video_message: MessageField::some(wa::message::VideoMessage {
                direct_path: Some("/v/ptv".to_owned()),
                media_key: Some(vec![3u8; 32]),
                mimetype: Some("video/mp4".to_owned()),
                seconds: Some(12),
                streaming_sidecar: Some(vec![9, 9, 9]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let ptv = as_video_note(video);
        assert!(!ptv.video_message.is_set());
        let inner = ptv.ptv_message.as_option().expect("ptv payload");
        assert_eq!(inner.mimetype.as_deref(), Some("video/mp4"));
        assert_eq!(inner.seconds, Some(12));
        assert_eq!(inner.streaming_sidecar.as_deref(), Some(&[9u8, 9, 9][..]));
        // The inbound classifier must recognise what we just built.
        assert_eq!(
            classify_media(&ptv).expect("ptv classifies").kind,
            MessageKind::Video
        );
    }
}
