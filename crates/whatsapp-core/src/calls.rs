//! Calls: the signalling and media surface (milestone M7).
//!
//! The upstream crate ships a VoIP stack behind the `voip*` cargo features.
//! See `docs/research/calls-plan.md` for the full integration plan. This
//! module is intentionally a thin, honest skeleton until that lands.

use crate::client::WaClient;
use crate::error::{CoreError, Result};
use crate::types::Jid;

impl WaClient {
    /// Start an outgoing voice or video call.
    pub async fn start_call(&self, _chat_id: &Jid, _video: bool) -> Result<()> {
        Err(CoreError::Internal(
            "calling is not implemented yet (see docs/research/calls-plan.md)".into(),
        ))
    }

    /// Hang up the current call.
    pub async fn end_call(&self, _chat_id: &Jid) -> Result<()> {
        Err(CoreError::Internal(
            "calling is not implemented yet (see docs/research/calls-plan.md)".into(),
        ))
    }
}
