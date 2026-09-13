//! Privacy and blocking: the account's privacy settings and the blocklist.
//!
//! Privacy settings travel over the `privacy` IQ namespace (upstream
//! `Client::fetch_privacy_settings` / `Client::set_privacy_setting` /
//! `Client::set_default_disappearing_mode`); blocking uses the `blocklist`
//! namespace (upstream `Client::blocking`). This module only maps the upstream
//! types and errors onto the core's stable, UI-facing surface.

use serde::Serialize;
use whatsapp_rust::privacy_settings::{PrivacyCategory, PrivacySettingsResponse, PrivacyValue};

use crate::client::{WaClient, from_upstream, to_upstream};
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// The privacy categories [`WaClient::set_privacy_setting`] accepts, in
/// upstream wire spelling (`WAWebPrivacySettings`). Server-added categories are
/// rejected rather than forwarded, so a typo never reaches the server.
pub const PRIVACY_SETTING_NAMES: [&str; 9] = [
    "last",
    "online",
    "profile",
    "status",
    "groupadd",
    "readreceipts",
    "calladd",
    "messages",
    "defense",
];

/// One privacy category and its current value, as wire strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettingEntry {
    /// Category wire name, e.g. `last` or `readreceipts`.
    pub category: String,
    /// Value wire name, e.g. `all`, `contacts`, `contact_blacklist`.
    pub value: String,
}

/// The account's privacy settings as the server reports them.
///
/// The upstream response is a flat list of categories; keeping the list (and
/// the wire strings) preserves categories this build does not know yet instead
/// of silently dropping them from the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySnapshot {
    /// Every category the server returned, in server order.
    pub settings: Vec<PrivacySettingEntry>,
}

impl PrivacySnapshot {
    /// Current wire value for `category`, if the server reported it.
    pub fn value_of(&self, category: &str) -> Option<&str> {
        self.settings
            .iter()
            .find(|entry| entry.category == category)
            .map(|entry| entry.value.as_str())
    }
}

impl WaClient {
    /// Block a contact. Accepts a phone-number JID or a LID; the upstream layer
    /// resolves the LID↔PN pair the server requires.
    pub async fn block_contact(&self, jid: &Jid) -> Result<()> {
        let client = self.client().await?;
        let target = to_upstream(jid)?;
        client
            .blocking()
            .block(&target)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Unblock a contact. Accepts a phone-number JID or a LID.
    pub async fn unblock_contact(&self, jid: &Jid) -> Result<()> {
        let client = self.client().await?;
        let target = to_upstream(jid)?;
        client
            .blocking()
            .unblock(&target)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// The account's blocklist.
    pub async fn blocked_contacts(&self) -> Result<Vec<Jid>> {
        let client = self.client().await?;
        let entries = client
            .blocking()
            .get_blocklist()
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(blocklist_to_jids(&entries))
    }

    /// Fetch the account's privacy settings.
    pub async fn fetch_privacy_settings(&self) -> Result<PrivacySnapshot> {
        let client = self.client().await?;
        let response = client
            .fetch_privacy_settings()
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(snapshot_from_upstream(&response))
    }

    /// Update one privacy setting.
    ///
    /// `setting` and `value` use the upstream wire names (`"last"`, `"all"`,
    /// `"contact_blacklist"`, …). Unknown setting names, unknown values and
    /// category/value combinations the server does not accept are rejected
    /// before anything is sent.
    pub async fn set_privacy_setting(&self, setting: &str, value: &str) -> Result<()> {
        let (category, value) = parse_privacy_setting(setting, value)?;
        let client = self.client().await?;
        client
            .set_privacy_setting(category, value)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        Ok(())
    }

    /// Set the default disappearing-message timer for new chats, in seconds.
    ///
    /// `0` turns the default off. WhatsApp Web offers 24 hours (`86400`),
    /// 7 days (`604800`) and 90 days (`7776000`).
    pub async fn set_disappearing_default(&self, seconds: u32) -> Result<()> {
        let client = self.client().await?;
        client
            .set_default_disappearing_mode(seconds)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }
}

/// Project the upstream settings response onto the stable snapshot, keeping the
/// server's wire strings and order.
fn snapshot_from_upstream(response: &PrivacySettingsResponse) -> PrivacySnapshot {
    PrivacySnapshot {
        settings: response
            .settings
            .iter()
            .map(|setting| PrivacySettingEntry {
                category: setting.category.as_str().to_owned(),
                value: setting.value.as_str().to_owned(),
            })
            .collect(),
    }
}

/// Project upstream blocklist entries onto bare contact JIDs.
fn blocklist_to_jids(entries: &[whatsapp_rust::BlocklistEntry]) -> Vec<Jid> {
    entries
        .iter()
        .map(|entry| from_upstream(&entry.jid))
        .collect()
}

/// Validate a `(setting, value)` pair against the wire allow-list and map it to
/// the upstream enums.
///
/// Values are checked against the category because upstream asserts the
/// combination in debug builds and the server rejects it otherwise.
fn parse_privacy_setting(setting: &str, value: &str) -> Result<(PrivacyCategory, PrivacyValue)> {
    if !PRIVACY_SETTING_NAMES.contains(&setting) {
        return Err(CoreError::InvalidInput(format!(
            "unknown privacy setting '{setting}' (expected one of: {})",
            PRIVACY_SETTING_NAMES.join(", ")
        )));
    }

    let category = PrivacyCategory::from(setting);
    let parsed = PrivacyValue::from(value);
    if matches!(parsed, PrivacyValue::Other(_)) {
        return Err(CoreError::InvalidInput(format!(
            "unknown privacy value '{value}'"
        )));
    }
    if !category.is_valid_value(&parsed) {
        return Err(CoreError::InvalidInput(format!(
            "value '{value}' is not valid for privacy setting '{setting}'"
        )));
    }
    Ok((category, parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::privacy_settings::PrivacySetting;

    #[test]
    fn allow_list_covers_all_upstream_categories() {
        // Every name we accept must map to a known upstream category (not the
        // `Other` fallback), and round-trip to the same wire spelling.
        for name in PRIVACY_SETTING_NAMES {
            let category = PrivacyCategory::from(name);
            assert!(
                !matches!(category, PrivacyCategory::Other(_)),
                "'{name}' is not a known upstream category"
            );
            assert_eq!(category.as_str(), name);
        }
    }

    #[test]
    fn parses_valid_settings() {
        let cases = [
            ("last", "contacts"),
            ("online", "match_last_seen"),
            ("profile", "contact_blacklist"),
            ("status", "none"),
            ("groupadd", "all"),
            ("readreceipts", "none"),
            ("calladd", "known"),
            ("messages", "contacts"),
            ("defense", "on_standard"),
        ];
        for (setting, value) in cases {
            let (category, parsed) = parse_privacy_setting(setting, value).expect("valid pair");
            assert_eq!(category.as_str(), setting);
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn rejects_unknown_setting_names() {
        for setting in ["bogus", "Last", "", "read_receipts", "defence"] {
            let error = parse_privacy_setting(setting, "all").expect_err("unknown setting");
            assert!(
                matches!(error, CoreError::InvalidInput(_)),
                "{setting}: {error}"
            );
        }
    }

    #[test]
    fn rejects_unknown_values() {
        let error = parse_privacy_setting("last", "everyone").expect_err("unknown value");
        assert!(matches!(error, CoreError::InvalidInput(_)), "{error}");
    }

    #[test]
    fn rejects_invalid_category_value_combinations() {
        for (setting, value) in [
            ("readreceipts", "contacts"),
            ("online", "none"),
            ("defense", "all"),
            ("messages", "contact_blacklist"),
            ("calladd", "none"),
        ] {
            let error = parse_privacy_setting(setting, value).expect_err("invalid combination");
            assert!(
                matches!(error, CoreError::InvalidInput(_)),
                "{setting}={value}: {error}"
            );
        }
    }

    #[test]
    fn maps_settings_response_onto_wire_strings() {
        let response = PrivacySettingsResponse {
            settings: vec![
                PrivacySetting {
                    category: PrivacyCategory::Last,
                    value: PrivacyValue::Contacts,
                },
                PrivacySetting {
                    category: PrivacyCategory::ReadReceipts,
                    value: PrivacyValue::None,
                },
                // Unknown categories/values survive the mapping untouched.
                PrivacySetting {
                    category: PrivacyCategory::Other("future".to_owned()),
                    value: PrivacyValue::Other("future_value".to_owned()),
                },
            ],
        };

        let snapshot = snapshot_from_upstream(&response);
        assert_eq!(
            snapshot.settings,
            vec![
                PrivacySettingEntry {
                    category: "last".to_owned(),
                    value: "contacts".to_owned(),
                },
                PrivacySettingEntry {
                    category: "readreceipts".to_owned(),
                    value: "none".to_owned(),
                },
                PrivacySettingEntry {
                    category: "future".to_owned(),
                    value: "future_value".to_owned(),
                },
            ]
        );
        assert_eq!(snapshot.value_of("last"), Some("contacts"));
        assert_eq!(snapshot.value_of("online"), None);
    }

    #[test]
    fn maps_blocklist_entries_to_jids() {
        let entries = vec![
            whatsapp_rust::BlocklistEntry {
                jid: "15551234567@s.whatsapp.net".parse().expect("jid"),
                timestamp: Some(1_700_000_000),
            },
            whatsapp_rust::BlocklistEntry {
                jid: "100000012345678@lid".parse().expect("jid"),
                timestamp: None,
            },
        ];

        let jids = blocklist_to_jids(&entries);
        assert_eq!(jids.len(), 2);
        assert_eq!(jids[0].as_str(), "15551234567@s.whatsapp.net");
        assert_eq!(jids[1].as_str(), "100000012345678@lid");
    }
}
