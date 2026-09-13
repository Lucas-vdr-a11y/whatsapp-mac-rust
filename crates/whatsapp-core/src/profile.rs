//! The account's own profile: display name (push name), about text and
//! profile picture.
//!
//! Upstream (`whatsapp_rust::features::profile`) exposes the setters plus a
//! push-name read path (`Client::push_name`). There is no upstream getter for
//! the about text or the picture URL, so this module reuses the same
//! contacts/usync helpers the UI already uses for other people
//! (`WaClient::resolve_contacts` / `WaClient::avatar_url`); both are live
//! server queries, not cached state.

use std::path::Path;

use serde::Serialize;

use crate::client::{WaClient, from_upstream};
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// Longest push name WhatsApp Web accepts (characters, not bytes).
///
/// Upstream only rejects an empty name; the limit is the protocol's and is
/// enforced here so the server does not reject the update.
pub const PUSH_NAME_MAX_CHARS: usize = 25;

/// Longest about/status text WhatsApp Web accepts (characters, not bytes).
///
/// Upstream forwards the text as-is; this is the protocol's limit.
pub const ABOUT_MAX_CHARS: usize = 139;

/// Local guard against handing absurd files to the protocol layer. WhatsApp
/// profile pictures are 640×640 JPEGs far below this bound; upstream does not
/// resize or validate, so oversized input would be uploaded verbatim.
pub const PROFILE_PICTURE_MAX_BYTES: usize = 5 * 1024 * 1024;

/// The account's own profile, assembled from local state plus two server
/// queries (about/status and picture URL).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnProfile {
    /// Phone-number JID of the account, when the session knows it.
    pub jid: Option<Jid>,
    /// Display name peers see (upstream `Client::push_name`). `None` until a
    /// sync or a successful edit has delivered one.
    pub push_name: Option<String>,
    /// About/status text as the server reports it (`None` when unset).
    pub about: Option<String>,
    /// Direct profile-picture URL, when the account has a picture.
    pub avatar_url: Option<String>,
}

impl WaClient {
    /// Resolve the account's own profile.
    ///
    /// The push name comes from the local device snapshot (no round trip);
    /// the about text and picture URL are two usync/profile-picture queries
    /// against the server. Without a known own JID the remote fields are
    /// `None`.
    pub async fn own_profile(&self) -> Result<OwnProfile> {
        let client = self.client().await?;

        // The in-memory JID slot is filled by the pairing event; a session
        // restored from disk reconnects without it, so fall back to the
        // persistence snapshot (PN first, LID second).
        let own_jid = self
            .own_jid()
            .or_else(|| client.pn().map(|jid| from_upstream(&jid)))
            .or_else(|| client.lid().map(|jid| from_upstream(&jid)));

        let push_name = {
            let name = client.push_name();
            let trimmed = name.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        };

        let (about, avatar_url) = match own_jid.as_ref() {
            Some(jid) => {
                let about = self
                    .resolve_contacts(std::slice::from_ref(jid))
                    .await?
                    .into_iter()
                    .next()
                    .and_then(|profile| profile.about);
                (about, self.avatar_url(jid).await?)
            }
            None => (None, None),
        };

        Ok(OwnProfile {
            jid: own_jid,
            push_name,
            about,
            avatar_url,
        })
    }

    /// Change the account's display name (push name), synced across devices.
    ///
    /// The name is trimmed; empty names and names longer than
    /// [`PUSH_NAME_MAX_CHARS`] are rejected before anything reaches the wire.
    pub async fn set_push_name(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::InvalidInput("display name is empty".into()));
        }
        let length = name.chars().count();
        if length > PUSH_NAME_MAX_CHARS {
            return Err(CoreError::InvalidInput(format!(
                "display name is {length} characters; the maximum is {PUSH_NAME_MAX_CHARS}"
            )));
        }

        let client = self.client().await?;
        client
            .profile()
            .set_push_name(name)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Change the account's about/status text. Empty text clears it.
    ///
    /// The text is trimmed; text longer than [`ABOUT_MAX_CHARS`] is rejected
    /// before anything reaches the wire.
    pub async fn set_about(&self, text: &str) -> Result<()> {
        let text = text.trim();
        let length = text.chars().count();
        if length > ABOUT_MAX_CHARS {
            return Err(CoreError::InvalidInput(format!(
                "about text is {length} characters; the maximum is {ABOUT_MAX_CHARS}"
            )));
        }

        let client = self.client().await?;
        client
            .profile()
            .set_status_text(text)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Replace the account's profile picture with a JPEG file on disk.
    ///
    /// Upstream sends the bytes verbatim: it neither resizes nor re-encodes,
    /// so the caller must supply a JPEG (WhatsApp Web crops to 640×640 before
    /// upload; that step is out of scope here). Empty, oversized and
    /// non-JPEG files are rejected locally.
    pub async fn set_profile_picture(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|error| {
            CoreError::InvalidInput(format!("could not read {}: {error}", path.display()))
        })?;
        if data.is_empty() {
            return Err(CoreError::InvalidInput("the picture file is empty".into()));
        }
        if data.len() > PROFILE_PICTURE_MAX_BYTES {
            return Err(CoreError::InvalidInput(format!(
                "the picture is larger than {} MiB",
                PROFILE_PICTURE_MAX_BYTES / (1024 * 1024)
            )));
        }
        // JPEG files start with SOI (FF D8) followed by a marker (FF …).
        if !data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Err(CoreError::InvalidInput(
                "profile pictures must be JPEG files".into(),
            ));
        }

        let client = self.client().await?;
        client
            .profile()
            .set_profile_picture(data)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Remove the account's profile picture.
    pub async fn remove_profile_picture(&self) -> Result<()> {
        let client = self.client().await?;
        client
            .profile()
            .remove_profile_picture()
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }
}
