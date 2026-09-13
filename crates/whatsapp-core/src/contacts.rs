//! Contacts: profile metadata, LID mappings and profile pictures.
//!
//! WhatsApp addresses people by phone-number JIDs (PN) or newer linked IDs
//! (LID). This module resolves both into display metadata the UI can use, and
//! exposes profile-picture URLs.

use serde::Serialize;

use crate::client::{WaClient, from_upstream, to_upstream};
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// Resolved metadata for one contact.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactProfile {
    /// The JID that was queried.
    pub jid: Jid,
    /// The contact's LID, when the server reports one.
    pub lid: Option<Jid>,
    /// About/status text, when visible.
    pub about: Option<String>,
    /// Verified business name, when applicable.
    pub verified_name: Option<String>,
    /// Picture id (changes when the picture changes).
    pub picture_id: Option<String>,
    /// Direct profile-picture URL, when the contact has one and we may see it.
    pub avatar_url: Option<String>,
}

impl WaClient {
    /// Resolve display metadata for a set of contacts.
    pub async fn resolve_contacts(&self, jids: &[Jid]) -> Result<Vec<ContactProfile>> {
        if jids.is_empty() {
            return Ok(Vec::new());
        }
        let client = self.client().await?;
        let upstream: Vec<whatsapp_rust::Jid> =
            jids.iter().map(to_upstream).collect::<Result<Vec<_>>>()?;

        let info = client
            .contacts()
            .get_user_info(&upstream)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        let mut profiles = Vec::with_capacity(info.len());
        for (upstream_jid, user) in info {
            profiles.push(ContactProfile {
                jid: from_upstream(&upstream_jid),
                lid: user.lid.as_ref().map(from_upstream),
                about: user.status,
                verified_name: user.verified_name.and_then(|verified| verified.name),
                picture_id: user.picture_id,
                avatar_url: None,
            });
        }
        Ok(profiles)
    }

    /// Fetch the profile-picture URL for a contact or group, if any.
    pub async fn avatar_url(&self, jid: &Jid) -> Result<Option<String>> {
        let client = self.client().await?;
        let target = to_upstream(jid)?;
        let picture = client
            .contacts()
            .get_profile_picture(&target, false)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(picture.map(|picture| picture.url))
    }
}
