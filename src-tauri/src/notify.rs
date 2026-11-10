//! Site notifications. WebView2 shows them with the sound forced off, so a call never
//! rings when the tab itself is quiet. Handling `NotificationReceived` replaces that
//! silent toast with one Windows toast: a chime, or the looping call sound until the
//! toast is answered, declined, or closed by the page.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2_8, ICoreWebView2_24, ICoreWebView2Notification,
    ICoreWebView2NotificationReceivedEventArgs,
};
use webview2_com::{NotificationCloseRequestedEventHandler, NotificationReceivedEventHandler};
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastActivatedEventArgs, ToastNotification};
use windows::core::{IInspectable, Interface};

use crate::system::{
    NoteAction, ToastSound, create_toast, hide_created, note_action, note_dismiss_arg,
    note_open_arg, notification_is_call, notification_sound, show_created, toast_markup,
};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn native_tabs() -> &'static Mutex<HashSet<u32>> {
    static NATIVE: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();
    NATIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Set on the toast thread when the user clicks, before either event reaches the UI
/// thread. A click also dismisses the toast, and that dismissal must not hang up the call.
fn clicked_toasts() -> &'static Mutex<HashSet<u64>> {
    static CLICKED: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new();
    CLICKED.get_or_init(|| Mutex::new(HashSet::new()))
}

struct Note {
    tab: u32,
    tag: String,
    /// An incoming call. A newer call on this tab replaces it, which stops a ring already playing.
    call: bool,
    toast: ToastNotification,
    notification: Option<ICoreWebView2Notification>,
}

thread_local! {
    static NOTES: RefCell<HashMap<u64, Note>> = RefCell::new(HashMap::new());
}

/// The tab's notifications are shown here, with sound. The script message is then ignored
/// so the page does not get a second, silent toast.
pub fn handles(tab: u32) -> bool {
    native_tabs()
        .lock()
        .expect("site notifications")
        .contains(&tab)
}

pub fn attach(app: &AppHandle, core: &ICoreWebView2, tab: u32) {
    let Ok(core) = core.cast::<ICoreWebView2_24>() else {
        eprintln!("site notifications stay silent: this WebView2 has no NotificationReceived");
        return;
    };
    let app = app.clone();
    let mut token = 0i64;
    let handler = NotificationReceivedEventHandler::create(Box::new(move |sender, args| {
        let Some(args) = args else {
            return Ok(());
        };
        let audible = sender.as_ref().is_some_and(page_audible);
        on_received(&app, tab, &args, audible);
        Ok(())
    }));
    if unsafe { core.add_NotificationReceived(&handler, &mut token) }.is_err() {
        eprintln!("site notifications stay silent: NotificationReceived was not attached");
        return;
    }
    native_tabs()
        .lock()
        .expect("site notifications")
        .insert(tab);
}

/// A page script reported a notification. Ignored when `attach` succeeded, because the
/// WebView2 event is the one place that can play sound and answer the page.
pub fn from_script(app: &AppHandle, tab: u32, title: &str, body: &str, silent: bool, tag: &str) {
    if handles(tab) {
        return;
    }
    show(app, tab, title, body, tag, silent, false, None);
}

/// Stops every toast this tab still has open. Closing the tab does not raise
/// `CloseRequested`, and a call would keep ringing.
pub fn dismiss_tab(app: &AppHandle, tab: u32) {
    native_tabs()
        .lock()
        .expect("site notifications")
        .remove(&tab);
    let app = app.clone();
    let _ = app.run_on_main_thread(move || dismiss_tab_now(tab));
}

fn on_received(
    app: &AppHandle,
    tab: u32,
    args: &ICoreWebView2NotificationReceivedEventArgs,
    page_audible: bool,
) {
    let Ok(notification) = (unsafe { args.Notification() }) else {
        return;
    };
    let origin = crate::webview2::read_pwstr(|value| unsafe { args.SenderOrigin(value) });
    if crate::privacy::decision(app, &origin, "notifications") == Some(false) {
        unsafe {
            let _ = args.SetHandled(true);
            let _ = notification.ReportClosed();
        }
        return;
    }
    let title = crate::webview2::read_pwstr(|value| unsafe { notification.Title(value) });
    let body = crate::webview2::read_pwstr(|value| unsafe { notification.Body(value) });
    let tag = crate::webview2::read_pwstr(|value| unsafe { notification.Tag(value) });
    let silent = crate::webview2::read_bool(|value| unsafe { notification.IsSilent(value) });
    let title = if title.is_empty() {
        "Notification".to_string()
    } else {
        title
    };
    if show(
        app,
        tab,
        &title,
        &body,
        &tag,
        silent,
        page_audible,
        Some(notification),
    ) {
        let _ = unsafe { args.SetHandled(true) };
    }
}

fn show(
    app: &AppHandle,
    tab: u32,
    title: &str,
    body: &str,
    tag: &str,
    silent: bool,
    page_audible: bool,
    notification: Option<ICoreWebView2Notification>,
) -> bool {
    let incoming = notification_is_call(title, body, tag);
    let sound = notification_sound(silent, incoming, page_audible);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let open = note_open_arg(id);
    let dismiss = note_dismiss_arg(id);
    let Some(toast) = present(app, id, &open, title, body, sound, &dismiss) else {
        eprintln!("site notification was not shown: {title}");
        return false;
    };
    // Register the close handler before telling the page the toast is visible. The page
    // can close the notification inside ReportShown, and the note has to be in the map then.
    if let Some(notification) = &notification {
        watch_close(notification, id);
    }
    let stale = supersede(tab, tag, incoming);
    for note in &stale {
        hide_created(&note.toast);
    }
    let report = notification.clone();
    NOTES.with(|notes| {
        notes.borrow_mut().insert(
            id,
            Note {
                tab,
                tag: tag.chars().take(128).collect(),
                call: incoming,
                toast,
                notification,
            },
        );
    });
    if let Some(notification) = report {
        let _ = unsafe { notification.ReportShown() };
    }
    // The ring can start before anyone clicks the page. Release playback now so the
    // conversation is not still waiting on a gesture when the call is answered.
    if incoming {
        allow_playback(app, tab);
    }
    true
}

/// Shows `sound`, and a single chime when a call toast is rejected. The looping call
/// sound needs `scenario="incomingCall"`, which some Windows builds refuse.
fn present(
    app: &AppHandle,
    id: u64,
    open: &str,
    title: &str,
    body: &str,
    sound: ToastSound,
    dismiss: &str,
) -> Option<ToastNotification> {
    if let Some(toast) = show_one(app, id, open, title, body, sound, dismiss) {
        return Some(toast);
    }
    if sound == ToastSound::Call {
        return show_one(app, id, open, title, body, ToastSound::Default, "");
    }
    None
}

fn show_one(
    app: &AppHandle,
    id: u64,
    open: &str,
    title: &str,
    body: &str,
    sound: ToastSound,
    dismiss: &str,
) -> Option<ToastNotification> {
    let markup = toast_markup(open, title, body, sound, dismiss);
    let toast = create_toast(&markup)?;
    wire(&toast, app, id);
    show_created(&toast).then_some(toast)
}

fn wire(toast: &ToastNotification, app: &AppHandle, id: u64) {
    let opened = app.clone();
    let _ = toast.Activated(&TypedEventHandler::new(move |_, args| {
        let argument = toast_argument(args);
        clicked_toasts().lock().expect("toast click").insert(id);
        let later = opened.clone();
        let _ = opened.run_on_main_thread(move || on_activated(&later, id, &argument));
        Ok(())
    }));
    let closed = app.clone();
    let _ = toast.Dismissed(&TypedEventHandler::new(move |_, _| {
        let clicked = clicked_toasts().lock().expect("toast click").remove(&id);
        let _ = closed.run_on_main_thread(move || {
            if !clicked {
                on_dismissed(id);
            }
        });
        Ok(())
    }));
}

fn toast_argument(args: windows::core::Ref<'_, IInspectable>) -> String {
    let Some(value) = args.as_ref() else {
        return String::new();
    };
    value
        .cast::<ToastActivatedEventArgs>()
        .ok()
        .and_then(|args| args.Arguments().ok())
        .map(|text| text.to_string())
        .unwrap_or_default()
}

fn on_activated(app: &AppHandle, id: u64, argument: &str) {
    let Some(note) = take(id) else {
        return;
    };
    if matches!(note_action(argument), Some(NoteAction::Dismiss(_))) {
        report_closed(&note);
        return;
    }
    report_clicked(&note);
    reveal(app, note.tab);
    // The answer click is on the toast, not in the page. Release the autoplay guard so the
    // call's audio context, parked while the page waited for a gesture, can start.
    if let Some(webview) = crate::tabs::page_view(app, note.tab) {
        let _ = webview.eval("window.__ceAllowPlayback && window.__ceAllowPlayback()");
    }
}

fn on_dismissed(id: u64) {
    let Some(note) = take(id) else {
        return;
    };
    report_closed(&note);
}

fn on_page_closed(id: u64) {
    let Some(note) = take(id) else {
        return;
    };
    hide_created(&note.toast);
}

fn watch_close(notification: &ICoreWebView2Notification, id: u64) {
    let mut token = 0i64;
    let handler = NotificationCloseRequestedEventHandler::create(Box::new(move |_, _| {
        on_page_closed(id);
        Ok(())
    }));
    let _ = unsafe { notification.add_CloseRequested(&handler, &mut token) };
}

fn dismiss_tab_now(tab: u32) {
    let gone = NOTES.with(|notes| {
        let mut notes = notes.borrow_mut();
        let ids: Vec<u64> = notes
            .iter()
            .filter(|(_, note)| note.tab == tab)
            .map(|(&id, _)| id)
            .collect();
        ids.into_iter()
            .filter_map(|id| notes.remove(&id))
            .collect::<Vec<_>>()
    });
    for note in &gone {
        hide_created(&note.toast);
    }
}

fn supersede(tab: u32, tag: &str, call: bool) -> Vec<Note> {
    NOTES.with(|notes| {
        let mut notes = notes.borrow_mut();
        let ids: Vec<u64> = notes
            .iter()
            .filter(|(_, note)| {
                note.tab == tab && ((call && note.call) || (!tag.is_empty() && note.tag == tag))
            })
            .map(|(&id, _)| id)
            .collect();
        ids.into_iter().filter_map(|id| notes.remove(&id)).collect()
    })
}

fn take(id: u64) -> Option<Note> {
    NOTES.with(|notes| notes.borrow_mut().remove(&id))
}

fn report_clicked(note: &Note) {
    if let Some(notification) = &note.notification {
        let _ = unsafe { notification.ReportClicked() };
    }
}

fn report_closed(note: &Note) {
    if let Some(notification) = &note.notification {
        let _ = unsafe { notification.ReportClosed() };
    }
}

fn allow_playback(app: &AppHandle, tab: u32) {
    let Some(webview) = crate::tabs::page_view(app, tab) else {
        return;
    };
    let _ = webview.eval("window.__ceAllowPlayback && window.__ceAllowPlayback()");
}

fn reveal(app: &AppHandle, tab: u32) {
    if let Some(label) = crate::tabs::tab_window_label(app, tab)
        && let Some(window) = app.get_window(&label)
    {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = crate::tabs::activate(app, tab);
}

fn page_audible(core: &ICoreWebView2) -> bool {
    let Ok(core) = core.cast::<ICoreWebView2_8>() else {
        return false;
    };
    let playing = crate::webview2::read_bool(|value| unsafe { core.IsDocumentPlayingAudio(value) });
    let muted = crate::webview2::read_bool(|value| unsafe { core.IsMuted(value) });
    playing && !muted
}
