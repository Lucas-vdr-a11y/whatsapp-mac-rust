//! Communities: parent groups with linked subgroups.
//!
//! Upstream exposes the community feature through `Client::community()`
//! (`whatsapp-rust-0.7.0/src/features/community.rs`): community creation and
//! deletion, subgroup (un)linking, MEX (GraphQL) subgroup queries, and
//! linked-group joins. A community parent *is* a group, so its name,
//! description, and invite link come from the regular group surface
//! (`src/features/groups.rs`); this module composes both views into the
//! stable [`WaClient`] API used by the desktop host.
//!
//! Not every community operation upstream offers is wrapped here yet
//! (`create_subgroup`, `remove_participants`, `join_subgroup`,
//! `query_linked_group`, `get_participating`, per-subgroup participant
//! counts). Callers that need them should surface a requirement instead of
//! inventing protocol code.

use serde::Serialize;
use whatsapp_rust::wacore::iq::groups::{GROUP_DESCRIPTION_MAX_LENGTH, GROUP_SUBJECT_MAX_LENGTH};
use whatsapp_rust::{CommunitySubgroup, CreateCommunityOptions};

use crate::client::{WaClient, from_upstream, to_upstream};
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// One linked subgroup of a community.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityLinkedGroup {
    /// Subgroup JID.
    pub id: Jid,
    /// Subgroup subject.
    pub name: String,
    /// Server-reported subgroup size, when the metadata query includes one.
    pub participant_count: Option<u32>,
    /// True for the community's default announcement subgroup.
    pub is_default_sub_group: bool,
    /// True for the community's general-chat subgroup.
    pub is_general_chat: bool,
}

impl CommunityLinkedGroup {
    /// Project the upstream subgroup view into the stable UI type.
    fn from_subgroup(subgroup: &CommunitySubgroup) -> Self {
        Self {
            id: from_upstream(&subgroup.id),
            name: subgroup.subject.clone(),
            participant_count: subgroup.participant_count,
            is_default_sub_group: subgroup.is_default_sub_group,
            is_general_chat: subgroup.is_general_chat,
        }
    }
}

/// Public information about a community.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityInfo {
    /// Community parent-group JID.
    pub id: Jid,
    /// Community name (the parent group's subject).
    pub name: String,
    /// Description, when the community has one.
    pub description: Option<String>,
    /// Linked subgroups, the announcement subgroup first.
    pub linked_groups: Vec<CommunityLinkedGroup>,
    /// Server-reported participant count of the community parent group.
    pub participant_count: u32,
}

impl WaClient {
    /// Create a community and return its parent-group JID.
    ///
    /// Upstream creates the parent group with the community flags and, when a
    /// description is given, sets it with a follow-up description IQ
    /// (`community.rs:153-188`). The defaults are the WhatsApp Web ones:
    /// open community, general chat created, non-admins cannot create
    /// subgroups.
    pub async fn create_community(&self, name: &str, description: Option<&str>) -> Result<Jid> {
        let (name, description) = normalize_community_input(name, description)?;

        let client = self.client().await?;
        let mut options = CreateCommunityOptions::new(name);
        options.description = description;

        let created = client
            .community()
            .create(options)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        Ok(from_upstream(&created.metadata.id))
    }

    /// Fetch a community's name, description, linked subgroups, and size.
    ///
    /// The community view is composed from two upstream calls: the parent
    /// group's metadata (`groups.rs:689-701`; a community parent is a group
    /// and carries `is_parent_group`) and the MEX subgroup query
    /// (`community.rs:296-339`, which needs a server round trip and does not
    /// populate the group cache). The returned JID is rejected with
    /// [`CoreError::InvalidInput`] when the server says it is not a community
    /// parent, so a regular group can not masquerade as one.
    pub async fn community_metadata(&self, community_id: &Jid) -> Result<CommunityInfo> {
        let client = self.client().await?;
        let jid = to_upstream(community_id)?;

        let metadata = client
            .groups()
            .get_metadata(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        if !metadata.is_parent_group {
            return Err(CoreError::InvalidInput(format!(
                "{community_id} is not a community"
            )));
        }

        let subgroups = client
            .community()
            .get_subgroups(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        // `size` is the server's authoritative count; the parent group's
        // participant list is the fallback when the server omits it.
        let participant_count = metadata
            .size
            .unwrap_or_else(|| u32::try_from(metadata.participants.len()).unwrap_or(u32::MAX));

        Ok(CommunityInfo {
            id: from_upstream(&metadata.id),
            name: metadata.subject.clone(),
            description: metadata.description.clone(),
            linked_groups: subgroups
                .iter()
                .map(CommunityLinkedGroup::from_subgroup)
                .collect(),
            participant_count,
        })
    }

    /// Link one existing group to a community as a subgroup.
    ///
    /// Upstream reports per-group outcomes instead of failing the whole IQ
    /// (`community.rs:233-260`); a group the server refused is turned into
    /// [`CoreError::Protocol`] carrying the server error code.
    pub async fn link_group_to_community(&self, community_id: &Jid, group_id: &Jid) -> Result<()> {
        let client = self.client().await?;
        let community = to_upstream(community_id)?;
        let group = to_upstream(group_id)?;

        let result = client
            .community()
            .link_subgroups(&community, std::slice::from_ref(&group))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        if let Some((_, code)) = result.failed_groups.first() {
            return Err(CoreError::Protocol(format!(
                "the server refused to link {group_id} to {community_id} (error code {code})"
            )));
        }
        Ok(())
    }

    /// Unlink one subgroup from a community.
    ///
    /// `remove_orphan_members` is fixed to `false`: the group is detached but
    /// its members stay in the community, which is the non-destructive WA Web
    /// default. The upstream flag (`community.rs:263-294`) can be exposed when
    /// the UI needs the alternative.
    pub async fn unlink_group_from_community(
        &self,
        community_id: &Jid,
        group_id: &Jid,
    ) -> Result<()> {
        let client = self.client().await?;
        let community = to_upstream(community_id)?;
        let group = to_upstream(group_id)?;

        let result = client
            .community()
            .unlink_subgroups(&community, std::slice::from_ref(&group), false)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        if let Some((_, code)) = result.failed_groups.first() {
            return Err(CoreError::Protocol(format!(
                "the server refused to unlink {group_id} from {community_id} (error code {code})"
            )));
        }
        Ok(())
    }

    /// Deactivate (delete) a community.
    ///
    /// Subgroups are unlinked but not deleted (`community.rs:211-218`).
    pub async fn deactivate_community(&self, community_id: &Jid) -> Result<()> {
        let client = self.client().await?;
        let jid = to_upstream(community_id)?;

        client
            .community()
            .deactivate(&jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Fetch the community's current invite link without resetting it.
    ///
    /// A community parent is a group, so this is the group invite IQ with
    /// `reset = false` (`groups.rs:973-983`); the existing link stays valid.
    pub async fn community_invite_link(&self, community_id: &Jid) -> Result<String> {
        self.invite_link(community_id).await
    }

    /// Join a community through its invite link, returning the community JID.
    ///
    /// Accepts the link shapes WhatsApp uses (`chat.whatsapp.com/...`,
    /// `web.whatsapp.com/...?code=...`, `whatsapp://chat/?code=...`) and a
    /// bare invite code. The upstream join parser already accepts a
    /// `<community>` response node (`wacore/src/iq/groups.rs:2916-2937`), so a
    /// community invite resolves to the parent-group JID. A community that
    /// requires approval still returns its JID; the pending state is not
    /// surfaced yet.
    pub async fn join_community(&self, invite_url: &str) -> Result<Jid> {
        let invite_url = invite_url.trim();
        if invite_url.is_empty() {
            return Err(CoreError::InvalidInput(
                "community invite link is empty".into(),
            ));
        }
        if !is_invite_link(invite_url) {
            return Err(CoreError::InvalidInput(format!(
                "malformed community invite link: {invite_url}"
            )));
        }

        let client = self.client().await?;
        let joined = client
            .groups()
            .join_with_invite_code(invite_url)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;

        Ok(from_upstream(joined.group_jid()))
    }
}

/// Trim and validate the create-community inputs before the network is hit.
fn normalize_community_input(
    name: &str,
    description: Option<&str>,
) -> Result<(String, Option<String>)> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::InvalidInput("community name is empty".into()));
    }
    if name.chars().count() > GROUP_SUBJECT_MAX_LENGTH {
        return Err(CoreError::InvalidInput(format!(
            "community name is longer than {GROUP_SUBJECT_MAX_LENGTH} characters"
        )));
    }

    let description = description
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    if let Some(text) = description.as_deref()
        && text.chars().count() > GROUP_DESCRIPTION_MAX_LENGTH
    {
        return Err(CoreError::InvalidInput(format!(
            "community description is longer than {GROUP_DESCRIPTION_MAX_LENGTH} characters"
        )));
    }

    Ok((name.to_owned(), description))
}

/// True when the text is one of the invite shapes the server accepts.
///
/// Mirrors the private extractor upstream uses before sending the join IQ
/// (`groups.rs:1537-1574`): a `chat.whatsapp.com` URL, a `code=` deep link
/// (`whatsapp://chat`, `web.whatsapp.com`), or a bare code. Anything else is
/// rejected as [`CoreError::InvalidInput`] before a network call.
fn is_invite_link(input: &str) -> bool {
    let input = input.trim();
    if input.is_empty() {
        return false;
    }

    if let Some(path) = input
        .strip_prefix("https://chat.whatsapp.com/")
        .or_else(|| input.strip_prefix("http://chat.whatsapp.com/"))
    {
        let path = path.split('?').next().unwrap_or(path);
        let code = path
            .strip_prefix("invite/")
            .unwrap_or(path)
            .trim_end_matches('/');
        return !code.is_empty() && !code.chars().any(char::is_whitespace);
    }

    let query_target = input.starts_with("whatsapp://chat")
        || input.starts_with("https://web.whatsapp.com/")
        || input.starts_with("http://web.whatsapp.com/");
    if query_target
        && let Some(query) = input.split('?').nth(1)
    {
        return query.split('&').any(|pair| {
            pair.strip_prefix("code=").is_some_and(|value| {
                let value = value.trim_end_matches('/');
                !value.is_empty() && !value.chars().any(char::is_whitespace)
            })
        });
    }

    // A bare invite code (no URL wrapper).
    !input.contains("://") && !input.contains('?') && !input.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_invite_shapes() {
        assert!(is_invite_link("https://chat.whatsapp.com/AbCdEfGh"));
        assert!(is_invite_link("http://chat.whatsapp.com/AbCdEfGh/"));
        assert!(is_invite_link(
            "https://chat.whatsapp.com/invite/AbCdEfGh?utm_source=x"
        ));
        assert!(is_invite_link(
            "https://web.whatsapp.com/accept?code=AbCdEfGh&other=1"
        ));
        assert!(is_invite_link("whatsapp://chat/?code=AbCdEfGh"));
        assert!(is_invite_link("  AbCdEfGh  "));
    }

    #[test]
    fn rejects_malformed_invites() {
        assert!(!is_invite_link(""));
        assert!(!is_invite_link("   "));
        assert!(!is_invite_link("https://chat.whatsapp.com/"));
        assert!(!is_invite_link("https://chat.whatsapp.com/invite/"));
        assert!(!is_invite_link("https://chat.whatsapp.com/AbCdEfGh extra"));
        assert!(!is_invite_link("https://example.com/AbCdEfGh"));
        assert!(!is_invite_link("whatsapp://chat/?code="));
        assert!(!is_invite_link("two words"));
    }

    #[test]
    fn normalizes_create_input() {
        let (name, description) =
            normalize_community_input("  RustWA  ", Some("  Developers  ")).expect("valid input");
        assert_eq!(name, "RustWA");
        assert_eq!(description.as_deref(), Some("Developers"));

        let (_, blank) = normalize_community_input("RustWA", Some("   ")).expect("valid input");
        assert_eq!(blank, None);

        let (_, empty) = normalize_community_input("RustWA", None).expect("valid input");
        assert_eq!(empty, None);
    }

    #[test]
    fn rejects_invalid_create_input() {
        assert!(matches!(
            normalize_community_input("   ", None),
            Err(CoreError::InvalidInput(_))
        ));
        assert!(matches!(
            normalize_community_input(&"n".repeat(GROUP_SUBJECT_MAX_LENGTH + 1), None),
            Err(CoreError::InvalidInput(_))
        ));
        assert!(matches!(
            normalize_community_input(
                "RustWA",
                Some(&"d".repeat(GROUP_DESCRIPTION_MAX_LENGTH + 1))
            ),
            Err(CoreError::InvalidInput(_))
        ));
    }
}
