//! Shared runtime state for the Tauri host.

use std::sync::Arc;

use whatsapp_core::WaClient;

/// State shared by every Tauri command.
#[derive(Clone)]
pub struct AppState {
    core: Arc<WaClient>,
}

impl AppState {
    /// Wrap the protocol client. The client owns its event bus; the host
    /// subscribes through [`WaClient::subscribe`].
    pub fn new(core: Arc<WaClient>) -> Self {
        Self { core }
    }

    /// The protocol client.
    pub fn core(&self) -> &Arc<WaClient> {
        &self.core
    }
}
