//! `whatsapp-core` — the domain core of RustWA.
//!
//! Everything that is not UI lives here:
//!
//! - connection lifecycle and pairing (via the upstream `whatsapp-rust` crate)
//! - a stable domain event bus consumed by the desktop shell
//! - persistent state (SQLite) and the command surface used by the UI
//!
//! The crate deliberately has no Tauri dependency so it stays testable and
//! reusable from a CLI or a future native UI.

pub mod actions;
pub mod calls;
pub mod channels;
pub mod client;
pub mod contacts;
pub mod error;
pub mod events;
pub mod groups;
pub mod media;
pub mod store;
pub mod types;

pub use client::{ClientConfig, EVENT_BUS_CAPACITY, WaClient};
pub use contacts::ContactProfile;
pub use error::{CoreError, Result};
pub use events::CoreEvent;
pub use groups::GroupInfo;
pub use media::MediaFile;
pub use store::Store;
pub use types::{ChatSummary, Jid, Message, MessageKind, MessageStatus};
