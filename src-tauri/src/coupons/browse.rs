use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};
use url::Url;

use crate::{tabs, webview2};

static NEXT_READER: AtomicU32 = AtomicU32::new(1);

const SETTLE: Duration = Duration::from_millis(1200);
const POLL: Duration = Duration::from_millis(800);

/// Cloudflare and similar checks pass on their own in a real browser after a few seconds.
/// A captcha that needs a click stays and the page counts as blocked.
const SNAPSHOT: &str = r##"(function () {
  var nav = performance.getEntriesByType("navigation")[0];
  var title = document.title || "";
  var text = document.body ? document.body.innerText.length : 0;
  var check = /just a moment|attention required|verify you are human|checking your browser|security check|are you a robot/i.test(title)
    || (text < 3000 && !!document.querySelector("#challenge-form, #challenge-running, #cf-challenge-running, iframe[src*='challenges.cloudflare.com'], .anomaly-modal__modal, #px-captcha"));
  var html = document.documentElement ? document.documentElement.outerHTML : "";
  return {
    url: location.href,
    status: (nav && nav.responseStatus) || 0,
    ready: document.readyState === "complete",
    challenge: check,
    html: html.length > 3000000 ? html.slice(0, 3000000) : html
  };
})()"##;

#[derive(Deserialize)]
struct Snapshot {
    url: String,
    status: u16,
    ready: bool,
    challenge: bool,
    html: String,
}

pub struct Page {
    /// Address after redirects.
    pub url: String,
    pub html: String,
}

/// A webview parked off the window that loads pages with the browser's own profile, so search
/// engines and coupon sites see a normal browser with its cookies instead of a bare HTTP client.
pub struct Reader {
    webview: Webview,
    loads: Mutex<Receiver<String>>,
}

impl Reader {
    pub fn open(app: &AppHandle) -> Result<Self, String> {
        let window = app
            .get_window(&tabs::focused_window(app))
            .or_else(|| app.get_window(tabs::WINDOW))
            .ok_or("The browser window is missing.")?;
        let label = format!("coupon-reader-{}", NEXT_READER.fetch_add(1, Ordering::Relaxed));
        let blank = Url::parse("about:blank").map_err(|error| error.to_string())?;
        let (tx, loads) = mpsc::channel::<String>();
        let builder = tabs::base_builder(app, &label, WebviewUrl::External(blank))
            .focused(false)
            .on_page_load(move |_, payload| {
                if matches!(payload.event(), PageLoadEvent::Finished) {
                    let _ = tx.send(payload.url().to_string());
                }
            });
        let webview = window
            .add_child(
                builder,
                LogicalPosition::new(-20000.0, -20000.0),
                LogicalSize::new(1280.0, 1400.0),
            )
            .map_err(|error| format!("Couldn't open the coupon reader. {error}"))?;
        Ok(Self {
            webview,
            loads: Mutex::new(loads),
        })
    }

    /// Loads `address` and returns its HTML once the page has settled.
    pub fn read(&self, address: &str, timeout: Duration) -> Result<Page, String> {
        let target = Url::parse(address).map_err(|_| "That address isn't valid.".to_string())?;
        let loads = self.loads.lock().map_err(|_| "The coupon reader is busy.".to_string())?;
        while loads.try_recv().is_ok() {}
        self.webview
            .navigate(target)
            .map_err(|error| format!("Couldn't open the page. {error}"))?;
        let deadline = Instant::now() + timeout;
        loop {
            match loads.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(url) if url.starts_with("http") => break,
                Ok(_) => {}
                Err(_) => return Err("The page took too long to load.".into()),
            }
        }
        let mut settled = false;
        loop {
            let snapshot = webview2::evaluate(&self.webview, SNAPSHOT, Duration::from_secs(8), "")
                .ok()
                .and_then(|value| serde_json::from_value::<Snapshot>(value).ok());
            if let Some(snapshot) = snapshot {
                if snapshot.status >= 400 && !snapshot.challenge {
                    return Err(format!("The page answered {}.", snapshot.status));
                }
                if snapshot.ready && !snapshot.challenge {
                    if settled {
                        return Ok(Page {
                            url: snapshot.url,
                            html: snapshot.html,
                        });
                    }
                    settled = true;
                    std::thread::sleep(SETTLE);
                    continue;
                }
            }
            if Instant::now() >= deadline {
                return Err("The page asked for a check.".into());
            }
            std::thread::sleep(POLL);
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.webview.close();
    }
}
