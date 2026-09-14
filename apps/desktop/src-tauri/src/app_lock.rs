//! App-lock preference storage.
//!
//! The preference lives as JSON in the platform config directory
//! (`~/Library/Application Support/<bundle id>/app-lock.json` on macOS), next
//! to the protocol session. It gets its own file so this module cannot clobber
//! other preference writers, and it is written with a write-then-rename so a
//! crash cannot leave half a file behind.
//!
//! Two keys are stored:
//!
//! ```json
//! { "enabled": true, "timeoutSecs": 300 }
//! ```
//!
//! Files written by older builds only contain `enabled`; the missing timeout
//! falls back to [`DEFAULT_TIMEOUT_SECS`] (5 minutes). The unlock prompt and
//! the refocus hook live in [`crate::security`].

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Preference file under the app config directory.
const FILE_NAME: &str = "app-lock.json";

/// How long the window may be hidden before a refocus locks the app again
/// when no timeout has been stored yet.
pub const DEFAULT_TIMEOUT_SECS: u64 = 5 * 60;

/// Shortest accepted timeout. `set_app_lock_timeout` rejects `0` so a
/// malformed value can never lock the app on every refocus.
const MIN_TIMEOUT_SECS: u64 = 1;

/// Longest accepted timeout (24 hours).
const MAX_TIMEOUT_SECS: u64 = 24 * 60 * 60;

/// On-disk shape; keep it tolerant of future keys by only reading what is
/// needed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppLockPreference {
    enabled: bool,
    #[serde(default = "default_timeout_secs")]
    timeout_secs: u64,
}

fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

/// Stores the app-lock preference, preserving any previously stored timeout.
#[tauri::command]
pub fn set_app_lock(app: AppHandle, enabled: bool) -> Result<(), String> {
    let path = preference_path(&app)?;
    let timeout_secs = read_preference(&path).map_or(DEFAULT_TIMEOUT_SECS, |p| p.timeout_secs);
    write_preference(
        &path,
        AppLockPreference {
            enabled,
            timeout_secs,
        },
    )
}

/// Reads the app-lock preference.
///
/// Missing or malformed files report `false` (a failed read must never lock
/// the user out) and log a warning.
#[tauri::command]
pub fn app_lock_enabled(app: AppHandle) -> bool {
    is_enabled(&app)
}

/// Stores how long the window may stay hidden before a refocus locks the app.
///
/// The settings UI offers 5/15/60 minutes; any value in `1..=86400` seconds is
/// accepted so tests and future UIs are not boxed in.
#[tauri::command]
pub fn set_app_lock_timeout(app: AppHandle, secs: u64) -> Result<(), String> {
    if !(MIN_TIMEOUT_SECS..=MAX_TIMEOUT_SECS).contains(&secs) {
        return Err(format!(
            "the app-lock timeout must be between {MIN_TIMEOUT_SECS} and {MAX_TIMEOUT_SECS} seconds"
        ));
    }

    let path = preference_path(&app)?;
    let enabled = read_preference(&path).is_some_and(|p| p.enabled);
    write_preference(
        &path,
        AppLockPreference {
            enabled,
            timeout_secs: secs,
        },
    )
}

/// Reads the configured lock timeout in seconds.
#[tauri::command]
pub fn app_lock_timeout(app: AppHandle) -> u64 {
    timeout_secs(&app)
}

/// Whether the preference is enabled. Missing/malformed files read as `false`.
pub(crate) fn is_enabled(app: &AppHandle) -> bool {
    let path = match preference_path(app) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "failed to resolve the app-lock preference path");
            return false;
        }
    };
    read_preference(&path).is_some_and(|preference| preference.enabled)
}

/// The configured lock timeout, clamped to at least one second.
pub(crate) fn timeout(app: &AppHandle) -> Duration {
    Duration::from_secs(timeout_secs(app).max(MIN_TIMEOUT_SECS))
}

fn timeout_secs(app: &AppHandle) -> u64 {
    let path = match preference_path(app) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "failed to resolve the app-lock preference path");
            return DEFAULT_TIMEOUT_SECS;
        }
    };
    read_preference(&path).map_or(DEFAULT_TIMEOUT_SECS, |preference| preference.timeout_secs)
}

fn read_preference(path: &Path) -> Option<AppLockPreference> {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<AppLockPreference>(&bytes) {
            Ok(preference) => Some(preference),
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "ignoring a malformed app-lock preference");
                None
            }
        },
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "failed to read the app-lock preference");
            None
        }
    }
}

fn write_preference(path: &Path, preference: AppLockPreference) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let json = serde_json::to_vec_pretty(&preference)
        .map_err(|error| format!("failed to encode the app-lock preference: {error}"))?;

    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("failed to replace {}: {error}", path.display()))
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
        let json = serde_json::to_string(&AppLockPreference {
            enabled: true,
            timeout_secs: 900,
        })
        .expect("serialise");
        assert_eq!(json, r#"{"enabled":true,"timeoutSecs":900}"#);

        let parsed: AppLockPreference =
            serde_json::from_str(r#"{"enabled":false}"#).expect("deserialise");
        assert!(!parsed.enabled);
        assert_eq!(parsed.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn timeout_round_trips() {
        let parsed: AppLockPreference =
            serde_json::from_str(r#"{"enabled":true,"timeoutSecs":3600}"#).expect("deserialise");
        assert!(parsed.enabled);
        assert_eq!(parsed.timeout_secs, 3600);
    }
}
