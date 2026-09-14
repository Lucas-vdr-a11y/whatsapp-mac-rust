//! Groups: creation, metadata, participant and join-request management,
//! invite links and group settings.

use serde::Serialize;
use whatsapp_rust::{
    GroupCreateOptions, GroupDescription, GroupMetadata, GroupParticipantOptions, GroupSubject,
    PreviousDescription,
};

use crate::client::{WaClient, from_upstream, to_upstream};
use crate::error::{CoreError, Result};
use crate::types::Jid;

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

/// Public information about a pending join (membership approval) request.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    /// Requesting user's JID.
    pub id: Jid,
    /// Unix seconds when the request was created, when the server reports it.
    pub requested_at: Option<u64>,
}

impl WaClient {
    /// Create a group with the given subject and initial participants.
    ///
    /// Returns the JID of the newly created group.
    pub async fn create_group(&self, subject: &str, participants: &[Jid]) -> Result<Jid> {
        let client = self.client().await?;
        let upstream = upstream_jids(participants)?;
        let options = GroupCreateOptions::new(subject).with_participants(
            upstream
                .into_iter()
                .map(GroupParticipantOptions::new)
                .collect(),
        );

        let created = client
            .groups()
            .create_group(options)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        Ok(from_upstream(&created.metadata.id))
    }

    /// Fetch the group's metadata from the server.
    ///
    /// Always hits the network (the upstream metadata cache is deliberately
    /// bypassed); `participants` are the JIDs the server reports.
    pub async fn group_info(&self, chat_id: &Jid) -> Result<GroupInfo> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        let metadata = client
            .groups()
            .get_metadata(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        Ok(group_info_from_metadata(&metadata))
    }

    /// Add participants to a group.
    pub async fn add_participants(&self, chat_id: &Jid, participants: &[Jid]) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        let upstream = upstream_jids(participants)?;

        client
            .groups()
            .add_participants(&jid, &upstream)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Remove participants from a group.
    pub async fn remove_participants(&self, chat_id: &Jid, participants: &[Jid]) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        let upstream = upstream_jids(participants)?;

        client
            .groups()
            .remove_participants(&jid, &upstream)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Leave a group.
    pub async fn leave_group(&self, chat_id: &Jid) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        client
            .groups()
            .leave(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Fetch the group's current invite link without resetting it.
    pub async fn invite_link(&self, chat_id: &Jid) -> Result<String> {
        self.group_invite_link(chat_id, false).await
    }

    /// Reset the group's invite link, invalidating the previous one, and
    /// return the new link.
    pub async fn reset_invite_link(&self, chat_id: &Jid) -> Result<String> {
        self.group_invite_link(chat_id, true).await
    }

    /// Set the group subject.
    pub async fn set_group_subject(&self, chat_id: &Jid, subject: &str) -> Result<()> {
        let subject = subject.trim();
        if subject.is_empty() {
            return Err(CoreError::InvalidInput("group subject is empty".into()));
        }
        let subject = GroupSubject::new(subject)
            .map_err(|error| CoreError::InvalidInput(error.to_string()))?;

        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        client
            .groups()
            .set_subject(&jid, subject)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Set the group description; an empty `text` deletes the current one.
    ///
    /// The upstream replaces the description optimistically: it resolves the
    /// current description id first, so a concurrent update surfaces as a
    /// protocol error instead of silently overwriting it.
    pub async fn set_group_description(&self, chat_id: &Jid, text: &str) -> Result<()> {
        let text = text.trim();
        let description = if text.is_empty() {
            None
        } else {
            Some(
                GroupDescription::new(text)
                    .map_err(|error| CoreError::InvalidInput(error.to_string()))?,
            )
        };

        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        client
            .groups()
            .set_description(&jid, description, PreviousDescription::Resolve)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// List the group's pending join (membership approval) requests.
    pub async fn pending_participants(&self, chat_id: &Jid) -> Result<Vec<JoinRequest>> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        let requests = client
            .groups()
            .get_membership_requests(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        Ok(requests
            .into_iter()
            .map(|request| JoinRequest {
                id: from_upstream(&request.jid),
                requested_at: request.request_time,
            })
            .collect())
    }

    /// Approve pending join requests, adding the requesters to the group.
    pub async fn approve_participants(&self, chat_id: &Jid, participants: &[Jid]) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        let upstream = upstream_jids(participants)?;

        client
            .groups()
            .approve_membership_requests(&jid, &upstream)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Reject pending join requests.
    pub async fn reject_participants(&self, chat_id: &Jid, participants: &[Jid]) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;
        let upstream = upstream_jids(participants)?;

        client
            .groups()
            .reject_membership_requests(&jid, &upstream)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Shared invite-link fetch; `reset` invalidates the previous link.
    async fn group_invite_link(&self, chat_id: &Jid, reset: bool) -> Result<String> {
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        client
            .groups()
            .get_invite_link(&jid, reset)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }
}

/// Project upstream metadata into the stable UI type.
///
/// Participants are converted from upstream [`GroupParticipant`] structs to
/// their primary JIDs. In LID-addressed groups those are LID JIDs; the upstream
/// metadata also backfills `phone_number` mappings, which this view does not
/// carry yet.
fn group_info_from_metadata(metadata: &GroupMetadata) -> GroupInfo {
    let participants: Vec<Jid> = metadata
        .participants
        .iter()
        .map(|participant| from_upstream(&participant.jid))
        .collect();

    GroupInfo {
        id: from_upstream(&metadata.id),
        subject: metadata.subject.clone(),
        // `size` is the server's authoritative count; fall back to the listed
        // participants when the server omitted it.
        participant_count: metadata
            .size
            .unwrap_or_else(|| u32::try_from(participants.len()).unwrap_or(u32::MAX)),
        participants,
    }
}

fn upstream_jids(jids: &[Jid]) -> Result<Vec<whatsapp_rust::Jid>> {
    jids.iter().map(to_upstream).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::{GroupParticipant, ParticipantType};

    fn participant(user: &str) -> GroupParticipant {
        GroupParticipant {
            jid: format!("{user}@s.whatsapp.net")
                .parse()
                .expect("valid participant jid"),
            phone_number: None,
            lid: None,
            username: None,
            participant_type: ParticipantType::Member,
            details: None,
        }
    }

    #[test]
    fn converts_participants_to_jids() {
        let metadata = GroupMetadata {
            id: "123456789@g.us".parse().expect("valid group jid"),
            subject: "RustWA".to_owned(),
            participants: vec![participant("15551234567"), participant("15557654321")],
            ..Default::default()
        };

        let info = group_info_from_metadata(&metadata);

        assert_eq!(info.id, Jid::new("123456789@g.us"));
        assert_eq!(info.subject, "RustWA");
        assert_eq!(
            info.participants,
            vec![
                Jid::new("15551234567@s.whatsapp.net"),
                Jid::new("15557654321@s.whatsapp.net"),
            ]
        );
        assert_eq!(info.participant_count, 2);
    }

    #[test]
    fn prefers_server_size_over_listed_participants() {
        let metadata = GroupMetadata {
            participants: vec![participant("15551234567")],
            size: Some(42),
            ..Default::default()
        };

        let info = group_info_from_metadata(&metadata);

        assert_eq!(info.participant_count, 42);
        assert_eq!(info.participants.len(), 1);
    }

    #[test]
    fn converts_local_jid_lists_to_upstream() {
        let jids = upstream_jids(&[
            Jid::new("15551234567@s.whatsapp.net"),
            Jid::new("15557654321@s.whatsapp.net"),
        ])
        .expect("converts");

        assert_eq!(
            jids,
            vec![
                whatsapp_rust::Jid::pn("15551234567"),
                whatsapp_rust::Jid::pn("15557654321"),
            ]
        );
    }

    #[test]
    fn rejects_unparseable_jids_as_invalid_input() {
        let result = upstream_jids(&[Jid::new("not-a-jid")]);
        assert!(matches!(result, Err(CoreError::InvalidInput(_))));
    }
}
