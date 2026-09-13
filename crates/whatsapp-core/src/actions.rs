//! Message-level actions: quoting replies, reactions, edits, revokes, stars.
//!
//! This module extends [`WaClient`] with the message-action surface. It is
//! implemented separately from `client.rs` so it can evolve independently.

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::types::{Jid, Message};

impl WaClient {
    /// Send a text message that quotes an existing message.
    pub async fn send_text_quoting(
        &self,
        _chat_id: &Jid,
        _text: &str,
        _quoted_message_id: &str,
    ) -> Result<Message> {
        Err(CoreError::Internal(
            "message actions are not implemented yet".into(),
        ))
    }

    /// Send (or clear, with an empty emoji) a reaction to a message.
    pub async fn send_reaction(
        &self,
        _chat_id: &Jid,
        _message_id: &str,
        _emoji: &str,
        _from_me: bool,
    ) -> Result<()> {
        Err(CoreError::Internal(
            "message actions are not implemented yet".into(),
        ))
    }

    /// Edit a message this account sent.
    pub async fn edit_message(
        &self,
        _chat_id: &Jid,
        _message_id: &str,
        _new_text: &str,
    ) -> Result<()> {
        Err(CoreError::Internal(
            "message actions are not implemented yet".into(),
        ))
    }

    /// Delete a message, for everyone or just locally.
    pub async fn revoke_message(
        &self,
        _chat_id: &Jid,
        _message_id: &str,
        _for_everyone: bool,
    ) -> Result<()> {
        Err(CoreError::Internal(
            "message actions are not implemented yet".into(),
        ))
    }

    /// Star or unstar a message.
    pub async fn star_message(
        &self,
        _chat_id: &Jid,
        _message_id: &str,
        _from_me: bool,
        _star: bool,
    ) -> Result<()> {
        Err(CoreError::Internal(
            "message actions are not implemented yet".into(),
        ))
    }
}
