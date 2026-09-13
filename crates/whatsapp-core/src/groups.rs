//! Groups: creation, metadata, participant management, invite links.

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::types::Jid;
use serde::Serialize;

/// Public information about a group.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    /// Group JID.
    pub id: Jid,
    /// Group subject.
    pub subject: String,
    /// Participant JIDs.
    pub participants: Vec<Jid>,
    /// Participant count.
    pub participant_count: u32,
}

impl WaClient {
    /// Create a group with the given subject and initial participants.
    pub async fn create_group(&self, _subject: &str, _participants: &[Jid]) -> Result<Jid> {
        Err(CoreError::Internal(
            "group support is not implemented yet".into(),
        ))
    }

    /// Fetch cached group metadata.
    pub async fn group_info(&self, _chat_id: &Jid) -> Result<GroupInfo> {
        Err(CoreError::Internal(
            "group support is not implemented yet".into(),
        ))
    }

    /// Add participants to a group.
    pub async fn add_participants(&self, _chat_id: &Jid, _participants: &[Jid]) -> Result<()> {
        Err(CoreError::Internal(
            "group support is not implemented yet".into(),
        ))
    }

    /// Remove participants from a group.
    pub async fn remove_participants(&self, _chat_id: &Jid, _participants: &[Jid]) -> Result<()> {
        Err(CoreError::Internal(
            "group support is not implemented yet".into(),
        ))
    }

    /// Leave a group.
    pub async fn leave_group(&self, _chat_id: &Jid) -> Result<()> {
        Err(CoreError::Internal(
            "group support is not implemented yet".into(),
        ))
    }

    /// Fetch (or reset) the group's invite link.
    pub async fn invite_link(&self, _chat_id: &Jid) -> Result<String> {
        Err(CoreError::Internal(
            "group support is not implemented yet".into(),
        ))
    }
}
