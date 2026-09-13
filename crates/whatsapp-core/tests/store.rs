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
fn opening_twice_is_idempotent() {
    // The same in-memory store cannot be reopened, but re-running migrations
    // on one connection must be a no-op. This guards the user_version logic.
    let store = Store::open_in_memory().expect("open store");
    store.migrate_for_tests().expect("second migrate");
    assert_eq!(store.list_chats().unwrap().len(), 0);
}
