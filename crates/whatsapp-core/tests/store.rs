//! Integration tests for the SQLite store.

use whatsapp_core::{ChatSummary, Jid, Message, MessageKind, MessageStatus, Store};

fn sample_chat(id: &str, name: &str, ts: u64) -> ChatSummary {
    ChatSummary {
        id: Jid::new(id),
        name: name.to_owned(),
        last_message_preview: Some("hello".to_owned()),
        last_activity_ts: ts,
        unread_count: 3,
        muted: false,
        pinned: true,
        is_group: false,
        is_archived: false,
        last_message_kind: None,
        last_status: None,
        last_from_me: false,
    }
}

fn sample_message(id: &str, chat_id: &str, timestamp: u64) -> Message {
    Message {
        id: id.to_owned(),
        chat_id: Jid::new(chat_id),
        sender_id: Jid::new(chat_id),
        from_me: false,
        timestamp,
        kind: MessageKind::Text,
        text: Some(format!("message {id}")),
        status: MessageStatus::Delivered,
        view_once: false,
    }
}

#[test]
fn chat_roundtrip_preserves_fields() {
    let store = Store::open_in_memory().expect("open store");
    let chat = sample_chat("alice@s.whatsapp.net", "Alice", 1_700_000_000);
    store.upsert_chat(&chat).expect("upsert");

    let chats = store.list_chats().expect("list");
    assert_eq!(chats.len(), 1);
    let stored = &chats[0];
    assert_eq!(stored.id, chat.id);
    assert_eq!(stored.name, "Alice");
    assert_eq!(stored.unread_count, 3);
    assert!(stored.pinned);
    assert!(!stored.muted);
}

#[test]
fn chats_are_ordered_pinned_first_then_recent() {
    let store = Store::open_in_memory().expect("open store");

    let mut chat_a = sample_chat("a@s.whatsapp.net", "A", 300);
    chat_a.pinned = false;
    store.upsert_chat(&chat_a).unwrap();

    // B stays pinned, so it must sort first despite the oldest activity.
    store
        .upsert_chat(&sample_chat("b@s.whatsapp.net", "B", 100))
        .unwrap();

    let mut chat_c = sample_chat("c@s.whatsapp.net", "C", 200);
    chat_c.pinned = false;
    store.upsert_chat(&chat_c).unwrap();

    let names: Vec<String> = store
        .list_chats()
        .unwrap()
        .into_iter()
        .map(|chat| chat.name)
        .collect();
    assert_eq!(names, vec!["B", "A", "C"]);
}

#[test]
fn messages_roundtrip_in_chronological_order() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 30))
        .unwrap();
    store
        .upsert_message(&sample_message("m2", chat_id, 20))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", chat_id, 10))
        .unwrap();
    store
        .upsert_message(&sample_message("m3", chat_id, 30))
        .unwrap();

    let messages = store.list_messages(&Jid::new(chat_id), 50).unwrap();
    let ids: Vec<&str> = messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m1", "m2", "m3"]);
    assert_eq!(messages[0].kind, MessageKind::Text);
    assert_eq!(messages[0].status, MessageStatus::Delivered);
}

#[test]
fn older_pages_walk_backwards_without_gaps_or_overlap() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 50))
        .unwrap();
    for (id, ts) in [("m1", 10), ("m2", 20), ("m3", 30), ("m4", 40), ("m5", 50)] {
        store
            .upsert_message(&sample_message(id, chat_id, ts))
            .unwrap();
    }

    // Newest page first (oldest of the page in front), then walk back.
    let page = store.list_messages(&Jid::new(chat_id), 2).unwrap();
    let ids: Vec<&str> = page.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m4", "m5"]);

    let older = store.messages_before(&Jid::new(chat_id), "m4", 2).unwrap();
    let ids: Vec<&str> = older.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m2", "m3"]);

    let rest = store.messages_before(&Jid::new(chat_id), "m2", 2).unwrap();
    let ids: Vec<&str> = rest.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m1"]);

    assert!(
        store
            .messages_before(&Jid::new(chat_id), "m1", 2)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn older_pages_stay_consistent_for_equal_timestamps() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 20))
        .unwrap();
    for id in ["a", "b", "c", "d"] {
        store
            .upsert_message(&sample_message(id, chat_id, 20))
            .unwrap();
    }

    let page = store.list_messages(&Jid::new(chat_id), 2).unwrap();
    let ids: Vec<&str> = page.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["c", "d"]);

    let older = store.messages_before(&Jid::new(chat_id), "c", 2).unwrap();
    let ids: Vec<&str> = older.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn oldest_message_reports_the_history_anchor() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 30))
        .unwrap();
    assert!(store.oldest_message(&Jid::new(chat_id)).unwrap().is_none());

    let mut first = sample_message("m1", chat_id, 10);
    first.from_me = true;
    store.upsert_message(&first).unwrap();
    store
        .upsert_message(&sample_message("m2", chat_id, 30))
        .unwrap();

    let (id, from_me, timestamp) = store
        .oldest_message(&Jid::new(chat_id))
        .unwrap()
        .expect("anchor");
    assert_eq!(id, "m1");
    assert!(from_me);
    assert_eq!(timestamp, 10);
}

#[test]
fn message_status_can_be_updated() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 10))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", chat_id, 10))
        .unwrap();

    store
        .set_message_status("m1", MessageStatus::Read)
        .expect("update");

    let stored = store.find_message("m1").unwrap().expect("exists");
    assert_eq!(stored.status, MessageStatus::Read);
    assert_eq!(store.message_count().unwrap(), 1);
}

#[test]
fn message_status_only_upgrades_never_downgrades() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 10))
        .unwrap();

    // A Delivered message: an out-of-order Sender receipt must not pull it
    // back to Sent.
    store
        .upsert_message(&sample_message("m1", chat_id, 10))
        .unwrap();
    store
        .set_message_status("m1", MessageStatus::Delivered)
        .unwrap();
    store.set_message_status("m1", MessageStatus::Sent).unwrap();
    assert_eq!(
        store.find_message("m1").unwrap().unwrap().status,
        MessageStatus::Delivered
    );

    // Read upgrades Delivered; a late Delivered receipt keeps Read.
    store.set_message_status("m1", MessageStatus::Read).unwrap();
    store
        .set_message_status("m1", MessageStatus::Delivered)
        .unwrap();
    assert_eq!(
        store.find_message("m1").unwrap().unwrap().status,
        MessageStatus::Read
    );

    // Pending is the bottom of the ladder: it never overrides anything.
    store
        .set_message_status("m1", MessageStatus::Pending)
        .unwrap();
    assert_eq!(
        store.find_message("m1").unwrap().unwrap().status,
        MessageStatus::Read
    );

    // Equal statuses are idempotent, not errors.
    store.set_message_status("m1", MessageStatus::Read).unwrap();
    assert_eq!(
        store.find_message("m1").unwrap().unwrap().status,
        MessageStatus::Read
    );

    // Failed is terminal: no later receipt resurrects a failed send.
    store
        .upsert_message(&sample_message("m2", chat_id, 20))
        .unwrap();
    store
        .set_message_status("m2", MessageStatus::Failed)
        .unwrap();
    store.set_message_status("m2", MessageStatus::Read).unwrap();
    assert_eq!(
        store.find_message("m2").unwrap().unwrap().status,
        MessageStatus::Failed
    );

    // The normal pending → sent ladder still works.
    let mut m3 = sample_message("m3", chat_id, 30);
    m3.status = MessageStatus::Pending;
    store.upsert_message(&m3).unwrap();
    store.set_message_status("m3", MessageStatus::Sent).unwrap();
    assert_eq!(
        store.find_message("m3").unwrap().unwrap().status,
        MessageStatus::Sent
    );
}

#[test]
fn chat_flags_and_read_state_can_be_updated() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 10))
        .unwrap();

    let jid = Jid::new(chat_id);
    store.set_chat_pinned(&jid, false).unwrap();
    store.set_chat_muted(&jid, true).unwrap();
    store.set_chat_archived(&jid, true).unwrap();
    store.mark_chat_read(&jid).unwrap();

    let mut chats = store.list_chats().unwrap();
    let stored = chats.remove(0);
    assert!(!stored.pinned);
    assert!(stored.muted);
    assert!(stored.is_archived);
    assert_eq!(stored.unread_count, 0);
}

#[test]
fn push_names_replace_numeric_placeholders() {
    let store = Store::open_in_memory().expect("open store");
    let jid = Jid::new("254970750308491@lid");

    // History creates the chat with a numeric placeholder name.
    store
        .record_message_activity(&jid, "[Message]", 100, None, false)
        .unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(chats[0].name, "254970750308491");

    // A real push name later replaces it.
    store
        .record_message_activity(&jid, "hoi", 200, Some("Meike"), true)
        .unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(chats[0].name, "Meike");
    assert_eq!(chats[0].unread_count, 1);

    // A different push name does not clobber the resolved name.
    store
        .record_message_activity(&jid, "hoi weer", 300, Some("Meike (werk)"), false)
        .unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(chats[0].name, "Meike");
}

#[test]
fn searches_message_text_ignoring_like_wildcards() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 10))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", chat_id, 10))
        .unwrap();
    // "message m1" from the fixture; add one with wildcard characters.
    let mut tricky = sample_message("m2", chat_id, 20);
    tricky.text = Some("100% done_now".to_owned());
    store.upsert_message(&tricky).unwrap();

    let hits = store.search_messages("message", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "m1");

    // A literal '%' must not act as a wildcard.
    let hits = store.search_messages("100%", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "m2");

    let hits = store.search_messages("done_now", 10).unwrap();
    assert_eq!(hits.len(), 1);
}

#[test]
fn history_merge_preserves_names_and_flags() {
    let store = Store::open_in_memory().expect("open store");

    // Live state: a resolved name, pinned, unread, recent activity.
    let mut live = sample_chat("a@s.whatsapp.net", "Meike", 100);
    live.pinned = true;
    live.unread_count = 4;
    store.upsert_chat(&live).unwrap();

    // A later history chunk with no name and zeroed flags must not clobber it.
    // The chunk carries no unread counter (like an older-message backfill), so
    // the locally accumulated count stays.
    let mut history = sample_chat("a@s.whatsapp.net", "31619446549", 200);
    history.pinned = false;
    history.unread_count = 0;
    store
        .upsert_chat_from_history(&history, false, "31619446549", None)
        .unwrap();

    let stored = store.list_chats().unwrap().remove(0);
    assert_eq!(stored.name, "Meike");
    assert!(stored.pinned);
    assert_eq!(stored.unread_count, 4);
    assert_eq!(stored.last_activity_ts, 200);

    // A numeric placeholder chat does get a real name from history.
    let numeric = sample_chat("b@s.whatsapp.net", "999", 10);
    store
        .upsert_chat_from_history(&numeric, false, "999", None)
        .unwrap();
    let real = sample_chat("b@s.whatsapp.net", "Bob", 20);
    store
        .upsert_chat_from_history(&real, true, "999", None)
        .unwrap();

    let bob = store
        .list_chats()
        .unwrap()
        .into_iter()
        .find(|chat| chat.id == Jid::new("b@s.whatsapp.net"))
        .expect("bob");
    assert_eq!(bob.name, "Bob");
}

#[test]
fn history_server_unread_replaces_the_local_counter() {
    // The server counter is the source of truth: a locally inflated count
    // (unread accumulated before history synced) must converge to whatever
    // the sync chunk carries — including a lower value.
    let store = Store::open_in_memory().expect("open store");
    let mut live = sample_chat("a@s.whatsapp.net", "Meike", 100);
    live.unread_count = 12;
    store.upsert_chat(&live).unwrap();

    let mut history = sample_chat("a@s.whatsapp.net", "Meike", 200);
    history.unread_count = 5;
    store
        .upsert_chat_from_history(&history, true, "316", Some(5))
        .unwrap();
    let stored = store.list_chats().unwrap().remove(0);
    assert_eq!(stored.unread_count, 5);

    // A later chunk that clears the counter clears the badge too.
    let mut read = sample_chat("a@s.whatsapp.net", "Meike", 300);
    read.unread_count = 0;
    store
        .upsert_chat_from_history(&read, true, "316", Some(0))
        .unwrap();
    let stored = store.list_chats().unwrap().remove(0);
    assert_eq!(stored.unread_count, 0);
}

#[test]
fn history_chunk_without_counter_keeps_local_unread() {
    // Chunks that carry no unread counter (older-message backfills) must not
    // zero the badge of a chat the server has not counted in that chunk.
    let store = Store::open_in_memory().expect("open store");
    let mut live = sample_chat("a@s.whatsapp.net", "Meike", 100);
    live.unread_count = 3;
    store.upsert_chat(&live).unwrap();

    let mut backfill = sample_chat("a@s.whatsapp.net", "Meike", 200);
    backfill.unread_count = 0;
    store
        .upsert_chat_from_history(&backfill, true, "316", None)
        .unwrap();

    let stored = store.list_chats().unwrap().remove(0);
    assert_eq!(stored.unread_count, 3);
}

#[test]
fn lid_and_phone_jids_merge_into_one_chat() {
    let store = Store::open_in_memory().expect("open store");
    let pn = "31619446549@s.whatsapp.net";
    let lid = "254970750308491@lid";

    store.upsert_chat(&sample_chat(pn, "Meike", 100)).unwrap();
    store
        .upsert_message(&sample_message("m-pn", pn, 10))
        .unwrap();

    let mut lid_chat = sample_chat(lid, "254970750308491", 200);
    lid_chat.unread_count = 5;
    store.upsert_chat(&lid_chat).unwrap();
    store
        .upsert_message(&sample_message("m-lid", lid, 20))
        .unwrap();

    let canonical = store
        .link_jids(&Jid::new(pn), &Jid::new(lid))
        .unwrap()
        .expect("merged");
    assert_eq!(canonical.as_str(), pn);
    assert_eq!(store.canonical_jid(&Jid::new(lid)).unwrap().as_str(), pn);

    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].name, "Meike");

    let via_pn = store.list_messages(&Jid::new(pn), 10).unwrap();
    let via_lid = store.list_messages(&Jid::new(lid), 10).unwrap();
    let ids: Vec<&str> = via_pn.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m-pn", "m-lid"]);
    assert_eq!(via_lid.len(), 2);
}

#[test]
fn opening_twice_is_idempotent() {
    // The same in-memory store cannot be reopened, but re-running migrations
    // on one connection must be a no-op. This guards the user_version logic.
    let store = Store::open_in_memory().expect("open store");
    store.migrate_for_tests().expect("second migrate");
    assert_eq!(store.list_chats().unwrap().len(), 0);
}

#[test]
fn flag_patches_apply_to_the_canonical_alias_row() {
    let store = Store::open_in_memory().expect("open store");
    let pn = "31619446549@s.whatsapp.net";
    let lid = "254970750308491@lid";

    store.upsert_chat(&sample_chat(pn, "Meike", 100)).unwrap();
    // Register the LID ↔ phone identity the way `link_jids` does.
    let canonical = store
        .link_jids(&Jid::new(lid), &Jid::new(pn))
        .unwrap()
        .expect("merged");
    assert_eq!(canonical.as_str(), pn);

    // An app-state patch addressed to the alias JID must land on the
    // canonical chat row, not fork a second one.
    let alias = Jid::new(lid);
    store.set_chat_archived(&alias, true).unwrap();
    store.set_chat_muted(&alias, true).unwrap();
    store.set_chat_pinned(&alias, false).unwrap();
    store.mark_chat_read(&alias).unwrap();

    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1, "no duplicate chat row for the alias");
    let stored = &chats[0];
    assert_eq!(stored.id.as_str(), pn);
    assert!(stored.is_archived);
    assert!(stored.muted);
    assert!(!stored.pinned);
    assert_eq!(stored.unread_count, 0);
}

#[test]
fn flag_patches_create_a_deferred_chat_row_before_history_sync() {
    let store = Store::open_in_memory().expect("open store");
    let jid = Jid::new("alice@s.whatsapp.net");

    // The archive patch arrives before the chat row exists.
    assert!(store.set_chat_archived(&jid, true).unwrap(), "row created");

    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert!(chats[0].is_archived);
    assert_eq!(chats[0].name, "");
    assert_eq!(chats[0].last_activity_ts, 0);

    // A second patch updates the existing row: not deferred anymore.
    assert!(!store.set_chat_muted(&jid, true).unwrap());

    // The later history upsert must not resurrect the archive flag or the
    // cleared mute flag.
    let mut history = sample_chat("alice@s.whatsapp.net", "Alice", 500);
    history.is_archived = false;
    history.muted = false;
    store
        .upsert_chat_from_history(&history, false, "alice", None)
        .unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert!(chats[0].is_archived, "history keeps the live archive flag");
    assert!(chats[0].muted, "history keeps the live mute flag");
    assert_eq!(chats[0].last_activity_ts, 500);
}

#[test]
fn read_patches_create_deferred_rows_and_support_unread() {
    let store = Store::open_in_memory().expect("open store");
    let jid = Jid::new("alice@s.whatsapp.net");

    // A read patch before the row exists creates a deferred row.
    assert!(store.mark_chat_read(&jid).unwrap(), "row created");
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].unread_count, 0);

    // The phone marked the chat unread: at least one unread must show.
    store.mark_chat_unread(&jid).unwrap();
    assert_eq!(store.list_chats().unwrap()[0].unread_count, 1);

    // An existing count is never lowered by a mark-unread patch.
    let mut chat = sample_chat("alice@s.whatsapp.net", "Alice", 10);
    chat.unread_count = 7;
    store.upsert_chat(&chat).unwrap();
    assert!(!store.mark_chat_unread(&jid).unwrap(), "row exists");
    assert_eq!(store.list_chats().unwrap()[0].unread_count, 7);
}

#[test]
fn reactions_arriving_early_are_buffered_then_drained() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = "alice@s.whatsapp.net";

    // Reactions for a message that does not exist yet.
    store
        .buffer_pending_reaction(
            &Jid::new(chat_id),
            "m1",
            &Jid::new("bob@s.whatsapp.net"),
            "👍",
            100,
        )
        .unwrap();
    store
        .buffer_pending_reaction(
            &Jid::new(chat_id),
            "m1",
            &Jid::new("carol@s.whatsapp.net"),
            "❤️",
            101,
        )
        .unwrap();
    // The same actor reacting again replaces the buffered value.
    store
        .buffer_pending_reaction(
            &Jid::new(chat_id),
            "m1",
            &Jid::new("bob@s.whatsapp.net"),
            "🎉",
            102,
        )
        .unwrap();

    // Draining before the message exists keeps everything buffered.
    assert_eq!(store.drain_pending_reactions("m1").unwrap(), 0);

    // The message arrives (history sync or live traffic) and drains the
    // buffer.
    store
        .upsert_chat(&sample_chat(chat_id, "Alice", 200))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", chat_id, 150))
        .unwrap();
    assert_eq!(store.drain_pending_reactions("m1").unwrap(), 2);

    let reactions = store.reactions_for_chat(&Jid::new(chat_id)).unwrap();
    assert!(reactions.contains(&(
        "m1".to_owned(),
        "bob@s.whatsapp.net".to_owned(),
        "🎉".to_owned()
    )));
    assert!(reactions.contains(&(
        "m1".to_owned(),
        "carol@s.whatsapp.net".to_owned(),
        "❤️".to_owned()
    )));

    // The buffer is empty after draining; a second pass does nothing.
    assert_eq!(store.drain_pending_reactions("m1").unwrap(), 0);

    // A buffered removal (empty emoji) clears the stored reaction.
    store
        .buffer_pending_reaction(
            &Jid::new(chat_id),
            "m1",
            &Jid::new("bob@s.whatsapp.net"),
            "",
            300,
        )
        .unwrap();
    assert_eq!(store.drain_pending_reactions("m1").unwrap(), 1);
    let reactions = store.reactions_for_chat(&Jid::new(chat_id)).unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].1, "carol@s.whatsapp.net");
}

#[test]
fn upserting_a_message_keeps_its_original_chat() {
    let store = Store::open_in_memory().expect("open store");
    store
        .upsert_chat(&sample_chat("a@s.whatsapp.net", "A", 10))
        .unwrap();
    store
        .upsert_chat(&sample_chat("b@s.whatsapp.net", "B", 10))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", "a@s.whatsapp.net", 10))
        .unwrap();

    // A duplicate upsert under another (alias) chat id must not move the
    // message out of its original conversation.
    store
        .upsert_message(&sample_message("m1", "b@s.whatsapp.net", 20))
        .unwrap();

    let in_a = store
        .list_messages(&Jid::new("a@s.whatsapp.net"), 10)
        .unwrap();
    assert_eq!(in_a.len(), 1);
    assert_eq!(in_a[0].chat_id.as_str(), "a@s.whatsapp.net");
    assert_eq!(in_a[0].status, MessageStatus::Delivered);
    let in_b = store
        .list_messages(&Jid::new("b@s.whatsapp.net"), 10)
        .unwrap();
    assert!(in_b.is_empty(), "chat id must stay authoritative");
}

#[test]
fn linking_jids_moves_call_log_rows() {
    use whatsapp_core::calls::{CallLogRecord, CallLogStore, CallOutcome};

    let store = Store::open_in_memory().expect("open store");
    let pn = "31619446549@s.whatsapp.net";
    let lid = "254970750308491@lid";

    store.upsert_chat(&sample_chat(pn, "Meike", 100)).unwrap();
    let mut lid_chat = sample_chat(lid, "254970750308491", 200);
    lid_chat.unread_count = 1;
    store.upsert_chat(&lid_chat).unwrap();

    store
        .upsert_call_log(&CallLogRecord {
            id: "call-1".to_owned(),
            chat_id: Some(Jid::new(lid)),
            from_me: false,
            video: false,
            outcome: CallOutcome::Missed,
            started_at: 50,
            duration_secs: None,
            raw: None,
        })
        .unwrap();

    let canonical = store
        .link_jids(&Jid::new(pn), &Jid::new(lid))
        .unwrap()
        .expect("merged");
    assert_eq!(canonical.as_str(), pn);

    let rows = store.list_call_log(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].chat_id.as_ref().map(Jid::as_str),
        Some(pn),
        "call log must follow the canonical chat id"
    );
}

#[test]
fn group_chats_ignore_push_name_hints() {
    let store = Store::open_in_memory().expect("open store");
    let group = "120363404062663307@g.us";

    // Live traffic creates the group row: the push name of whoever spoke
    // last must never become the chat name (audit S3).
    store
        .record_message_activity(&Jid::new(group), "hoi", 100, Some("Meike"), true)
        .unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert!(chats[0].is_group);
    assert_eq!(chats[0].name, "120363404062663307");
    assert_eq!(chats[0].unread_count, 1);

    // The placeholder name is picked up by the group-name pass.
    let targets = store.chats_needing_group_names(10).unwrap();
    assert_eq!(targets, vec![Jid::new(group)]);

    // A subject resolves the row, and later push names never clobber it.
    assert!(
        store
            .rename_chat(&Jid::new(group), "Algemene chat")
            .unwrap()
    );
    store
        .record_message_activity(&Jid::new(group), "hoi weer", 200, Some("Senna"), false)
        .unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(chats[0].name, "Algemene chat");
    assert!(store.chats_needing_group_names(10).unwrap().is_empty());

    // Direct chats keep using push names.
    let direct = "31657632048@s.whatsapp.net";
    store
        .record_message_activity(&Jid::new(direct), "hoi", 300, Some("Robert"), false)
        .unwrap();
    let chats = store.list_chats().unwrap();
    let direct_chat = chats
        .iter()
        .find(|chat| chat.id.as_str() == direct)
        .unwrap();
    assert_eq!(direct_chat.name, "Robert");
}

#[test]
fn migration_010_backfill_resets_poisoned_group_names() {
    // Build a v9-shaped database whose group rows carry push names instead
    // of subjects, exactly the state the audit found in the wild.
    let dir = std::env::temp_dir().join(format!(
        "rustwa-store-migration-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rustwa.db");

    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE chats (
                     id TEXT PRIMARY KEY,
                     name TEXT NOT NULL DEFAULT '',
                     last_message_preview TEXT,
                     last_activity_ts INTEGER NOT NULL DEFAULT 0,
                     unread_count INTEGER NOT NULL DEFAULT 0,
                     muted INTEGER NOT NULL DEFAULT 0,
                     pinned INTEGER NOT NULL DEFAULT 0,
                     is_group INTEGER NOT NULL DEFAULT 0,
                     is_archived INTEGER NOT NULL DEFAULT 0,
                     last_kind TEXT,
                     last_from_me INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE messages (
                     id TEXT PRIMARY KEY,
                     chat_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
                     sender_id TEXT NOT NULL,
                     from_me INTEGER NOT NULL,
                     timestamp INTEGER NOT NULL,
                     kind TEXT NOT NULL,
                     text TEXT,
                     status TEXT NOT NULL DEFAULT 'pending'
                 );
                 INSERT INTO chats (id, name, is_group)
                     VALUES ('120363404062663307@g.us', 'Meike', 1);
                 INSERT INTO chats (id, name, is_group)
                     VALUES ('120363404062663307@lid', 'Senna', 0);
                 INSERT INTO chats (id, name, is_group)
                     VALUES ('31657632048@s.whatsapp.net', 'Robert', 0);
                 PRAGMA user_version = 9;",
            )
            .unwrap();
    }

    let store = Store::open(&path).unwrap();
    let chats = store.list_chats().unwrap();
    let by_id = |id: &str| chats.iter().find(|chat| chat.id.as_str() == id).unwrap();

    // Every group row (including the reclassified group-LID row) is reset to
    // a placeholder so the group-metadata pass re-fetches subjects.
    assert_eq!(by_id("120363404062663307@g.us").name, "");
    assert!(by_id("120363404062663307@g.us").is_group);
    assert_eq!(by_id("120363404062663307@lid").name, "");
    assert!(
        by_id("120363404062663307@lid").is_group,
        "group-LID rows must be reclassified as groups"
    );
    // Direct chats keep their name.
    assert_eq!(by_id("31657632048@s.whatsapp.net").name, "Robert");

    // Both group rows are now targets for the repair pass.
    let mut targets = store.chats_needing_group_names(100).unwrap();
    targets.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        targets,
        vec![
            Jid::new("120363404062663307@g.us"),
            Jid::new("120363404062663307@lid"),
        ]
    );

    // Re-running the migrations must be a no-op.
    store.migrate_for_tests().unwrap();
    assert_eq!(store.list_chats().unwrap().len(), 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn group_lid_twins_merge_onto_the_g_us_row() {
    let store = Store::open_in_memory().expect("open store");
    let gus = "120363404062663307@g.us";
    let lid = "120363404062663307@lid";

    let mut gus_chat = sample_chat(gus, "Algemene chat", 100);
    gus_chat.is_group = true;
    store.upsert_chat(&gus_chat).unwrap();
    store
        .upsert_message(&sample_message("m-gus", gus, 10))
        .unwrap();

    // The LID twin was misclassified as a direct chat by the old classifier.
    let mut lid_chat = sample_chat(lid, "120363404062663307", 200);
    lid_chat.is_group = false;
    lid_chat.unread_count = 5;
    store.upsert_chat(&lid_chat).unwrap();
    store
        .upsert_message(&sample_message("m-lid", lid, 20))
        .unwrap();

    let merged = store.merge_group_lid_twins().unwrap();
    assert_eq!(merged, vec![Jid::new(gus)]);

    // The @g.us row survives (non-LID form wins), the LID row is gone and
    // registered as its alias, and the messages live together.
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id.as_str(), gus);
    assert!(chats[0].is_group);
    assert_eq!(chats[0].unread_count, 5);
    assert_eq!(chats[0].name, "Algemene chat");
    assert_eq!(
        store.canonical_jid(&Jid::new(lid)).unwrap().as_str(),
        gus,
        "the LID form must be registered as an alias"
    );
    let via_gus = store.list_messages(&Jid::new(gus), 10).unwrap();
    let ids: Vec<&str> = via_gus.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["m-gus", "m-lid"]);

    // Without a twin there is nothing to merge, and a second run is a no-op.
    assert!(store.merge_group_lid_twins().unwrap().is_empty());
}

#[test]
fn group_lid_rows_without_a_twin_are_left_alone() {
    let store = Store::open_in_memory().expect("open store");
    let lid = "120363404062663307@lid";

    let mut chat = sample_chat(lid, "120363404062663307", 100);
    chat.is_group = true;
    store.upsert_chat(&chat).unwrap();
    store
        .upsert_message(&sample_message("m-lid", lid, 10))
        .unwrap();

    // No @g.us row and no @g.us messages: nothing to merge onto.
    let merged = store.merge_group_lid_twins().unwrap();
    assert!(merged.is_empty());
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id.as_str(), lid);
    assert_eq!(
        store.canonical_jid(&Jid::new(lid)).unwrap().as_str(),
        lid,
        "no alias may be registered without a twin"
    );
}

#[test]
fn contact_backfill_creates_rows_only_for_named_contacts() {
    let store = Store::open_in_memory().expect("open store");

    // Named contacts without a chat row: the backfill's target audience.
    store
        .upsert_contact(
            &Jid::new("31657632048@s.whatsapp.net"),
            Some("Vincent"),
            None,
        )
        .unwrap();
    store
        .upsert_contact(&Jid::new("31612345678@s.whatsapp.net"), Some("Levi"), None)
        .unwrap();
    // Push-name-only contact: must be skipped (a stale push name is not a
    // chat-worthy name).
    store
        .upsert_contact(&Jid::new("31699999999@s.whatsapp.net"), None, Some("Sasha"))
        .unwrap();
    // An existing chat row must never be touched.
    store
        .upsert_chat(&sample_chat("31688888888@s.whatsapp.net", "Existing", 100))
        .unwrap();
    store
        .upsert_contact(&Jid::new("31688888888@s.whatsapp.net"), Some("Iwan"), None)
        .unwrap();

    let candidates = store.contacts_missing_chats(100).unwrap();
    let ids: Vec<&str> = candidates
        .iter()
        .map(|(jid, _)| jid.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2, "only named contacts without a chat row");
    assert!(ids.contains(&"31657632048@s.whatsapp.net"));
    assert!(ids.contains(&"31612345678@s.whatsapp.net"));

    for (jid, name) in &candidates {
        assert!(store.upsert_chat_from_contact(jid, name).unwrap());
    }

    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 3);
    let vincent = chats
        .iter()
        .find(|chat| chat.id.as_str() == "31657632048@s.whatsapp.net")
        .expect("Vincent row");
    assert_eq!(vincent.name, "Vincent");
    assert!(!vincent.is_group);
    assert_eq!(vincent.last_activity_ts, 0);

    // A second run finds nothing new: existing rows are never rewritten.
    assert!(store.contacts_missing_chats(100).unwrap().is_empty());
    assert!(
        !store
            .upsert_chat_from_contact(&Jid::new("31657632048@s.whatsapp.net"), "Different Name",)
            .unwrap()
    );
    assert_eq!(
        store
            .list_chats()
            .unwrap()
            .iter()
            .find(|chat| chat.id.as_str() == "31657632048@s.whatsapp.net")
            .unwrap()
            .name,
        "Vincent"
    );
}

#[test]
fn contact_backfill_rows_resolve_group_flag_and_aliases() {
    let store = Store::open_in_memory().expect("open store");

    // An alias row must land on the canonical chat id.
    store
        .upsert_contact(&Jid::new("14083231338501@lid"), Some("jacques Opa"), None)
        .unwrap();
    store
        .link_jids(
            &Jid::new("14083231338501@lid"),
            &Jid::new("31641234567@s.whatsapp.net"),
        )
        .unwrap();

    let candidates = store.contacts_missing_chats(10).unwrap();
    assert_eq!(candidates.len(), 1);
    assert!(
        store
            .upsert_chat_from_contact(&candidates[0].0, &candidates[0].1)
            .unwrap()
    );
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id.as_str(), "31641234567@s.whatsapp.net");
}

#[test]
fn contact_backfill_dedupes_lid_and_phone_contacts() {
    // The address book stores the same person under both forms (the contact
    // update carries pn_jid and lid_jid). No alias exists yet, so the
    // selector offers both; the backfill must produce one canonical row.
    let store = Store::open_in_memory().expect("open store");
    let pn = Jid::new("31643784576@s.whatsapp.net");
    let lid = Jid::new("134625900925100@lid");
    store.upsert_contact(&pn, Some("Levi"), None).unwrap();
    store.upsert_contact(&lid, Some("Levi"), None).unwrap();

    let candidates = store.contacts_missing_chats(100).unwrap();
    assert_eq!(candidates.len(), 2, "both forms still have no chat row");

    // Order is insertion order; both inserts must land on the same row.
    let mut created = Vec::new();
    for (jid, name) in &candidates {
        if store.upsert_chat_from_contact(jid, name).unwrap() {
            created.push(store.canonical_jid(jid).unwrap());
        }
    }

    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1, "one person, one chat row");
    assert_eq!(
        chats[0].id, pn,
        "the phone-number form is the canonical row"
    );
    assert_eq!(chats[0].name, "Levi");
    assert_eq!(
        store.canonical_jid(&lid).unwrap(),
        pn,
        "the LID must be registered as the PN row's alias"
    );
    // The reported created ids are the canonical row (for history requests).
    assert!(created.contains(&pn));

    // A second backfill run finds nothing left to do.
    assert!(store.contacts_missing_chats(100).unwrap().is_empty());
}

#[test]
fn contact_backfill_resolves_lid_contacts_without_an_alias() {
    // A LID-only contact whose same-named phone-number contact exists:
    // the row is created under the PN id even when the LID contact comes
    // first, and the LID traffic alias is registered.
    let store = Store::open_in_memory().expect("open store");
    let lid = Jid::new("14083231338501@lid");
    store
        .upsert_contact(&lid, Some("jacques Opa"), None)
        .unwrap();
    store
        .upsert_contact(
            &Jid::new("31641234567@s.whatsapp.net"),
            Some("jacques Opa"),
            None,
        )
        .unwrap();

    assert!(store.upsert_chat_from_contact(&lid, "jacques Opa").unwrap());
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id.as_str(), "31641234567@s.whatsapp.net");
    assert_eq!(
        store.canonical_jid(&lid).unwrap().as_str(),
        "31641234567@s.whatsapp.net"
    );

    // A different-named LID contact has no PN counterpart to resolve onto:
    // it keeps its own row (nothing better is known).
    let other = Jid::new("254970750308491@lid");
    store
        .upsert_contact(&other, Some("Zomaar Iemand"), None)
        .unwrap();
    assert!(
        store
            .upsert_chat_from_contact(&other, "Zomaar Iemand")
            .unwrap()
    );
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 2);
    assert!(chats.iter().any(|chat| chat.id == other));
}

#[test]
fn contact_backfill_skips_contacts_whose_alias_row_exists() {
    // Alias already registered (e.g. by usync): the LID contact must not
    // even be selected when the canonical row exists.
    let store = Store::open_in_memory().expect("open store");
    let pn = "31641234567@s.whatsapp.net";
    let lid = "14083231338501@lid";

    store
        .upsert_chat(&sample_chat(pn, "jacques Opa", 100))
        .unwrap();
    store
        .upsert_contact(&Jid::new(pn), Some("jacques Opa"), None)
        .unwrap();
    store
        .upsert_contact(&Jid::new(lid), Some("jacques Opa"), None)
        .unwrap();
    store.link_jids(&Jid::new(lid), &Jid::new(pn)).unwrap();

    assert!(store.contacts_missing_chats(100).unwrap().is_empty());
    assert!(
        !store
            .upsert_chat_from_contact(&Jid::new(lid), "jacques Opa")
            .unwrap()
    );
    assert_eq!(store.list_chats().unwrap().len(), 1);
}

#[test]
fn empty_contact_backfill_rows_are_swept() {
    // Convergence cleanup: contact-sourced rows that never received a
    // message are not real conversations. Rows with a message, with
    // activity, with an unread counter, with another name source, or
    // excluded as in-flight must all survive.
    let store = Store::open_in_memory().expect("open store");

    // Two plain backfill rows: prime sweep candidates.
    store
        .upsert_contact(&Jid::new("31611111111@s.whatsapp.net"), Some("Empty"), None)
        .unwrap();
    store
        .upsert_contact(
            &Jid::new("31622222222@s.whatsapp.net"),
            Some("InFlight"),
            None,
        )
        .unwrap();
    assert!(
        store
            .upsert_chat_from_contact(&Jid::new("31611111111@s.whatsapp.net"), "Empty")
            .unwrap()
    );
    let in_flight = Jid::new("31622222222@s.whatsapp.net");
    assert!(
        store
            .upsert_chat_from_contact(&in_flight, "InFlight")
            .unwrap()
    );

    // A backfill row that did get its history: kept.
    store
        .upsert_contact(
            &Jid::new("31633333333@s.whatsapp.net"),
            Some("Filled"),
            None,
        )
        .unwrap();
    assert!(
        store
            .upsert_chat_from_contact(&Jid::new("31633333333@s.whatsapp.net"), "Filled")
            .unwrap()
    );
    store
        .upsert_message(&sample_message("m1", "31633333333@s.whatsapp.net", 10))
        .unwrap();

    // A live row (activity, unread): kept even with a contact name.
    store
        .record_message_activity(
            &Jid::new("31644444444@s.whatsapp.net"),
            "hoi",
            500,
            Some("Active"),
            true,
        )
        .unwrap();

    // A deferred flag-patch row has a different name source: kept.
    store
        .set_chat_archived(&Jid::new("31655555555@s.whatsapp.net"), true)
        .unwrap();

    let selector = store.empty_contact_chats(100).unwrap();
    assert_eq!(
        selector,
        vec![Jid::new("31611111111@s.whatsapp.net"), in_flight.clone(),],
        "only message-less, inactive contact rows are sweep candidates"
    );

    // The in-flight chat is protected this round.
    let deleted = store
        .delete_empty_contact_chats(&[in_flight.clone()])
        .unwrap();
    assert_eq!(deleted, vec![Jid::new("31611111111@s.whatsapp.net")]);
    let ids: Vec<String> = store
        .list_chats()
        .unwrap()
        .into_iter()
        .map(|chat| chat.id.as_str().to_owned())
        .collect();
    assert!(ids.contains(&in_flight.as_str().to_owned()));
    assert!(ids.contains(&"31633333333@s.whatsapp.net".to_owned()));
    assert!(ids.contains(&"31644444444@s.whatsapp.net".to_owned()));
    assert!(ids.contains(&"31655555555@s.whatsapp.net".to_owned()));

    // After the grace window the protection lapses.
    let deleted = store.delete_empty_contact_chats(&[]).unwrap();
    assert_eq!(deleted, vec![in_flight]);
    let ids: Vec<String> = store
        .list_chats()
        .unwrap()
        .into_iter()
        .map(|chat| chat.id.as_str().to_owned())
        .collect();
    assert_eq!(ids.len(), 3, "only the swept rows are gone");
    assert!(!ids.contains(&"31611111111@s.whatsapp.net".to_owned()));
    assert!(!ids.contains(&"31622222222@s.whatsapp.net".to_owned()));
    // A second sweep finds nothing left.
    assert!(store.delete_empty_contact_chats(&[]).unwrap().is_empty());
}

#[test]
fn apply_push_name_only_renames_weak_direct_rows() {
    let store = Store::open_in_memory().expect("open store");

    // A placeholder direct row (name = JID user part) gets the push name.
    store
        .upsert_chat(&sample_chat(
            "31657632048@s.whatsapp.net",
            "31657632048",
            10,
        ))
        .unwrap();
    assert!(
        store
            .apply_push_name(&Jid::new("31657632048@s.whatsapp.net"), "Vincent")
            .unwrap()
    );
    assert_eq!(store.list_chats().unwrap()[0].name, "Vincent");
    // The push name is also recorded on the contact.
    assert_eq!(
        store
            .contact_name(&Jid::new("31657632048@s.whatsapp.net"))
            .unwrap()
            .as_deref(),
        Some("Vincent")
    );

    // A row that already carries a real push name is updated too (it is the
    // same source strength), but a *new* push name never clobbers a contact
    // name.
    store
        .apply_contact_name(&Jid::new("31657632048@s.whatsapp.net"), "Vincent B.")
        .unwrap();
    assert!(
        !store
            .apply_push_name(&Jid::new("31657632048@s.whatsapp.net"), "Vinnie")
            .unwrap()
    );
    assert_eq!(store.list_chats().unwrap()[0].name, "Vincent B.");

    // Group rows are never renamed by a push name.
    store
        .upsert_chat(&{
            let mut chat = sample_chat("120363404062663307@g.us", "", 10);
            chat.is_group = true;
            chat
        })
        .unwrap();
    assert!(
        !store
            .apply_push_name(&Jid::new("120363404062663307@g.us"), "Someone")
            .unwrap()
    );
    let group = store
        .list_chats()
        .unwrap()
        .into_iter()
        .find(|chat| chat.id.as_str() == "120363404062663307@g.us")
        .unwrap();
    assert_eq!(group.name, "");

    // Empty push names are ignored.
    assert!(
        !store
            .apply_push_name(&Jid::new("31657632048@s.whatsapp.net"), "  ")
            .unwrap()
    );
}

#[test]
fn clear_chat_empties_messages_but_keeps_the_row() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = Jid::new("alice@s.whatsapp.net");
    store
        .upsert_chat(&sample_chat(chat_id.as_str(), "Alice", 30))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", chat_id.as_str(), 10))
        .unwrap();
    store
        .upsert_message(&sample_message("m2", chat_id.as_str(), 20))
        .unwrap();
    store
        .upsert_reaction("m1", &Jid::new("bob@s.whatsapp.net"), "👍")
        .unwrap();

    assert!(store.clear_chat(&chat_id).unwrap());

    assert!(store.list_messages(&chat_id, 50).unwrap().is_empty());
    // The row survives, with preview and unread reset.
    let chat = &store.list_chats().unwrap()[0];
    assert_eq!(chat.name, "Alice");
    assert_eq!(chat.unread_count, 0);
    assert_eq!(chat.last_message_preview, None);
    assert_eq!(chat.last_message_kind, None);

    // Clearing again is a no-op on a missing row.
    assert!(
        !store
            .clear_chat(&Jid::new("missing@s.whatsapp.net"))
            .unwrap()
    );
}

#[test]
fn delete_chat_removes_messages_and_reactions_with_it() {
    let store = Store::open_in_memory().expect("open store");
    let chat_id = Jid::new("alice@s.whatsapp.net");
    store
        .upsert_chat(&sample_chat(chat_id.as_str(), "Alice", 30))
        .unwrap();
    store
        .upsert_message(&sample_message("m1", chat_id.as_str(), 10))
        .unwrap();
    store
        .upsert_reaction("m1", &Jid::new("bob@s.whatsapp.net"), "👍")
        .unwrap();
    store
        .buffer_pending_reaction(
            &chat_id,
            "pending-1",
            &Jid::new("bob@s.whatsapp.net"),
            "🎉",
            5,
        )
        .unwrap();

    assert!(store.delete_chat(&chat_id).unwrap());
    assert!(store.list_chats().unwrap().is_empty());
    assert!(store.list_messages(&chat_id, 50).unwrap().is_empty());
    assert_eq!(store.message_count().unwrap(), 0);
    // Buffered reactions for the deleted chat cannot resurrect it.
    store
        .upsert_chat(&sample_chat(chat_id.as_str(), "Alice", 30))
        .unwrap();
    assert_eq!(
        store.drain_pending_reactions("pending-1").unwrap(),
        0,
        "buffered reactions of a deleted chat must be gone"
    );

    assert!(
        !store
            .delete_chat(&Jid::new("missing@s.whatsapp.net"))
            .unwrap()
    );
}
