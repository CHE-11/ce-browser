use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::{tabs, webview2};

const FAILURE: &str = "Couldn't search this page.";
const TIMEOUT: Duration = Duration::from_secs(3);

/// Text matches on one page. `index` is the highlighted match, or -1.
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FindResult {
    pub count: u32,
    pub index: i32,
}

/// Matches in one tab for "Find in all tabs".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindTabResult {
    pub id: u32,
    pub title: String,
    pub url: String,
    /// `None` when the tab was not searched.
    pub count: Option<u32>,
    /// Why the tab was not searched, such as `Not loaded`.
    pub skipped: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    case_sensitive: bool,
    whole_word: bool,
    backwards: bool,
    /// Move to the next or previous match. Typing searches again from the top of the view.
    step: bool,
    /// Count only. The page is not highlighted or scrolled.
    count: bool,
    clear: bool,
}

/// Finds text nodes, highlights matches with the CSS Custom Highlight API and keeps the current
/// match between calls, so Enter moves through them.
const FINDER_JS: &str = r#"(function (query, options) {
  var state = window.__ceFind;
  if (!state) {
    state = window.__ceFind = { ranges: [], index: -1, key: "", dirty: true, mark: null };
    new MutationObserver(function () { state.dirty = true; }).observe(document.documentElement, { subtree: true, childList: true, characterData: true });
    var style = document.createElement("style");
    style.textContent = "::highlight(ce-find){background-color:rgba(250,204,21,.45);color:inherit}::highlight(ce-find-current){background-color:#f97316;color:#111}";
    (document.head || document.documentElement).appendChild(style);
  }
  var canHighlight = !!(window.CSS && CSS.highlights && window.Highlight);
  function unhighlight() {
    if (canHighlight) { CSS.highlights.delete("ce-find"); CSS.highlights.delete("ce-find-current"); }
  }
  if (options.clear) {
    unhighlight();
    state.ranges = []; state.index = -1; state.key = ""; state.mark = null;
    return { count: 0, index: -1 };
  }
  var word = /[\p{L}\p{N}_]/u;
  function collect() {
    var out = [];
    if (!query) return out;
    var needle = options.caseSensitive ? query : query.toLowerCase();
    var seen = new WeakMap();
    var visible = function (element) {
      if (seen.has(element)) return seen.get(element);
      var shown = element.checkVisibility ? element.checkVisibility() : true;
      seen.set(element, shown);
      return shown;
    };
    var walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT, {
      acceptNode: function (node) {
        var parent = node.parentElement;
        if (!parent) return NodeFilter.FILTER_REJECT;
        var tag = parent.tagName;
        if (tag === "SCRIPT" || tag === "STYLE" || tag === "NOSCRIPT" || tag === "TEMPLATE") return NodeFilter.FILTER_REJECT;
        return visible(parent) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      }
    });
    var node;
    while ((node = walker.nextNode()) && out.length < 2000) {
      var text = node.nodeValue || "";
      var hay = options.caseSensitive ? text : text.toLowerCase();
      var at = hay.indexOf(needle);
      while (at !== -1 && out.length < 2000) {
        var before = at > 0 ? text.charAt(at - 1) : "";
        var after = text.charAt(at + needle.length);
        if (!options.wholeWord || ((!before || !word.test(before)) && (!after || !word.test(after)))) {
          var range = document.createRange();
          range.setStart(node, at);
          range.setEnd(node, at + needle.length);
          out.push(range);
        }
        at = hay.indexOf(needle, at + Math.max(1, needle.length));
      }
    }
    return out;
  }
  if (options.count) return { count: collect().length, index: -1 };
  var key = JSON.stringify([query, !!options.caseSensitive, !!options.wholeWord]);
  var sameQuery = key === state.key;
  if (!sameQuery || state.dirty) {
    state.ranges = collect();
    state.dirty = false;
    state.key = key;
    var kept = -1;
    if (sameQuery && state.mark) {
      for (var k = 0; k < state.ranges.length; k++) {
        if (state.ranges[k].startContainer === state.mark.node && state.ranges[k].startOffset === state.mark.offset) { kept = k; break; }
      }
    }
    state.index = kept;
  }
  var count = state.ranges.length;
  if (count === 0) {
    unhighlight();
    state.index = -1; state.mark = null;
    return { count: 0, index: -1 };
  }
  if (state.index < 0 || !sameQuery) {
    state.index = 0;
    for (var i = 0; i < count; i++) {
      if (state.ranges[i].getBoundingClientRect().bottom >= 0) { state.index = i; break; }
    }
    if (options.step && options.backwards) state.index = (state.index - 1 + count) % count;
  } else if (options.step) {
    state.index = (state.index + (options.backwards ? -1 : 1) + count) % count;
  }
  var current = state.ranges[state.index];
  state.mark = { node: current.startContainer, offset: current.startOffset };
  if (canHighlight) {
    CSS.highlights.set("ce-find", new Highlight(...state.ranges));
    CSS.highlights.set("ce-find-current", new Highlight(current));
  } else {
    var selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(current);
  }
  var target = current.startContainer.parentElement;
  if (target) target.scrollIntoView({ block: "center", inline: "nearest" });
  return { count: count, index: state.index };
})"#;

fn run(app: &AppHandle, id: u32, query: &str, options: &Options) -> Result<FindResult, String> {
    let webview = tabs::page_view(app, id).ok_or("That tab has no page loaded.")?;
    let query = serde_json::to_string(query).map_err(|error| error.to_string())?;
    let options = serde_json::to_string(options).map_err(|error| error.to_string())?;
    let value = webview2::evaluate(
        &webview,
        &format!("{FINDER_JS}({query}, {options})"),
        TIMEOUT,
        FAILURE,
    )?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

pub fn find(
    app: &AppHandle,
    id: u32,
    query: &str,
    backwards: bool,
    case_sensitive: bool,
    whole_word: bool,
    step: bool,
) -> Result<FindResult, String> {
    let options = Options {
        case_sensitive,
        whole_word,
        backwards,
        step,
        count: false,
        clear: query.is_empty(),
    };
    run(app, id, query, &options)
}

pub fn clear(app: &AppHandle, id: u32) {
    let options = Options {
        case_sensitive: false,
        whole_word: false,
        backwards: false,
        step: false,
        count: false,
        clear: true,
    };
    let _ = run(app, id, "", &options);
}

/// Counts matches in every tab of the window's current workspace. Tabs that have not loaded or
/// are asleep are listed as skipped, so searching does not wake them.
pub fn find_in_tabs(
    app: &AppHandle,
    window: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<FindTabResult> {
    let options = Options {
        case_sensitive,
        whole_word,
        backwards: false,
        step: false,
        count: true,
        clear: false,
    };
    tabs::strip_tabs(app, window)
        .into_iter()
        .map(|tab| {
            let skipped = if !tab.loaded {
                "Not loaded"
            } else if tab.sleeping {
                "Asleep"
            } else if tab.failed {
                "Didn't load"
            } else {
                ""
            };
            let count = if skipped.is_empty() {
                run(app, tab.id, query, &options)
                    .ok()
                    .map(|result| result.count)
            } else {
                None
            };
            FindTabResult {
                id: tab.id,
                title: tab.title,
                url: tab.url,
                skipped: if skipped.is_empty() && count.is_none() {
                    "Couldn't search".into()
                } else {
                    skipped.into()
                },
                count,
            }
        })
        .collect()
}
