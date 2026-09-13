//! Shared runtime state for the Tauri host.

use tokio::sync::broadcast;
use whatsapp_core::CoreEvent;

/// Capacity of the core event bus. Large enough to absorb a busy history sync
/// burst; slow consumers resync from the `list_*` commands.
const EVENT_BUS_CAPACITY: usize = 1024;

/// State shared by every Tauri command.
#[derive(Clone)]
pub struct AppState {
    events: broadcast::Sender<CoreEvent>,
}

impl AppState {
    /// Create the state and its event bus.
    pub fn new() -> Self {
        let (events, _receiver) = broadcast::channel(EVENT_BUS_CAPACITY);
        Self { events }
    }

    /// Subscribe to core events. Each subscriber gets every event emitted
    /// after subscribing; the forwarder holds one subscription.
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }

    /// Publish an event to every subscriber.
    pub fn emit(&self, event: CoreEvent) {
        // No subscribers is a normal (early) state, not an error.
        let _ = self.events.send(event);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
