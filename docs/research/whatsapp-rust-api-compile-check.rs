//! Compile-time verification of the whatsapp-rust 0.7 API surface documented in
//! docs/research/whatsapp-rust-api.md. This binary is never run; it only has to
//! type-check against the real crate.

#![allow(dead_code, unused_variables, unused_imports)]

use std::sync::Arc;
use std::time::Duration;

use whatsapp_rust::bot::{Bot, BotHandle, EventDelivery, MessageContext};
use whatsapp_rust::download::{Downloadable, MediaType};
use whatsapp_rust::media::{self, AudioOptions, DocumentOptions, ImageOptions, VideoOptions};
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::prelude::*;
use whatsapp_rust::store::{SqliteStore, SqliteStoreConfig};
use whatsapp_rust::types::events::{
    ChannelEventHandler, EventInterest, EventKind, InboundMessage, PairingCodeError,
};
use whatsapp_rust::types::durability_hook::InboundDurabilityHook;
use whatsapp_rust::upload::UploadOptions;
use whatsapp_rust::{Client, GroupCreateOptions, GroupParticipantOptions, SendOptions, StanzaType};

async fn bot_flow() -> Result<(), Box<dyn std::error::Error>> {
    // --- Backends -----------------------------------------------------------
    let store = SqliteStore::new("whatsapp.db").await?;
    let store2 = SqliteStore::with_config("whatsapp.db", SqliteStoreConfig::default()).await?;
    let store3 = SqliteStore::new_for_device("whatsapp.db", 1).await?;
    let memory = whatsapp_rust::wacore::store::InMemoryBackend::new();

    // --- High-level Bot builder --------------------------------------------
    let mut builder = Bot::builder()
        .with_backend(store)
        .on_qr_code(|code, timeout| async move {
            let _ = (code, timeout);
        })
        .on_pair_code(|code, timeout| async move {
            let _ = (code, timeout);
        })
        .on_pair_code_error(|err: PairingCodeError, _client| async move {
            let _ = err.rejection;
        })
        .on_connected(|_client| async {})
        .on_logged_out(|_info| async {})
        .on_event(|_event, _client| async {})
        .on_event_for(&[EventKind::Receipt, EventKind::Presence], |_event, _client| async {})
        .on_message(|ctx| async move {
            // Text, quote-reply, reaction, edit, revoke helpers on the context.
            if ctx.message.text_content() == Some("ping") {
                let _ = ctx.reply("pong").await;
                let _ = ctx.reply_quoting("pong (quoted)").await;
                let _ = ctx.react("👍").await;
                let _ = ctx.edit_message("some-id", wa::Message::text("edited")).await;
                let _ =
                    ctx.revoke_message("some-id", whatsapp_rust::send::RevokeType::Sender)
                        .await;
            }
            // Raw pass-through use of the client + message.
            let _ = ctx
                .client
                .send_text(ctx.info.source.chat.clone(), "direct")
                .await;
            let _key = ctx.message_key();
            let _chat = ctx.info.source.chat.clone();
            let _sender = ctx.info.source.sender.clone();
            let _id = ctx.info.id.clone();
            let _ts = ctx.info.timestamp;
        })
        .with_event_delivery(EventDelivery::Ordered { capacity: 256 })
        .with_version((2, 3000, 1033952532))
        .with_wanted_pre_key_count(812)
        .with_resend_rate_limit(20, 10)
        .skip_history_sync()
        .with_push_name("my device");

    builder = builder.with_pair_code(PairCodeOptions {
        phone_number: "15551234567".to_string(),
        custom_code: None,
        show_push_notification: true,
        ..Default::default()
    });

    let bot = builder.build().await?;
    let handle: BotHandle = bot.spawn();
    let _client: Arc<Client> = handle.client();
    handle.shutdown().await;
    Ok(())
}

async fn client_surface(client: &Arc<Client>, chat: Jid) -> Result<(), Box<dyn std::error::Error>> {
    // --- lifecycle ----------------------------------------------------------
    client.connect().await?;
    let _ = client.wait_for_socket(Duration::from_secs(30)).await;
    let _ = client.wait_for_connected(Duration::from_secs(30)).await;
    let _ = client.is_connected();
    let _ = client.is_logged_in();
    let _ = client.push_name();
    let _ = client.pn();
    let _ = client.lid();
    let _ = client.stats();
    let _ = client.memory_report().await;
    let _ = client.resource_report().await;

    // --- sending ------------------------------------------------------------
    let sent = client.send_text(chat.clone(), "hello").await?;
    let _id: String = sent.message_id.clone();
    let _to: Jid = sent.to.clone();
    let _key = sent.message_key();
    let msg = wa::Message::text("hi");
    let _ = client.send_message(chat.clone(), msg.clone()).await?;
    let _ = client
        .send_message_with_options(
            chat.clone(),
            msg.clone(),
            SendOptions::default()
                .with_message_id("custom-id")
                .with_ephemeral_expiration(86_400),
        )
        .await?;
    let _ = client.forward_message(chat.clone(), &msg).await?;

    // --- replies (quoting) --------------------------------------------------
    let quote_ctx = whatsapp_rust::wacore::proto_helpers::build_quote_context_with_info(
        "original-id",
        &chat,
        &chat,
        &chat,
        &msg,
    );
    let _ = client
        .send_message(chat.clone(), wa::Message::text_with_context("reply", quote_ctx))
        .await?;

    // --- reactions / edits / deletes ---------------------------------------
    let target = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some("target-id".to_string()),
        participant: None,
    };
    let _ = client.send_reaction(chat.clone(), target, "❤️").await?;
    let _ = client
        .edit_message(chat.clone(), "original-id", wa::Message::text("edited"))
        .await?;
    let _ = client
        .revoke_message(chat.clone(), "original-id", whatsapp_rust::RevokeType::Sender)
        .await?;
    let _ = client
        .revoke_message(
            chat.clone(),
            "someone-elses-id",
            whatsapp_rust::send::RevokeType::Admin {
                original_sender: chat.clone(),
            },
        )
        .await;
    let _ = client
        .keep_message(chat.clone(), wa::MessageKey::default(), true)
        .await;
    let _ = client
        .pin_message(
            chat.clone(),
            wa::MessageKey::default(),
            whatsapp_rust::PinDuration::Days7,
        )
        .await;
    let _ = client
        .unpin_message(chat.clone(), wa::MessageKey::default())
        .await;

    // --- media upload + download -------------------------------------------
    let bytes = std::fs::read("/tmp/nonexistent")?;
    let upload = client
        .upload(bytes, MediaType::Image, UploadOptions::default())
        .await?;
    let image_msg = media::image_message(
        upload,
        ImageOptions {
            caption: Some("caption".into()),
            ..Default::default()
        },
    );
    let _ = client.send_message(chat.clone(), image_msg.clone()).await?;
    if let Some(img) = image_msg.image_message.as_option() {
        let data: Vec<u8> = client.download(img as &dyn Downloadable).await?;
        let _ = data.len();
    }

    // --- receipts / presence / chatstate -----------------------------------
    client
        .mark_as_read(&chat, None, &["message-id-1", "message-id-2"])
        .await?;
    client.mark_as_played(&chat, None, &["message-id-1"]).await?;
    client.presence().set_available().await?;
    client.presence().set_unavailable().await?;
    client.presence().subscribe(chat.clone()).await?;
    client.presence().unsubscribe(&chat).await?;
    client.chatstate().send_composing(&chat).await?;
    client.chatstate().send_paused(&chat).await?;
    client.profile().set_push_name("New Name").await?;
    client.profile().set_status_text("About me").await?;
    let _ = client.contacts().get_profile_picture(&chat, true).await?;
    let _ = client.contacts().is_on_whatsapp(&[chat.clone()]).await?;

    // --- groups -------------------------------------------------------------
    let meta = client.groups().get_metadata(&chat).await?;
    let _subject = meta.subject.clone();
    let created = client
        .groups()
        .create_group(
            GroupCreateOptions::new("Test group").with_participants(vec![
                GroupParticipantOptions::new(Jid::pn("15551230000")),
            ]),
        )
        .await?;
    let _gid = created.metadata.id.clone();
    let _link = client.groups().get_invite_link(&chat, false).await?;
    client.groups().add_participants(&chat, &[Jid::pn("15551230001")]).await?;
    client.groups().promote_participants(&chat, &[Jid::pn("15551230001")]).await?;

    // --- newsletters --------------------------------------------------------
    let nl: Jid = "120363000000000000@newsletter".parse()?;
    let _nl_meta = client.newsletter().get_metadata(&nl).await?;
    let _subs = client.newsletter().list_subscribed().await?;
    client.newsletter().set_follower_mute(&nl, true).await?;
    let _msgs = client.newsletter().get_messages(&nl, 50, None).await?;

    // --- event bus: raw handler subscription -------------------------------
    let (handler, rx) = ChannelEventHandler::new();
    let subscription = client.subscribe_handler(handler);
    let _interest: EventInterest = EventInterest::of(&[EventKind::Messages]);
    while let Ok(event) = rx.recv().await {
        if let Some(batch) = event.as_messages() {
            for m in batch {
                let _: &InboundMessage = m;
            }
        }
    }
    subscription.detach();

    // --- logout / disconnect ------------------------------------------------
    client.logout().await;
    client.disconnect().await;
    Ok(())
}

struct DurableHook;

#[whatsapp_rust::async_trait]
impl InboundDurabilityHook for DurableHook {
    async fn on_messages(
        &self,
        _client: Arc<Client>,
        batch: &[InboundMessage],
    ) -> whatsapp_rust::anyhow::Result<()> {
        let _ = batch.len();
        Ok(())
    }
}

fn lower_level_builder(client: Arc<Client>) -> Result<(), Box<dyn std::error::Error>> {
    // The low-level builder is public too (runtime-validated, no typestate).
    let _builder = Client::builder()
        .with_version_override((2, 3000, 1033952532))
        .with_skip_history_sync(true)
        .with_wanted_pre_key_count(812)
        .with_resend_rate_limit(20, 10)
        .with_inbound_durability_hook(DurableHook);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = bot_flow().await;
    Ok(())
}
