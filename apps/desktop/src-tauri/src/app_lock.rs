//! App-lock preference storage (groundwork).
//!
//! The preference lives as JSON in the platform config directory
//! (`~/Library/Application Support/<bundle id>/app-lock.json` on macOS), next
//! to the protocol session. It gets its own file so this module cannot clobber
//! other preference writers, and it is written with a write-then-rename so a
//! crash cannot leave half a file behind.
//!
//! # Not implemented yet
//!
//! Only the preference is persisted. Nothing enforces the lock and there is no
//! biometric prompt. On macOS a future unlock would use `LocalAuthentication`
//! (`LAContext.evaluatePolicy`); no Tauri plugin covers that today —
//! `tauri-plugin-biometric` is iOS/Android only — so it needs an
//! `objc2-local-authentication` binding (or a small Swift/ObjC shim) driven
//! from this module. [`set_app_lock`] and [`app_lock_enabled`] are the stable
//! seam that prompt will sit behind, and [`crate::platform::PlatformCapabilities`]
//! reports the preference as available without claiming biometric unlock.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Preference file under the app config directory.
const FILE_NAME: &str = "app-lock.json";

/// On-disk shape; keep it tolerant of future keys by only reading what is
/// needed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct AppLockPreference {
    enabled: bool,
}

/// Stores the app-lock preference.
///
/// This only records the choice; see the module docs for the unlock work that
/// is still outstanding.
#[tauri::command]
pub fn set_app_lock(app: AppHandle, enabled: bool) -> Result<(), String> {
    let path = preference_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let json = serde_json::to_vec_pretty(&AppLockPreference { enabled })
        .map_err(|error| format!("failed to encode the app-lock preference: {error}"))?;

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("failed to replace {}: {error}", path.display()))
}

/// Reads the app-lock preference.
///
/// Missing or malformed files report `false` (a failed read must never lock
/// the user out) and log a warning.
#[tauri::command]
pub fn app_lock_enabled(app: AppHandle) -> bool {
    let path = match preference_path(&app) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "failed to resolve the app-lock preference path");
            return false;
        }
    };

    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<AppLockPreference>(&bytes) {
            Ok(preference) => preference.enabled,
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "ignoring a malformed app-lock preference");
                false
            }
        },
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read the app-lock preference");
            false
        }
    }
}

fn preference_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(FILE_NAME))
        .map_err(|error| format!("failed to resolve the app config directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_serialises_as_expected_document() {
        let json = serde_json::to_string(&AppLockPreference { enabled: true }).expect("serialise");
        assert_eq!(json, r#"{"enabled":true}"#);
        let parsed: AppLockPreference =
            serde_json::from_str(r#"{"enabled":false}"#).expect("deserialise");
        assert!(!parsed.enabled);
    }
}
