//! Notification click-through for chat messages (macOS).
//!
//! `tauri-plugin-notification`'s desktop backend posts notifications through
//! `notify-rust`'s legacy `NSUserNotificationCenter` path. That path drops the
//! `chatId` extra and never talks to `UNUserNotificationCenter`, so no
//! `UNUserNotificationCenterDelegate` would ever see its notifications.
//!
//! On macOS, chat notifications therefore go through `UNUserNotificationCenter`
//! directly (see [`notify_chat`]), carrying the chat id in `userInfo` under the
//! same `chatId` key the plugin uses on mobile. [`install`] registers a
//! delegate once during setup; clicking a notification focuses the main window
//! and routes the chat id through [`crate::deep_link::open_link`], which emits
//! the same `ui://open-chat` event (and cold-start buffer) as a
//! `rustwa://chat/…` deep link, so the frontend needs no extra routing.
//!
//! The plugin is still used for plain [`crate::platform::notify`] calls and as
//! the delivery fallback outside macOS and in unbundled dev builds, where
//! `UNUserNotificationCenter` is unavailable because it requires a real `.app`
//! bundle.
//!
//! No entitlement or `Info.plist` key is required: local notifications and
//! click-through are available to any bundled app once the user grants
//! permission (macOS prompts on the first authorization request).

#![cfg_attr(target_os = "macos", allow(unsafe_code))]

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[cfg(target_os = "macos")]
use {
    objc2::rc::Retained,
    objc2::runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject},
    objc2::{AnyThread, DefinedClass, MainThreadMarker, define_class, msg_send},
    objc2_foundation::{NSBundle, NSDictionary, NSError, NSString},
    objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
        UNNotificationDefaultActionIdentifier, UNNotificationPresentationOptions,
        UNNotificationRequest, UNNotificationResponse, UNUserNotificationCenter,
        UNUserNotificationCenterDelegate,
    },
    std::sync::OnceLock,
    std::sync::atomic::{AtomicBool, Ordering},
    tauri::Manager,
};

/// `userInfo` key carrying the WhatsApp chat id.
const CHAT_ID_KEY: &str = "chatId";

/// Installs the notification delegate exactly once.
///
/// Called from Tauri's setup hook, which runs on the main thread. The
/// `UNUserNotificationCenter` delegate property is weak, so the object is kept
/// alive in [`DELEGATE`] for the process lifetime.
///
/// In an unbundled dev build (`cargo run`, `cargo tauri dev`) the process has
/// no bundle identifier and `UNUserNotificationCenter` is not usable; chat
/// notifications then keep using the plugin's legacy backend, which delivers
/// but cannot report clicks.
#[cfg(target_os = "macos")]
pub fn install(app: &AppHandle) {
    if !bundle_identifier_available() {
        tracing::info!(
            "running outside a .app bundle; UNUserNotificationCenter is unavailable and \
             chat notifications fall back to tauri-plugin-notification"
        );
        return;
    }
    if DELEGATE.get().is_some() {
        return;
    }
    if MainThreadMarker::new().is_none() {
        tracing::warn!("installing the notification delegate off the main thread");
    }

    let delegate = DELEGATE.get_or_init(|| NotificationDelegate::new(app.clone()));
    current_center().setDelegate(Some(ProtocolObject::from_ref(&**delegate)));
    tracing::info!("installed the UNUserNotificationCenter delegate");
}

/// No-op outside macOS: there is no cross-platform click-through backend.
#[cfg(not(target_os = "macos"))]
pub fn install(_app: &AppHandle) {}

/// Shows a desktop notification for `chat_id` that opens the chat when clicked.
#[cfg(target_os = "macos")]
pub fn notify_chat(
    app: &AppHandle,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    if !native_delivery() {
        return plugin_chat_notification(app, chat_id, title, body);
    }
    post_native(chat_id, title, body);
    Ok(())
}

/// Shows a chat notification through the plugin; clicks are not reported there.
#[cfg(not(target_os = "macos"))]
pub fn notify_chat(
    app: &AppHandle,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    plugin_chat_notification(app, chat_id, title, body)
}

/// Whether chat notifications use the native click-through backend.
///
/// Only true after [`install`] has set the delegate, which in turn requires a
/// bundled `.app` process.
#[cfg(target_os = "macos")]
pub fn native_delivery() -> bool {
    DELEGATE.get().is_some()
}

/// Returns whether notifications are authorized, prompting the user when the
/// system has not decided yet.
///
/// The plugin reports `Granted` unconditionally on desktop, so the real state
/// can only come from `UNUserNotificationCenter`.
#[cfg(target_os = "macos")]
pub async fn request_permission() -> Result<bool, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    // A completion block must be `Fn`, so the sender is taken out of a shared
    // slot instead of being moved out of the closure.
    let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
    {
        let handler: block2::RcBlock<dyn Fn(Bool, *mut NSError)> =
            block2::RcBlock::new(move |granted: Bool, _error: *mut NSError| {
                if let Ok(mut slot) = sender.lock()
                    && let Some(sender) = slot.take()
                {
                    let _ = sender.send(granted.as_bool());
                }
            });
        current_center().requestAuthorizationWithOptions_completionHandler(
            authorization_options(),
            &handler,
        );
        // The framework retains its own copy of the block; dropping our handle
        // here keeps this future `Send` for Tauri's async command wrapper.
    }

    receiver
        .await
        .map_err(|error| format!("the notification permission request was dropped: {error}"))
}

/// Legacy plugin delivery, still used outside macOS and in unbundled dev
/// builds. On desktop the plugin ignores `extra`, so a click cannot be
/// correlated with a chat on this path; it is delivery only.
fn plugin_chat_notification(
    app: &AppHandle,
    chat_id: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .extra(CHAT_ID_KEY, chat_id)
        .show()
        .map_err(|error| error.to_string())
}

/// Strong reference keeping the delegate alive: `setDelegate:` stores a weak
/// reference and every response callback must be able to reach the object.
#[cfg(target_os = "macos")]
static DELEGATE: OnceLock<Retained<NotificationDelegate>> = OnceLock::new();

/// Guards the one-time `requestAuthorization` call from [`post_native`].
#[cfg(target_os = "macos")]
static AUTHORIZATION_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Instance variables of [`NotificationDelegate`].
#[cfg(target_os = "macos")]
struct DelegateIvars {
    app: AppHandle,
}

#[cfg(target_os = "macos")]
define_class!(
    // SAFETY:
    // - `NSObject` has no subclassing requirements.
    // - `NotificationDelegate` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    // The delegate is not tied to the main thread: responses and presentation
    // callbacks only touch `AppHandle` (thread-safe) and Rust statics. objc2
    // therefore keeps the class `Send + Sync`, which lets it live in a static.
    #[name = "RustWANotificationDelegate"]
    #[ivars = DelegateIvars]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        /// User clicked the notification (default action): open its chat.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive_notification_response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            self.open_clicked_chat(response);
            completion_handler.call(());
        }

        /// Show the banner and play the sound even while RustWA is
        /// frontmost; macOS suppresses notifications for the active app
        /// otherwise, and the frontend only notifies while hidden anyway.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present_notification(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<
                dyn Fn(UNNotificationPresentationOptions),
            >,
        ) {
            completion_handler.call((
                UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::Sound,
            ));
        }
    }
);

#[cfg(target_os = "macos")]
impl NotificationDelegate {
    fn new(app: AppHandle) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { app });
        // SAFETY: `init` is `NSObject`'s designated initializer and the ivars
        // have been set.
        unsafe { msg_send![super(this), init] }
    }

    /// Routes the clicked notification to the chat it was posted for.
    fn open_clicked_chat(&self, response: &UNNotificationResponse) {
        let action = response.actionIdentifier();
        // SAFETY: `UNNotificationDefaultActionIdentifier` is an immutable
        // framework constant; `isEqualToString:` does not mutate it.
        if !action.isEqualToString(unsafe { UNNotificationDefaultActionIdentifier }) {
            // Dismissals (`UNNotificationDismissActionIdentifier`) must not
            // open a chat.
            return;
        }

        let content = response.notification().request().content();
        let user_info = content.userInfo();
        let key = NSString::from_str(CHAT_ID_KEY);
        let key: &AnyObject = key.as_ref();
        let Some(chat_id) = user_info
            .objectForKey(key)
            .and_then(|value| value.downcast::<NSString>().ok())
            .map(|value| value.to_string())
        else {
            tracing::debug!("clicked notification without a chat id");
            return;
        };

        let app = self.ivars().app.clone();
        focus_main_window(&app);
        // Reuse the deep-link delivery path: it emits `ui://open-chat` and
        // buffers the chat id until the webview is ready on cold start.
        if let Err(error) = crate::deep_link::open_link(
            app,
            format!(
                "rustwa://chat/{}",
                percent_encoding::utf8_percent_encode(&chat_id, percent_encoding::NON_ALPHANUMERIC)
            ),
        ) {
            tracing::warn!(%error, "failed to deliver a notification click");
        }
    }
}

/// Brings the main window to the front before the chat opens.
#[cfg(target_os = "macos")]
fn focus_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // A click already activates the app; make sure a minimized window is
    // restored before the UI handles the event.
    if let Err(error) = window.show() {
        tracing::debug!(%error, "failed to show the main window");
    }
    if let Err(error) = window.unminimize() {
        tracing::debug!(%error, "failed to unminimize the main window");
    }
    if let Err(error) = window.set_focus() {
        tracing::debug!(%error, "failed to focus the main window");
    }
}

/// Posts a chat notification with the chat id in `userInfo`.
#[cfg(target_os = "macos")]
fn post_native(chat_id: &str, title: &str, body: &str) {
    ensure_authorization_requested();

    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    // Groups all notifications of one chat like the official client.
    content.setThreadIdentifier(&NSString::from_str(chat_id));

    let key = NSString::from_str(CHAT_ID_KEY);
    let value = NSString::from_str(chat_id);
    let user_info =
        NSDictionary::<NSString, NSString>::from_retained_objects(&[&*key], &[value]);
    // SAFETY: the dictionary stores `NSString` keys and values, which are
    // valid `AnyObject`s, so reinterpreting the collection generics cannot
    // change what is stored.
    unsafe { content.setUserInfo(user_info.cast_unchecked()) };

    // A unique identifier per message keeps notifications from replacing each
    // other; `threadIdentifier` still groups them per chat.
    let identifier = NSString::from_str(&format!("chat:{chat_id}:{}", timestamp_millis()));
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);

    // Fire and forget: the system delivers (or drops) the request and reports
    // clicks through the delegate.
    current_center().addNotificationRequest_withCompletionHandler(&request, None);
}

/// Requests alert/sound/badge authorization once per process, before the
/// first native post: `UNUserNotificationCenter` drops requests while the
/// user has not decided yet. After the first answer the call is a no-op.
#[cfg(target_os = "macos")]
fn ensure_authorization_requested() {
    if AUTHORIZATION_REQUESTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let handler: block2::RcBlock<dyn Fn(Bool, *mut NSError)> =
        block2::RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
    current_center().requestAuthorizationWithOptions_completionHandler(
        authorization_options(),
        &handler,
    );
}

#[cfg(target_os = "macos")]
fn authorization_options() -> UNAuthorizationOptions {
    UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound | UNAuthorizationOptions::Badge
}

#[cfg(target_os = "macos")]
fn current_center() -> Retained<UNUserNotificationCenter> {
    UNUserNotificationCenter::currentNotificationCenter()
}

/// `UNUserNotificationCenter` requires a process with a bundle identifier;
/// raw `target/debug/rustwa` binaries have none.
#[cfg(target_os = "macos")]
fn bundle_identifier_available() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}

#[cfg(target_os = "macos")]
fn timestamp_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}
