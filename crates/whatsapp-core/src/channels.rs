//! Channels (newsletters) and status updates.

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::types::Jid;

impl WaClient {
    /// Follow a channel from its invite link.
    pub async fn follow_channel(&self, _invite_url: &str) -> Result<Jid> {
        Err(CoreError::Internal(
            "channel support is not implemented yet".into(),
        ))
    }

    /// Unfollow (leave) a channel.
    pub async fn unfollow_channel(&self, _chat_id: &Jid) -> Result<()> {
        Err(CoreError::Internal(
            "channel support is not implemented yet".into(),
        ))
    }

    /// Send a text message to a channel we administer.
    pub async fn send_channel_message(&self, _chat_id: &Jid, _text: &str) -> Result<()> {
        Err(CoreError::Internal(
            "channel support is not implemented yet".into(),
        ))
    }

    /// Post a plain-text status update.
    pub async fn post_status_text(&self, _text: &str, _background_argb: u32) -> Result<()> {
        Err(CoreError::Internal(
            "status support is not implemented yet".into(),
        ))
    }
}
