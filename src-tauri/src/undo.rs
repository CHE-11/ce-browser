use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use url::form_urlencoded;

use crate::{bookmarks, history, tabs};

/// An offer whose toast never reported back is dropped after this long.
const KEEP: Duration = Duration::from_secs(120);

pub enum UndoAction {
    /// A message only. The toast has no Undo button.
    Nothing,
    /// A recently closed entry. Undo opens it again.
    Closed(u32),
    /// Deleted bookmarks with their folder and position, in the order they were deleted.
    Bookmarks(Vec<(bookmarks::BookmarkNode, String, usize)>),
    History {
        items: Vec<history::HistoryItem>,
        cleared: bool,
    },
}

struct Offer {
    id: u32,
    window: String,
    action: UndoAction,
    at: Instant,
}

static OFFERS: Mutex<Vec<Offer>> = Mutex::new(Vec::new());
static NEXT: AtomicU32 = AtomicU32::new(0);

/// Shows a toast with Undo in `window`. A newer toast in the same window ends the older offer.
pub fn offer(app: &AppHandle, window: &str, message: &str, action: UndoAction) -> u32 {
    let id = NEXT.fetch_add(1, Ordering::Relaxed) + 1;
    let undoable = !matches!(action, UndoAction::Nothing);
    let ended = {
        let mut offers = OFFERS.lock().expect("undo");
        let (ended, kept): (Vec<Offer>, Vec<Offer>) = offers
            .drain(..)
            .partition(|offer| offer.window == window || offer.at.elapsed() > KEEP);
        *offers = kept;
        offers.push(Offer {
            id,
            window: window.to_string(),
            action,
            at: Instant::now(),
        });
        ended
    };
    for offer in ended {
        finish(app, offer.action);
    }
    let params = form_urlencoded::Serializer::new(String::new())
        .append_pair("id", &id.to_string())
        .append_pair("message", message)
        .append_pair("undo", if undoable { "1" } else { "0" })
        .finish();
    if let Err(error) = tabs::show_toast(app, window, &params) {
        eprintln!("toast: {error}");
    }
    id
}

/// A toast with a message and no Undo.
pub fn notice(app: &AppHandle, window: &str, message: &str) {
    offer(app, window, message, UndoAction::Nothing);
}

/// Toast for tabs closed together. A single closed tab does not get one.
pub fn offer_closed(app: &AppHandle, window: &str, entry: Option<u32>) {
    let Some(entry) = entry else {
        return;
    };
    let Some(message) = tabs::closed_summary(app, entry) else {
        return;
    };
    offer(app, window, &message, UndoAction::Closed(entry));
}

fn take(id: u32) -> Option<Offer> {
    let mut offers = OFFERS.lock().expect("undo");
    let index = offers.iter().position(|offer| offer.id == id)?;
    Some(offers.remove(index))
}

/// Reverses the action behind toast `id`.
pub fn undo(app: &AppHandle, id: u32) -> Result<(), String> {
    let offer = take(id).ok_or("That can no longer be undone.")?;
    tabs::close_toast(app, &offer.window);
    match offer.action {
        UndoAction::Nothing => Ok(()),
        UndoAction::Closed(entry) => tabs::restore_closed(app, Some(entry)).map(|_| ()),
        UndoAction::Bookmarks(removed) => {
            for (node, parent, index) in removed.into_iter().rev() {
                bookmarks::restore(app, node, &parent, index)?;
            }
            Ok(())
        }
        UndoAction::History { items, .. } => {
            history::restore(app, items);
            Ok(())
        }
    }
}

/// The toast closed without Undo. The action is now final.
pub fn dismiss(app: &AppHandle, id: u32) {
    if let Some(offer) = take(id) {
        tabs::close_toast(app, &offer.window);
        finish(app, offer.action);
    }
}

/// The window is closing: its offers become final.
pub fn close_window(app: &AppHandle, window: &str) {
    let ended: Vec<Offer> = {
        let mut offers = OFFERS.lock().expect("undo");
        let (ended, kept) = offers.drain(..).partition(|offer| offer.window == window);
        *offers = kept;
        ended
    };
    for offer in ended {
        finish(app, offer.action);
    }
    tabs::close_toast(app, window);
}

/// Runs the parts of an action that wait for the undo window to pass.
fn finish(app: &AppHandle, action: UndoAction) {
    if let UndoAction::History { items, cleared } = action {
        history::forget_text(app, &items, cleared);
    }
}

/// Every pending offer becomes final, for example before the app exits.
pub fn finish_all(app: &AppHandle) {
    let ended: Vec<Offer> = OFFERS.lock().expect("undo").drain(..).collect();
    for offer in ended {
        finish(app, offer.action);
    }
}
