//! Keeps `window.alert`, `window.confirm`, and `window.prompt` on the webview.
//!
//! The dialog plugin replaces `alert` and `confirm` with `plugin:dialog|message`.
//! A page in a tab matches no capability, so that call is rejected and nothing is shown.
//! This script runs first and leaves the browser dialog in place. File dialogs still
//! go through the plugin from the browser UI.

/// Runs before `tauri_plugin_dialog`, which assigns over `alert` and `confirm`.
const KEEP_PAGE_DIALOGS: &str = r#"
var names = ["alert", "confirm", "prompt"];
for (var index = 0; index < names.length; index++) {
  (function (name) {
    var current = window[name];
    try {
      Object.defineProperty(window, name, {
        configurable: true,
        enumerable: true,
        get: function () {
          return current;
        },
        set: function (next) {
          var source = "";
          try {
            source = Function.prototype.toString.call(next);
          } catch (error) {}
          if (typeof next === "function" && source.indexOf("plugin:dialog|") !== -1) {
            return;
          }
          current = next;
        }
      });
    } catch (error) {}
  })(names[index]);
}
"#;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("page-dialogs")
        .js_init_script(KEEP_PAGE_DIALOGS)
        .build()
}
