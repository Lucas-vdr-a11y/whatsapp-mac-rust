//! Business and power features: business profiles, chat labels, catalogs and
//! username lookup.
//!
//! Upstream coverage is uneven and this module is explicit about where the
//! protocol surface stops:
//!
//! - Business profiles are a real IQ surface (`w:biz`, upstream
//!   `BusinessProfileSpec`) combined with usync metadata for the verified name
//!   and about text. Only fields upstream actually parses are surfaced; fields
//!   it does not parse (for example a business logo) are not invented here.
//! - Labels are app-state mutations in the `regular` collection. Upstream can
//!   write them (`Client::labels()`), but has no persisted read API, so
//!   [`WaClient::fetch_labels`] requests a full snapshot of that collection and
//!   folds the label events it observes.
//! - Quick replies are only half-covered upstream: the `quick_reply` syncd
//!   schema exists (`wacore::appstate::schemas::QUICK_REPLY`) and
//!   `Client::send_app_state_action` can write one, but no reader exists and
//!   the app-state dispatcher emits no quick-reply event, so there is no
//!   `fetch_quick_replies` here — a read would fabricate data.
//! - Catalogs are GraphQL-over-MEX only (`WAWebQueryCatalogQuery`), exposed
//!   through `Client::mex()`.
//! - Username lookup is the usync contact protocol (`<contact username="…"/>`),
//!   the same request WhatsApp Web's `WAWebQueryExistsJob.queryUsernameExists`
//!   builds. whatsapp-rust 0.7.0 exposes no wrapped username feature, but the
//!   typed `UsyncQuery`/`UsyncQuerySpec` builder it added for existence checks
//!   accepts a username user directly. There is no MEX username→JID operation:
//!   the generated registry's username entries are `get_username` (this
//!   account's own handle), `username_availability` (a candidate check with no
//!   identity) and `usync` (fetching users by JID/phone). The generated
//!   `set_username` mutations exist as descriptors but no write API is exposed
//!   here.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use whatsapp_rust::MexRequest;
use whatsapp_rust::prelude::{Event, EventHandler, EventInterest, EventKind};
use whatsapp_rust::serde_json;
use whatsapp_rust::sync_task::MajorSyncTask;
use whatsapp_rust::wacore::appstate::patch_decode::WAPatchName;
use whatsapp_rust::wacore::iq::business::{
    BusinessHours as UpstreamBusinessHours, BusinessProfile as UpstreamBusinessProfile,
    BusinessProfileSpec,
};
use whatsapp_rust::wacore::iq::mex_operations::query_catalog;
use whatsapp_rust::wacore::iq::usync::{
    UsyncAddressingMode, UsyncContext, UsyncMode, UsyncOutcome, UsyncProtocol, UsyncProtocolKind,
    UsyncProtocolResult, UsyncQuery, UsyncQuerySpec, UsyncResponse, UsyncUser,
};

use crate::client::{WaClient, from_upstream, to_upstream};
use crate::error::{CoreError, Result};
use crate::types::Jid;

/// Page size used for one catalog query.
const CATALOG_PAGE_SIZE: i64 = 20;
/// Maximum accepted label id length (defensive; real label ids are short).
const MAX_LABEL_ID_LEN: usize = 128;
/// Shortest username the official client will query
/// (`WAWebUsernameConstants.USERNAME_MIN_LENGTH`, not exported by 0.7.0).
const USERNAME_MIN_LENGTH: usize = 3;
/// Longest username the official client will query
/// (`WAWebUsernameConstants.USERNAME_MAX_LENGTH`, not exported by 0.7.0).
const USERNAME_MAX_LENGTH: usize = 35;

/// One business category (`<category id="…">name</category>`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessCategory {
    /// Server category id.
    pub id: String,
    /// Localized category name as sent by the server.
    pub name: String,
}

/// One opening-hours rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessHoursEntry {
    /// Wire day of week (`sun`..`sat`, or the raw fallback the server sent).
    pub day_of_week: String,
    /// Wire mode (`open_24h`, `specific_hours`, `appointment_only`).
    pub mode: String,
    /// Minutes after midnight, when the mode defines them.
    pub open_time: Option<u32>,
    /// Minutes after midnight, when the mode defines them.
    pub close_time: Option<u32>,
}

/// A business's opening hours.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessHours {
    /// IANA timezone the times are expressed in, when the server sent one.
    pub timezone: Option<String>,
    /// Per-day rules.
    pub entries: Vec<BusinessHoursEntry>,
}

/// A business profile as far as upstream can resolve one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessProfile {
    /// The queried JID.
    pub jid: Jid,
    /// Verified business name from usync, when the account has one.
    pub verified_name: Option<String>,
    /// Business description (`w:biz` `<description>`).
    pub description: Option<String>,
    /// About/status text from usync, when visible.
    pub about: Option<String>,
    /// Websites listed on the profile.
    pub website: Vec<String>,
    /// Contact email, when the profile exposes one.
    pub email: Option<String>,
    /// Street address, when the profile exposes one.
    pub address: Option<String>,
    /// Business categories.
    pub categories: Vec<BusinessCategory>,
    /// Opening hours, when the profile defines any.
    pub business_hours: Option<BusinessHours>,
}

/// One chat label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    /// Opaque server label id (not numeric).
    pub id: String,
    /// Current display name, when the account set one.
    pub name: Option<String>,
    /// WhatsApp color index.
    pub color: Option<i32>,
    /// Chats currently tagged with this label.
    pub chat_ids: Vec<Jid>,
    /// Unix milliseconds of the newest edit applied, when known.
    pub updated_at: Option<i64>,
}

/// One catalog product.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProduct {
    /// Product id.
    pub id: Option<String>,
    /// Product name.
    pub name: Option<String>,
    /// Product description.
    pub description: Option<String>,
    /// Display price (server-formatted).
    pub price: Option<String>,
    /// ISO currency code, when the server sent one.
    pub currency: Option<String>,
    /// Sale price, when the product is discounted.
    pub sale_price: Option<String>,
    /// Availability string (`in stock`, `out of stock`, …).
    pub availability: Option<String>,
    /// Primary image URL.
    pub image_url: Option<String>,
    /// Canonical product URL.
    pub url: Option<String>,
    /// Retailer-provided SKU.
    pub retailer_id: Option<String>,
    /// Whether the business hid this product.
    pub hidden: bool,
}

/// A business's product catalog page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// The business JID the catalog belongs to.
    pub jid: Jid,
    /// Products on this page.
    pub products: Vec<CatalogProduct>,
    /// Cursor for the next page, when the server reported one.
    pub next_cursor: Option<String>,
}

impl WaClient {
    /// Resolve a business profile for a user JID.
    ///
    /// Two server round trips: the `w:biz` profile IQ (description, website,
    /// email, address, categories, business hours) and a usync query for the
    /// verified name and about text. If the `w:biz` query fails (regular
    /// accounts have no business profile) the usync metadata is still
    /// returned; if the usync query fails the call fails.
    pub async fn business_profile(&self, jid: &Jid) -> Result<BusinessProfile> {
        if jid.is_group() {
            return Err(CoreError::InvalidInput(
                "business profiles exist for user JIDs, not groups".into(),
            ));
        }

        let client = self.client().await?;
        let target = to_upstream(jid)?;

        let profile = match client.execute(BusinessProfileSpec::new(&target)).await {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(
                    %error,
                    jid = %target,
                    "business profile query failed; falling back to usync metadata"
                );
                None
            }
        };

        let mut info = client
            .contacts()
            .get_user_info(std::slice::from_ref(&target))
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))?;
        let user = info.values_mut().next();

        let verified_name = user
            .as_ref()
            .and_then(|user| user.verified_name.as_ref())
            .and_then(|verified| verified.name.clone());
        let about = user.as_ref().and_then(|user| user.status.clone());

        Ok(business_profile_from_parts(
            jid.clone(),
            profile.as_ref(),
            verified_name,
            about,
        ))
    }

    /// Fetch the account's current chat labels.
    ///
    /// Upstream has no persisted label query: label edits and associations are
    /// app-state mutations in the `regular` collection, and the only way to
    /// read them is to observe the sync that applies them. This subscribes to
    /// the upstream label events, requests a full snapshot of that collection
    /// and folds the observed events into the current label set.
    ///
    /// The call waits for the collection reservation (another sync may hold
    /// it). If the client cannot reach the server the task is deferred
    /// silently, in which case the returned set can be empty; nothing is
    /// cached upstream to fall back to.
    pub async fn fetch_labels(&self) -> Result<Vec<Label>> {
        let client = self.client().await?;

        let collector = Arc::new(LabelCollector::default());
        let subscription = client.subscribe(
            EventInterest::of(&[
                EventKind::LabelEditUpdate,
                EventKind::LabelAssociationUpdate,
            ]),
            collector.clone(),
        );

        client
            .process_sync_task(MajorSyncTask::AppStateSync {
                name: WAPatchName::Regular,
                full_sync: true,
            })
            .await;

        // Events are dispatched synchronously on the sync task, so by the time
        // the await above returns the collector holds everything this sync
        // produced. Drop the subscription before building the result.
        drop(subscription);
        Ok(build_labels(collector.take_events()))
    }

    /// Associate a label with a chat.
    pub async fn add_label(&self, chat_id: &Jid, label_id: &str) -> Result<()> {
        let label_id = normalize_label_id(label_id)?;
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        client
            .labels()
            .add_chat_label(&label_id, &jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Remove a label association from a chat.
    pub async fn remove_label(&self, chat_id: &Jid, label_id: &str) -> Result<()> {
        let label_id = normalize_label_id(label_id)?;
        let client = self.client().await?;
        let jid = to_upstream(chat_id)?;

        client
            .labels()
            .remove_chat_label(&label_id, &jid)
            .await
            .map_err(|error| CoreError::Protocol(error.to_string()))
    }

    /// Fetch the first page of a business's product catalog.
    ///
    /// Pure MEX (`WAWebQueryCatalogQuery`); there is no catalog IQ surface in
    /// upstream. Pagination beyond the first page would need the returned
    /// cursor and a follow-up call surface that this crate does not expose yet.
    pub async fn fetch_catalog(&self, jid: &Jid) -> Result<Catalog> {
        let client = self.client().await?;
        let target = to_upstream(jid)?;

        let variables = query_catalog::Variables {
            request: Some(query_catalog::Request {
                product_catalog: Some(query_catalog::ProductCatalog {
                    jid: Some(target.to_string()),
                    limit: Some(CATALOG_PAGE_SIZE),
                    ..Default::default()
                }),
            }),
        };
        let request = MexRequest::new(query_catalog::NAME, query_catalog::DOC_ID, variables);
        let response = client
            .mex()
            .query(request)
            .await
            .map_err(|error| CoreError::Protocol(format!("catalog query failed: {error}")))?;

        let Some(data) = response.data else {
            return Ok(Catalog {
                jid: jid.clone(),
                products: Vec::new(),
                next_cursor: None,
            });
        };
        let parsed: query_catalog::Response = serde_json::from_value(data)
            .map_err(|error| CoreError::Protocol(format!("catalog decode failed: {error}")))?;
        Ok(catalog_from_response(jid.clone(), parsed))
    }

    /// Resolve a WhatsApp username to a JID.
    ///
    /// Usync contact-protocol query (`<contact username="…"/>`), the shape
    /// WhatsApp Web's `WAWebQueryExistsJob.queryUsernameExists` uses. Returns
    /// `Ok(None)` when the server reports the username is not reachable, when
    /// it echoes a different username, or when the account protects its
    /// username with a key: that last case is a real "key required" answer, but
    /// the `Option<Jid>` shape here cannot tell it apart from "not found" (the
    /// upstream error kinds needed for that are newer than 0.7.0).
    ///
    /// There is no MEX operation that maps a username to a JID in this version:
    /// `get_username` returns the account's own handle and `username_availability`
    /// checks a candidate without resolving it.
    pub async fn username_lookup(&self, username: &str) -> Result<Option<Jid>> {
        let username = normalize_username(username)?;
        let client = self.client().await?;

        let query = username_query(&username)
            .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
        // `Client::generate_request_id` is crate-private in 0.7.0; the public
        // message-id generator serves the same purpose (a unique usync `sid`).
        let spec = UsyncQuerySpec::new(query, client.generate_message_id())
            .map_err(|error| CoreError::InvalidInput(error.to_string()))?;
        let response = client
            .execute(spec)
            .await
            .map_err(|error| CoreError::Protocol(format!("username lookup failed: {error}")))?;

        Ok(extract_username_match(&response, &username))
    }
}

/// Typed usync query for one username. Split out so the tests can parse a
/// response with the same spec the production call builds.
fn username_query(
    username: &str,
) -> std::result::Result<UsyncQuery, whatsapp_rust::wacore::iq::usync::UsyncValidationError> {
    UsyncQuery::new(
        UsyncMode::Query,
        UsyncContext::Interactive,
        vec![
            UsyncProtocol::Contact {
                addressing_mode: UsyncAddressingMode::Lid,
            },
            UsyncProtocol::BusinessVerifiedName,
        ],
        vec![UsyncUser::from_username(username.to_owned())],
    )
}

/// Project a username lookup response. `contact_type == "in"` is the only
/// confirmation; "out" (and an empty list) is a miss. When the server echoes
/// the queried username it must match case-insensitively, so a stale/cached
/// answer cannot be handed back as the wrong person. An `"in"` contact with no
/// JID is the key-required case and maps to `None` here.
fn extract_username_match(response: &UsyncResponse, username: &str) -> Option<Jid> {
    let user = response.users.first()?;
    let Some(UsyncProtocolResult::Contact(UsyncOutcome::Value(contact))) =
        user.protocol(UsyncProtocolKind::Contact)
    else {
        return None;
    };
    if contact.contact_type != "in" {
        return None;
    }
    if let Some(found) = &contact.username
        && !found.eq_ignore_ascii_case(username)
    {
        return None;
    }
    user.id.as_ref().map(from_upstream)
}

/// Project the upstream `w:biz` profile plus usync metadata into the stable
/// UI type. Pure so it is unit-testable without a connection.
fn business_profile_from_parts(
    jid: Jid,
    profile: Option<&UpstreamBusinessProfile>,
    verified_name: Option<String>,
    about: Option<String>,
) -> BusinessProfile {
    let description = profile.and_then(|profile| clean_text(profile.description.clone()));
    let email = profile.and_then(|profile| clean_optional(profile.email.clone()));
    let address = profile.and_then(|profile| clean_optional(profile.address.clone()));
    let website = profile
        .map(|profile| {
            profile
                .website
                .iter()
                .filter_map(|url| clean_text(url.clone()))
                .collect()
        })
        .unwrap_or_default();
    let categories = profile
        .map(|profile| {
            profile
                .categories
                .iter()
                .map(|category| BusinessCategory {
                    id: category.id.clone(),
                    name: category.name.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let business_hours = profile.and_then(|profile| map_business_hours(&profile.business_hours));

    BusinessProfile {
        jid,
        verified_name: clean_optional(verified_name),
        description,
        about: clean_optional(about),
        website,
        email,
        address,
        categories,
        business_hours,
    }
}

/// Map upstream opening hours, dropping an entirely empty block.
fn map_business_hours(hours: &UpstreamBusinessHours) -> Option<BusinessHours> {
    let entries: Vec<BusinessHoursEntry> = hours
        .business_config
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|config| BusinessHoursEntry {
            day_of_week: config.day_of_week.as_str().to_owned(),
            mode: config.mode.as_str().to_owned(),
            open_time: config.open_time,
            close_time: config.close_time,
        })
        .collect();

    if hours.timezone.is_none() && entries.is_empty() {
        return None;
    }
    Some(BusinessHours {
        timezone: hours.timezone.clone(),
        entries,
    })
}

/// Trim and drop empty strings; both the UI and serde prefer `None`.
fn clean_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(clean_text)
}

/// Normalize a label id and reject input that upstream would refuse anyway.
fn normalize_label_id(label_id: &str) -> Result<String> {
    let trimmed = label_id.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput("label id is empty".into()));
    }
    if trimmed.len() > MAX_LABEL_ID_LEN {
        return Err(CoreError::InvalidInput(format!(
            "label id is longer than {MAX_LABEL_ID_LEN} bytes"
        )));
    }
    Ok(trimmed.to_owned())
}

/// Normalize a username: trim, drop a leading `@`, enforce the WhatsApp
/// username alphabet (`a-z`, `0-9`, `.`, `_`, ASCII case-insensitive) and the
/// 3..=35 length the official client uses.
fn normalize_username(username: &str) -> Result<String> {
    let trimmed = username.trim().trim_start_matches('@').trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput("username is empty".into()));
    }
    let length = trimmed.chars().count();
    if !(USERNAME_MIN_LENGTH..=USERNAME_MAX_LENGTH).contains(&length) {
        return Err(CoreError::InvalidInput(format!(
            "username length {length} is outside {USERNAME_MIN_LENGTH}..={USERNAME_MAX_LENGTH}"
        )));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    {
        return Err(CoreError::InvalidInput(format!(
            "invalid username: {username}"
        )));
    }
    Ok(trimmed.to_owned())
}

/// Project a decoded catalog page into the stable UI type.
fn catalog_from_response(jid: Jid, response: query_catalog::Response) -> Catalog {
    let Some(catalog) = response
        .xwa_product_catalog_get_product_catalog
        .and_then(|wrapper| wrapper.product_catalog)
    else {
        return Catalog {
            jid,
            products: Vec::new(),
            next_cursor: None,
        };
    };

    let products = catalog
        .products
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(product_from_upstream)
        .collect();

    Catalog {
        jid,
        products,
        next_cursor: catalog.paging.and_then(|paging| paging.after),
    }
}

fn product_from_upstream(product: &query_catalog::Products) -> CatalogProduct {
    CatalogProduct {
        id: product.id.clone(),
        name: product.name.clone(),
        description: product.description.clone(),
        price: product.price.clone(),
        currency: product.currency.clone(),
        sale_price: product
            .sale_price
            .as_ref()
            .and_then(|sale| sale.price.clone()),
        availability: product.product_availability.clone(),
        image_url: product
            .media
            .as_ref()
            .and_then(|media| media.images.as_ref())
            .and_then(|images| images.first())
            .and_then(|image| {
                image
                    .original_image_url
                    .clone()
                    .or_else(|| image.request_image_url.clone())
            }),
        url: product.url.clone().or_else(|| product.shimmed_url.clone()),
        retailer_id: product.retailer_id.clone(),
        hidden: product.is_hidden.unwrap_or(false),
    }
}

/// Folds label mutations into the current label set. Events are sorted by
/// their server timestamp first so a rename/delete that raced an association
/// ends up in the same order on every device.
fn build_labels(mut events: Vec<LabelEvent>) -> Vec<Label> {
    events.sort_by_key(LabelEvent::at_ms);

    let mut state: BTreeMap<String, LabelState> = BTreeMap::new();
    for event in events {
        match event {
            LabelEvent::Edit {
                at_ms,
                label_id,
                name,
                color,
                deleted,
            } => {
                let entry = state.entry(label_id).or_default();
                entry.updated_at = Some(at_ms);
                if deleted {
                    entry.deleted = true;
                    entry.chats.clear();
                } else {
                    entry.deleted = false;
                    if name.is_some() {
                        entry.name = name;
                    }
                    if color.is_some() {
                        entry.color = color;
                    }
                }
            }
            LabelEvent::Association {
                label_id,
                chat_id,
                labeled,
                ..
            } => {
                let entry = state.entry(label_id).or_default();
                if labeled {
                    entry.chats.insert(chat_id);
                } else {
                    entry.chats.remove(&chat_id);
                }
            }
        }
    }

    state
        .into_iter()
        .filter(|(_, label)| !label.deleted)
        .map(|(id, label)| Label {
            id,
            name: label.name,
            color: label.color,
            chat_ids: label.chats.into_iter().map(Jid::new).collect(),
            updated_at: label.updated_at,
        })
        .collect()
}

/// Accumulator state for one label while folding events.
#[derive(Default)]
struct LabelState {
    name: Option<String>,
    color: Option<i32>,
    deleted: bool,
    chats: BTreeSet<String>,
    updated_at: Option<i64>,
}

/// A chronological label event collected from the upstream event bus.
#[derive(Clone, Debug, PartialEq, Eq)]
enum LabelEvent {
    Edit {
        at_ms: i64,
        label_id: String,
        name: Option<String>,
        color: Option<i32>,
        deleted: bool,
    },
    Association {
        at_ms: i64,
        label_id: String,
        chat_id: String,
        labeled: bool,
    },
}

impl LabelEvent {
    fn at_ms(&self) -> i64 {
        match self {
            Self::Edit { at_ms, .. } | Self::Association { at_ms, .. } => *at_ms,
        }
    }
}

/// Collects label events dispatched while a `regular` collection sync runs.
#[derive(Default)]
struct LabelCollector {
    events: Mutex<Vec<LabelEvent>>,
}

impl LabelCollector {
    fn take_events(&self) -> Vec<LabelEvent> {
        self.events
            .lock()
            .map(|mut events| std::mem::take(&mut *events))
            .unwrap_or_default()
    }
}

impl EventHandler for LabelCollector {
    fn handle_event(&self, event: Arc<Event>) {
        let collected = match event.as_ref() {
            Event::LabelEditUpdate(update) => Some(LabelEvent::Edit {
                at_ms: update.timestamp.timestamp_millis(),
                label_id: update.label_id.clone(),
                name: update.action.name.clone(),
                color: update.action.color,
                deleted: update.action.deleted.unwrap_or(false),
            }),
            Event::LabelAssociationUpdate(update) => Some(LabelEvent::Association {
                at_ms: update.timestamp.timestamp_millis(),
                label_id: update.label_id.clone(),
                chat_id: update.chat_jid.to_string(),
                labeled: update.action.labeled.unwrap_or(false),
            }),
            _ => None,
        };

        if let Some(collected) = collected
            && let Ok(mut events) = self.events.lock()
        {
            events.push(collected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::NodeBuilder;
    use whatsapp_rust::serde_json::json;
    use whatsapp_rust::wacore::iq::spec::IqSpec;

    fn upstream_jid() -> whatsapp_rust::Jid {
        "15551234567@s.whatsapp.net".parse().expect("valid jid")
    }

    fn parse_biz_profile() -> UpstreamBusinessProfile {
        let jid = upstream_jid();
        let node = NodeBuilder::new("iq")
            .children([NodeBuilder::new("business_profile")
                .attr("v", "244")
                .children([NodeBuilder::new("profile")
                    .attr("jid", &jid)
                    .children([
                        NodeBuilder::new("description")
                            .string_content("We fix things")
                            .build(),
                        NodeBuilder::new("email")
                            .string_content("shop@example.com")
                            .build(),
                        NodeBuilder::new("website")
                            .string_content("https://example.com")
                            .build(),
                        NodeBuilder::new("website")
                            .string_content("https://shop.example.com")
                            .build(),
                        NodeBuilder::new("address")
                            .string_content("1 Main St")
                            .build(),
                        NodeBuilder::new("categories")
                            .children([
                                NodeBuilder::new("category")
                                    .attr("id", "1")
                                    .string_content("Retail")
                                    .build(),
                                NodeBuilder::new("category")
                                    .attr("id", "2")
                                    .string_content("Repair")
                                    .build(),
                            ])
                            .build(),
                        NodeBuilder::new("business_hours")
                            .attr("timezone", "Europe/Amsterdam")
                            .children([
                                NodeBuilder::new("business_hours_config")
                                    .attr("day_of_week", "mon")
                                    .attr("mode", "specific_hours")
                                    .attr("open_time", "480")
                                    .attr("close_time", "1080")
                                    .build(),
                                NodeBuilder::new("business_hours_config")
                                    .attr("day_of_week", "sun")
                                    .attr("mode", "open_24h")
                                    .build(),
                            ])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();

        BusinessProfileSpec::new(&jid)
            .parse_response(&node.as_node_ref())
            .expect("response parses")
            .expect("has a business profile")
    }

    #[test]
    fn maps_business_profile_fields() {
        let profile = parse_biz_profile();
        let mapped = business_profile_from_parts(
            Jid::new("15551234567@s.whatsapp.net"),
            Some(&profile),
            Some("  Acme Ltd  ".to_owned()),
            Some("We fix things fast".to_owned()),
        );

        assert_eq!(mapped.verified_name.as_deref(), Some("Acme Ltd"));
        assert_eq!(mapped.description.as_deref(), Some("We fix things"));
        assert_eq!(mapped.about.as_deref(), Some("We fix things fast"));
        assert_eq!(mapped.email.as_deref(), Some("shop@example.com"));
        assert_eq!(mapped.address.as_deref(), Some("1 Main St"));
        assert_eq!(
            mapped.website,
            vec![
                "https://example.com".to_owned(),
                "https://shop.example.com".to_owned(),
            ]
        );
        assert_eq!(mapped.categories.len(), 2);
        assert_eq!(mapped.categories[1].id, "2");
        assert_eq!(mapped.categories[1].name, "Repair");
    }

    #[test]
    fn maps_business_hours_including_absent_times() {
        let profile = parse_biz_profile();
        let mapped = business_profile_from_parts(
            Jid::new("15551234567@s.whatsapp.net"),
            Some(&profile),
            None,
            None,
        );
        let hours = mapped.business_hours.expect("hours present");

        assert_eq!(hours.timezone.as_deref(), Some("Europe/Amsterdam"));
        assert_eq!(hours.entries.len(), 2);
        assert_eq!(hours.entries[0].day_of_week, "mon");
        assert_eq!(hours.entries[0].mode, "specific_hours");
        assert_eq!(hours.entries[0].open_time, Some(480));
        assert_eq!(hours.entries[0].close_time, Some(1080));
        assert_eq!(hours.entries[1].day_of_week, "sun");
        assert_eq!(hours.entries[1].open_time, None);
        assert_eq!(hours.entries[1].close_time, None);
    }

    #[test]
    fn empty_business_profile_stays_empty() {
        let mapped = business_profile_from_parts(
            Jid::new("15551234567@s.whatsapp.net"),
            None,
            Some(" ".to_owned()),
            None,
        );

        assert!(mapped.verified_name.is_none());
        assert!(mapped.description.is_none());
        assert!(mapped.website.is_empty());
        assert!(mapped.categories.is_empty());
        assert!(mapped.business_hours.is_none());
    }

    #[test]
    fn folds_label_events_chronologically() {
        let events = vec![
            LabelEvent::Association {
                at_ms: 300,
                label_id: "5".into(),
                chat_id: "b@s.whatsapp.net".into(),
                labeled: true,
            },
            LabelEvent::Edit {
                at_ms: 100,
                label_id: "5".into(),
                name: Some("Work".into()),
                color: Some(1),
                deleted: false,
            },
            LabelEvent::Association {
                at_ms: 400,
                label_id: "5".into(),
                chat_id: "a@s.whatsapp.net".into(),
                labeled: true,
            },
            // A later rename wins, and removing the association sticks.
            LabelEvent::Edit {
                at_ms: 500,
                label_id: "5".into(),
                name: Some("Office".into()),
                color: Some(3),
                deleted: false,
            },
            LabelEvent::Association {
                at_ms: 600,
                label_id: "5".into(),
                chat_id: "b@s.whatsapp.net".into(),
                labeled: false,
            },
            LabelEvent::Edit {
                at_ms: 700,
                label_id: "9".into(),
                name: Some("Temporary".into()),
                color: None,
                deleted: true,
            },
        ];

        let labels = build_labels(events);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].id, "5");
        assert_eq!(labels[0].name.as_deref(), Some("Office"));
        assert_eq!(labels[0].color, Some(3));
        assert_eq!(labels[0].updated_at, Some(500));
        assert_eq!(
            labels[0].chat_ids,
            vec![Jid::new("a@s.whatsapp.net")],
            "only the still-associated chat remains"
        );
    }

    #[test]
    fn deleted_label_can_be_recreated() {
        let events = vec![
            LabelEvent::Edit {
                at_ms: 100,
                label_id: "5".into(),
                name: Some("Old".into()),
                color: Some(1),
                deleted: false,
            },
            LabelEvent::Edit {
                at_ms: 200,
                label_id: "5".into(),
                name: None,
                color: None,
                deleted: true,
            },
            LabelEvent::Edit {
                at_ms: 300,
                label_id: "5".into(),
                name: Some("New".into()),
                color: Some(2),
                deleted: false,
            },
        ];

        let labels = build_labels(events);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].name.as_deref(), Some("New"));
        assert_eq!(labels[0].updated_at, Some(300));
    }

    #[test]
    fn normalizes_and_rejects_label_ids() {
        assert_eq!(normalize_label_id("  7 ").unwrap(), "7");
        assert!(matches!(
            normalize_label_id("   "),
            Err(CoreError::InvalidInput(_))
        ));
        assert!(matches!(
            normalize_label_id(&"x".repeat(MAX_LABEL_ID_LEN + 1)),
            Err(CoreError::InvalidInput(_))
        ));
    }

    #[test]
    fn normalizes_and_rejects_usernames() {
        assert_eq!(normalize_username("  @Alice.W_1 ").unwrap(), "Alice.W_1");
        assert!(matches!(
            normalize_username(""),
            Err(CoreError::InvalidInput(_))
        ));
        assert!(
            matches!(normalize_username("ab"), Err(CoreError::InvalidInput(_))),
            "shorter than the 3-char minimum"
        );
        assert!(matches!(
            normalize_username("with space"),
            Err(CoreError::InvalidInput(_))
        ));
        assert!(matches!(
            normalize_username("user@example.com"),
            Err(CoreError::InvalidInput(_))
        ));
        assert!(matches!(
            normalize_username(&"a".repeat(USERNAME_MAX_LENGTH + 1)),
            Err(CoreError::InvalidInput(_))
        ));
    }

    /// Parse a usync response with the same spec the production call builds.
    fn parse_username_response(
        contact_type: &str,
        jid: Option<&str>,
        echoed_username: Option<&str>,
    ) -> UsyncResponse {
        let mut contact = NodeBuilder::new("contact").attr("type", contact_type);
        if let Some(username) = echoed_username {
            contact = contact.attr("username", username);
        }
        let mut user = NodeBuilder::new("user");
        if let Some(jid) = jid {
            user = user.attr("jid", jid);
        }
        let node = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([user.children([contact.build()]).build()])
                    .build()])
                .build()])
            .build();

        let spec = UsyncQuerySpec::new(
            username_query("alice").expect("valid query"),
            "test-sid".to_owned(),
        )
        .expect("valid spec");
        spec.parse_response(&node.as_node_ref())
            .expect("response parses")
    }

    #[test]
    fn extracts_matching_username_case_insensitively() {
        let response =
            parse_username_response("in", Some("15550000002@s.whatsapp.net"), Some("Alice.W"));

        assert_eq!(
            extract_username_match(&response, "alice.w"),
            Some(Jid::new("15550000002@s.whatsapp.net"))
        );
    }

    #[test]
    fn rejects_username_mismatch_and_contact_out() {
        let mismatch = parse_username_response(
            "in",
            Some("15550000001@s.whatsapp.net"),
            Some("someoneelse"),
        );
        assert_eq!(extract_username_match(&mismatch, "alice"), None);

        let out = parse_username_response("out", Some("15550000002@s.whatsapp.net"), None);
        assert_eq!(extract_username_match(&out, "alice"), None);
    }

    #[test]
    fn key_required_answer_maps_to_none() {
        // A PIN-protected username answers type="in" without disclosing an id.
        let response = parse_username_response("in", None, Some("alice"));
        assert_eq!(extract_username_match(&response, "alice"), None);
    }

    #[test]
    fn maps_catalog_page() {
        let response: query_catalog::Response = serde_json::from_value(json!({
            "xwa_product_catalog_get_product_catalog": {
                "product_catalog": {
                    "paging": { "after": "cursor-2" },
                    "products": [
                        {
                            "id": "p1",
                            "name": "Widget",
                            "description": "A sturdy widget",
                            "price": "€ 9,99",
                            "currency": "EUR",
                            "sale_price": { "price": "€ 7,99" },
                            "product_availability": "in stock",
                            "is_hidden": false,
                            "retailer_id": "SKU-1",
                            "url": "https://wa.me/p/p1",
                            "media": {
                                "images": [
                                    {
                                        "original_image_url": "https://cdn.example.com/p1.jpg",
                                        "request_image_url": "https://cdn.example.com/p1-thumb.jpg"
                                    }
                                ]
                            }
                        },
                        {
                            "id": "p2",
                            "name": "Hidden thing",
                            "is_hidden": true,
                            "media": {
                                "images": [
                                    { "request_image_url": "https://cdn.example.com/p2-thumb.jpg" }
                                ]
                            }
                        }
                    ]
                }
            }
        }))
        .expect("catalog decodes");

        let catalog = catalog_from_response(Jid::new("15551234567@s.whatsapp.net"), response);

        assert_eq!(catalog.next_cursor.as_deref(), Some("cursor-2"));
        assert_eq!(catalog.products.len(), 2);
        assert_eq!(catalog.products[0].id.as_deref(), Some("p1"));
        assert_eq!(catalog.products[0].sale_price.as_deref(), Some("€ 7,99"));
        assert_eq!(
            catalog.products[0].image_url.as_deref(),
            Some("https://cdn.example.com/p1.jpg")
        );
        assert!(catalog.products[1].hidden);
        assert_eq!(
            catalog.products[1].image_url.as_deref(),
            Some("https://cdn.example.com/p2-thumb.jpg")
        );
    }

    #[test]
    fn empty_catalog_maps_to_empty_page() {
        let response = query_catalog::Response::default();
        let catalog = catalog_from_response(Jid::new("15551234567@s.whatsapp.net"), response);

        assert!(catalog.products.is_empty());
        assert!(catalog.next_cursor.is_none());
    }
}
