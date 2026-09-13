//! Error type shared by the whole core.

use thiserror::Error;

/// Errors returned by core operations. UI-facing commands convert these into
/// user-presentable messages; the variants stay coarse on purpose.
#[derive(Debug, Error)]
pub enum CoreError {
    /// The operation requires a live connection.
    #[error("not connected to WhatsApp")]
    NotConnected,

    /// The client is already connected or mid-handshake.
    #[error("connection already in progress")]
    AlreadyConnected,

    /// The session cannot be restored and a new pairing is required.
    #[error("pairing required")]
    PairingRequired,

    /// Persistent storage failed.
    #[error("storage error: {0}")]
    Storage(String),

    /// The protocol layer rejected an operation.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Invalid input from the UI or a caller.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Anything else; a bug or an unforeseen upstream state.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience alias used across the crate.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;

/// The protocol layer may surface arbitrary error types. Convert them without
/// losing information.
pub trait IntoCoreError<T> {
    /// Map a foreign error into [`CoreError::Protocol`].
    fn into_core(self) -> Result<T>;
}

impl<T, E: std::fmt::Display> IntoCoreError<T> for std::result::Result<T, E> {
    fn into_core(self) -> Result<T> {
        self.map_err(|error| CoreError::Protocol(error.to_string()))
    }
}
