//! Media pipeline: download and cache of inbound media, upload of outbound
//! media. Binaries live in the cache directory; the store keeps metadata.
//!
//! This module extends [`WaClient`] with the media surface.

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::types::Jid;
use serde::Serialize;

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

impl WaClient {
    /// Download (and cache) the media attached to a message.
    pub async fn download_media(&self, _message_id: &str) -> Result<MediaFile> {
        Err(CoreError::Internal(
            "the media pipeline is not implemented yet".into(),
        ))
    }

    /// Send a file from disk to a chat, with an optional caption.
    pub async fn send_file(
        &self,
        _chat_id: &Jid,
        _path: &str,
        _caption: Option<&str>,
    ) -> Result<()> {
        Err(CoreError::Internal(
            "the media pipeline is not implemented yet".into(),
        ))
    }
}
