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
    let mut history = sample_chat("a@s.whatsapp.net", "31619446549", 200);
    history.pinned = false;
    history.unread_count = 0;
    store
        .upsert_chat_from_history(&history, false, "31619446549")
        .unwrap();

    let stored = store.list_chats().unwrap().remove(0);
    assert_eq!(stored.name, "Meike");
    assert!(stored.pinned);
    assert_eq!(stored.unread_count, 4);
    assert_eq!(stored.last_activity_ts, 200);

    // A numeric placeholder chat does get a real name from history.
    let numeric = sample_chat("b@s.whatsapp.net", "999", 10);
    store
        .upsert_chat_from_history(&numeric, false, "999")
        .unwrap();
    let real = sample_chat("b@s.whatsapp.net", "Bob", 20);
    store.upsert_chat_from_history(&real, true, "999").unwrap();

    let bob = store
        .list_chats()
        .unwrap()
        .into_iter()
        .find(|chat| chat.id == Jid::new("b@s.whatsapp.net"))
        .expect("bob");
    assert_eq!(bob.name, "Bob");
}

#[test]
fn history_unread_does_not_reinflate_a_read_chat() {
    let store = Store::open_in_memory().expect("open store");
    let mut live = sample_chat("a@s.whatsapp.net", "Meike", 100);
    live.unread_count = 0;
    store.upsert_chat(&live).unwrap();

    let mut history = sample_chat("a@s.whatsapp.net", "Meike", 200);
    history.unread_count = 69;
    store
        .upsert_chat_from_history(&history, true, "316")
        .unwrap();

    let stored = store.list_chats().unwrap().remove(0);
    assert_eq!(stored.unread_count, 0);
}

#[test]
fn lid_and_phone_jids_merge_into_one_chat() {
    let store = Store::open_in_memory().expect("open store");
    let pn = "31619446549@s.whatsapp.net";
    let lid = "254970750308491@lid";

    store
        .upsert_chat(&sample_chat(pn, "Meike", 100))
        .unwrap();
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
