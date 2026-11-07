//! JavaScript `alert`, `confirm`, `prompt` and before-unload dialogs.
//!
//! While a flow or the agent is driving a tab, the native dialog is deferred so the page
//! does not freeze. The run answers it, or the agent panel does.

use std::sync::{Mutex, OnceLock, mpsc};
use std::time::Duration;

use tauri::AppHandle;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_SCRIPT_DIALOG_KIND, COREWEBVIEW2_SCRIPT_DIALOG_KIND_BEFOREUNLOAD,
    COREWEBVIEW2_SCRIPT_DIALOG_KIND_CONFIRM, COREWEBVIEW2_SCRIPT_DIALOG_KIND_PROMPT,
    ICoreWebView2ScriptDialogOpeningEventArgs,
};
use windows::core::{HSTRING, Interface};

use crate::webview2::{self, MainThread};

#[derive(Clone)]
pub struct Pending {
    pub tab: u32,
    pub kind: String,
    pub message: String,
    pub default_text: String,
}

struct Held {
    info: Pending,
    args: MainThread<ICoreWebView2ScriptDialogOpeningEventArgs>,
    deferral: MainThread<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Deferral>,
}

fn held() -> &'static Mutex<Option<Held>> {
    static HELD: OnceLock<Mutex<Option<Held>>> = OnceLock::new();
    HELD.get_or_init(|| Mutex::new(None))
}

pub fn current() -> Option<Pending> {
    held()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|item| item.info.clone()))
}

fn kind_name(kind: COREWEBVIEW2_SCRIPT_DIALOG_KIND) -> &'static str {
    if kind == COREWEBVIEW2_SCRIPT_DIALOG_KIND_CONFIRM {
        "confirm"
    } else if kind == COREWEBVIEW2_SCRIPT_DIALOG_KIND_PROMPT {
        "prompt"
    } else if kind == COREWEBVIEW2_SCRIPT_DIALOG_KIND_BEFOREUNLOAD {
        "beforeunload"
    } else {
        "alert"
    }
}

/// Called on the WebView2 thread. Defers the dialog when automation is driving the tab.
pub fn handle(app: &AppHandle, tab: u32, args: ICoreWebView2ScriptDialogOpeningEventArgs) {
    if !crate::automation::driving(tab) {
        return;
    }
    let kind = {
        let mut kind = COREWEBVIEW2_SCRIPT_DIALOG_KIND::default();
        let _ = unsafe { args.Kind(&mut kind) };
        kind_name(kind).to_string()
    };
    let message = webview2::read_pwstr(|value| unsafe { args.Message(value) });
    let default_text = webview2::read_pwstr(|value| unsafe { args.DefaultText(value) });
    let Ok(deferral) = (unsafe { args.GetDeferral() }) else {
        return;
    };
    let info = Pending {
        tab,
        kind,
        message,
        default_text,
    };
    if let Some(previous) = held().lock().expect("dialogs").replace(Held {
        info: info.clone(),
        args: MainThread::new(args),
        deferral: MainThread::new(deferral),
    }) {
        let previous_tab = previous.info.tab;
        complete(previous, false, "");
        crate::automation::agent::clear_dialog(app, previous_tab);
    }
    crate::automation::agent::note_dialog(app, tab, &info.kind, &info.message, &info.default_text);
}

/// Accepts or dismisses the dialog open on `tab`.
pub fn answer(app: &AppHandle, tab: u32, accept: bool, text: &str) -> Result<(), String> {
    let pending = {
        let mut guard = held().lock().map_err(|_| "Dialog lock.".to_string())?;
        let pending = guard.take().ok_or("No dialog is open.")?;
        if pending.info.tab != tab {
            let owner = pending.info.tab;
            *guard = Some(pending);
            return Err(format!("The open dialog belongs to tab {owner}."));
        }
        pending
    };
    finish(app, pending, accept, text)
}

fn finish(app: &AppHandle, pending: Held, accept: bool, text: &str) -> Result<(), String> {
    let tab = pending.info.tab;
    let (tx, rx) = mpsc::channel();
    let text = text.to_string();
    app.run_on_main_thread(move || {
        complete(pending, accept, &text);
        let _ = tx.send(());
    })
    .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(2))
        .map_err(|_| "The dialog did not close.".to_string())?;
    crate::automation::agent::clear_dialog(app, tab);
    Ok(())
}

/// `pending` was created on the WebView2 thread, so this runs there too.
fn complete(pending: Held, accept: bool, text: &str) {
    let args = pending.args.into_inner();
    let deferral = pending.deferral.into_inner();
    unsafe {
        if !text.is_empty() {
            let _ = args.SetResultText(&HSTRING::from(text));
        }
        set_accept(&args, accept);
        let _ = deferral.Complete();
    }
}

/// The Win32 binding's `Accept` method drops the BOOL argument. The real COM slot still takes one.
unsafe fn set_accept(args: &ICoreWebView2ScriptDialogOpeningEventArgs, accept: bool) {
    unsafe {
        let raw = Interface::as_raw(args);
        let vtable = *(raw as *const *const usize);
        let slot = *vtable.add(6);
        let func: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            windows::core::BOOL,
        ) -> windows::core::HRESULT = std::mem::transmute(slot);
        let _ = func(raw, windows::core::BOOL(i32::from(accept)));
    }
}
