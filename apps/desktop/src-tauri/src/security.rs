//! App-lock enforcement (M8): Touch ID / Mac password via `LocalAuthentication`
//! plus the focus clock that decides when the host asks the UI to lock.
//!
//! The *preference* (on/off and timeout) lives in [`crate::app_lock`] so this
//! module cannot clobber other preference writers. This module owns:
//!
//! * [`security_biometry_available`] — can this Mac evaluate device-owner
//!   authentication (Touch ID, or the account password as fallback)?
//! * [`security_authenticate`] — run a standalone authentication prompt.
//! * [`security_unlock`] — authenticate and emit `ui://unlocked` on success.
//! * [`handle_focus_change`] — emit `ui://lock` when the main window regains
//!   focus after the configured `app-lock-timeout`.
//!
//! The frontend pairs these events with `LockScreen.tsx`: it gates the UI at
//! launch by reading [`crate::app_lock::app_lock_enabled`], waits for
//! `ui://lock`, and reveals the app on `ui://unlocked`.
//!
//! # macOS permissions
//!
//! `LocalAuthentication` needs no entitlement or usage-description key for
//! Touch ID / password prompts. The app must be a running (ideally active) GUI
//! process; `LAPolicyDeviceOwnerAuthentication` shows the system sheet and
//! falls back to the account password when Touch ID is missing, not enrolled
//! or locked out.

use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};

/// Emitted to the webview when the app should cover itself with the lock
/// overlay (refocus after the timeout).
pub const LOCK_EVENT: &str = "ui://lock";

/// Emitted to the webview after a successful [`security_unlock`].
pub const UNLOCKED_EVENT: &str = "ui://unlocked";

/// Tracks how long the main window has been unfocused.
///
/// Managed in `lib.rs`; the main window's `Focused` events feed
/// [`handle_focus_change`].
#[derive(Debug, Default)]
pub struct FocusClock {
    hidden_since: Mutex<Option<Instant>>,
}

impl FocusClock {
    pub fn new() -> Self {
        Self::default()
    }

    fn note_hidden(&self) {
        *self
            .hidden_since
            .lock()
            .expect("app-lock focus clock poisoned") = Some(Instant::now());
    }

    fn take_hidden_for(&self) -> Option<std::time::Duration> {
        self.hidden_since
            .lock()
            .expect("app-lock focus clock poisoned")
            .take()
            .map(|hidden_since| hidden_since.elapsed())
    }
}

/// Records a window focus change and emits `ui://lock` when the window comes
/// back after the preference's timeout.
///
/// Called from the `main` window's event hook; safe to call when app lock is
/// disabled (the hidden timestamp is still tracked so enabling the preference
/// takes effect immediately).
pub fn handle_focus_change(app: &AppHandle, focused: bool) {
    let Some(clock) = app.try_state::<FocusClock>() else {
        return;
    };

    if !focused {
        clock.note_hidden();
        return;
    }

    let Some(hidden_for) = clock.take_hidden_for() else {
        return;
    };

    if !crate::app_lock::is_enabled(app) {
        return;
    }
    if hidden_for < crate::app_lock::timeout(app) {
        return;
    }

    if let Err(error) = app.emit(LOCK_EVENT, ()) {
        tracing::warn!(%error, "failed to emit the app-lock event");
    } else {
        tracing::debug!(?hidden_for, "asking the UI to lock after refocus");
    }
}

/// Whether device-owner authentication can run on this machine.
///
/// True when Touch ID or the account password can be used, i.e. app lock can
/// actually be enforced. A `true` here does not guarantee Touch ID is set up —
/// [`authenticate`] falls back to the Mac password.
pub fn biometry_available() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        platform_impl::biometry_available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

/// Runs the system authentication prompt.
///
/// Returns `Ok(())` only after the user authenticated. Cancel, failure and
/// "no biometry/password configured" come back as short human-readable
/// errors; this is deliberately blocking, so callers must not run it on the
/// main thread.
pub fn authenticate(reason: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        platform_impl::authenticate(&reason)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = reason;
        Err("App lock is only supported on macOS in this build.".to_string())
    }
}

/// Can the app show a Touch ID / password prompt on this Mac?
#[tauri::command]
pub fn security_biometry_available() -> Result<bool, String> {
    biometry_available()
}

/// Runs the authentication prompt without emitting any event.
///
/// Used by settings surfaces that want to confirm the user before changing a
/// security preference.
#[tauri::command]
pub async fn security_authenticate(reason: Option<String>) -> Result<(), String> {
    let reason = normalise_reason(reason);
    tauri::async_runtime::spawn_blocking(move || authenticate(reason))
        .await
        .map_err(|error| format!("the authentication task failed: {error}"))?
}

/// Authenticates and, on success, emits `ui://unlocked` to every webview.
///
/// `LockScreen.tsx` invokes this; the event keeps the lock state authoritative
/// for any other listener.
#[tauri::command]
pub async fn security_unlock(app: AppHandle, reason: Option<String>) -> Result<(), String> {
    let reason = normalise_reason(reason);
    let result = tauri::async_runtime::spawn_blocking(move || authenticate(reason))
        .await
        .map_err(|error| format!("the authentication task failed: {error}"))?;
    result?;

    app.emit(UNLOCKED_EVENT, ())
        .map_err(|error| format!("failed to emit {UNLOCKED_EVENT}: {error}"))
}

/// Trims an optional caller-provided prompt reason down to something safe to
/// show in the system sheet. Empty/blank reasons fall back to a generic one.
fn normalise_reason(reason: Option<String>) -> String {
    match reason {
        Some(reason) if !reason.trim().is_empty() => reason.trim().to_string(),
        _ => "unlock your chats".to_string(),
    }
}

#[cfg(target_os = "macos")]
mod platform_impl {
    // The workspace denies `unsafe_code`; this module is the narrow, audited
    // exception: every unsafe call is an Objective-C binding whose safety
    // argument is documented inline.
    #![allow(unsafe_code)]

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAError, LAPolicy};

    /// Touch ID first, account password when biometrics are unavailable,
    /// not enrolled, or locked out.
    const POLICY: LAPolicy = LAPolicy::DeviceOwnerAuthentication;

    pub fn biometry_available() -> Result<bool, String> {
        // SAFETY: `LAContext::new` has no preconditions and returns an owned
        // context, which stays alive for the whole call.
        let context = unsafe { LAContext::new() };
        // The error is only needed for diagnostics; `Err` means the policy
        // cannot be evaluated at all.
        Ok(unsafe { context.canEvaluatePolicy_error(POLICY) }.is_ok())
    }

    pub fn authenticate(reason: &str) -> Result<(), String> {
        // SAFETY: see `biometry_available`.
        let context = unsafe { LAContext::new() };
        if let Err(error) = unsafe { context.canEvaluatePolicy_error(POLICY) } {
            return Err(describe_error(&error));
        }

        let (sender, receiver) = std::sync::mpsc::channel();
        let reply = RcBlock::new(move |success: Bool, error: *mut NSError| {
            let outcome = if success.as_bool() {
                Ok(())
            } else if error.is_null() {
                Err("Authentication failed. Try again.".to_string())
            } else {
                // SAFETY: LocalAuthentication guarantees a valid `NSError`
                // when the evaluation fails and the pointer is non-null.
                Err(describe_error(unsafe { &*error }))
            };
            let _ = sender.send(outcome);
        });

        let reason = NSString::from_str(reason);
        // SAFETY: the reply block is sendable (it only owns an `mpsc::Sender`)
        // and `context` — which the framework requires to stay alive while the
        // evaluation runs — lives until after `receiver.recv()` returns.
        unsafe { context.evaluatePolicy_localizedReason_reply(POLICY, &reason, &reply) };

        receiver.recv().map_err(|_| {
            "Authentication was interrupted before it finished. Try again.".to_string()
        })?
    }

    /// Maps `LAError` codes to short, actionable messages, falling back to the
    /// framework's localized description for everything else.
    fn describe_error(error: &NSError) -> String {
        let code = error.code();
        let known = match code {
            c if c == LAError::UserCancel.0 => Some("Authentication was cancelled."),
            c if c == LAError::UserFallback.0 => Some("Enter your Mac password to unlock."),
            c if c == LAError::SystemCancel.0 => {
                Some("Authentication was interrupted by the system.")
            }
            c if c == LAError::AppCancel.0 => Some("Authentication was cancelled. Try again."),
            c if c == LAError::PasscodeNotSet.0 => {
                Some("Set a login password on this Mac to use app lock.")
            }
            c if c == LAError::BiometryNotAvailable.0 => {
                Some("Touch ID isn't available on this Mac. Use your Mac password instead.")
            }
            c if c == LAError::BiometryNotEnrolled.0 => {
                Some("Set up Touch ID in System Settings to unlock with your fingerprint.")
            }
            c if c == LAError::BiometryLockout.0 => Some(
                "Touch ID is locked after too many attempts. Unlock your Mac with your password, then try again.",
            ),
            c if c == LAError::AuthenticationFailed.0 => {
                Some("Couldn't verify it's you. Try again.")
            }
            c if c == LAError::NotInteractive.0 => {
                Some("Keep RustWA active while you authenticate.")
            }
            c if c == LAError::InvalidContext.0 => {
                Some("Authentication couldn't start. Relaunch RustWA and try again.")
            }
            _ => None,
        };

        if let Some(message) = known {
            return message.to_string();
        }

        let description = error.localizedDescription().to_string();
        if description.trim().is_empty() {
            format!("Authentication failed (LocalAuthentication error {code}).")
        } else {
            description
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_reasons_fall_back_to_the_default_prompt() {
        assert_eq!(normalise_reason(None), "unlock your chats");
        assert_eq!(
            normalise_reason(Some("   ".to_string())),
            "unlock your chats"
        );
    }

    #[test]
    fn explicit_reasons_are_trimmed_and_kept() {
        assert_eq!(
            normalise_reason(Some(" unlock RustWA ".to_string())),
            "unlock RustWA"
        );
    }

    #[test]
    fn focus_clock_reports_the_hidden_duration_once() {
        let clock = FocusClock::new();
        assert!(clock.take_hidden_for().is_none());
        clock.note_hidden();
        let hidden_for = clock.take_hidden_for().expect("hidden duration");
        assert!(hidden_for.as_secs() < 5);
        // The timestamp is consumed by the first refocus.
        assert!(clock.take_hidden_for().is_none());
    }
}
