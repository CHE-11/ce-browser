//! Copies stdout and stderr into the debug log. Everything still reaches the terminal.
//!
//! Rust's std looks the standard handles up on every write, so pointing them at a pipe catches
//! every `println!` and `eprintln!` without touching the call sites. The reader thread must
//! keep running: once the read end closes, the next `eprintln!` panics.

use std::fs::File;
use std::io::{PipeReader, Read, Write};
use std::mem::ManuallyDrop;
use std::os::windows::io::{FromRawHandle, IntoRawHandle, RawHandle};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
};

/// A line this long is logged even without its newline.
const MAX_LINE: usize = 16 * 1024;
/// Longer prefixes are part of the message, not an area name.
const MAX_AREA: usize = 40;

/// Call before anything prints. Does nothing in a release build.
pub fn capture() {
    if !super::enabled() {
        return;
    }
    tee(STD_OUTPUT_HANDLE, "stdout");
    tee(STD_ERROR_HANDLE, "stderr");
}

fn tee(which: STD_HANDLE, stream: &'static str) {
    let original = unsafe { GetStdHandle(which) }
        .ok()
        .filter(|handle| !handle.is_invalid() && !handle.0.is_null())
        .map(|handle| handle.0 as isize);
    let Ok((reader, writer)) = std::io::pipe() else {
        return;
    };
    // The write end stays open for the life of the process; std writes to it from now on.
    let raw = writer.into_raw_handle();
    if unsafe { SetStdHandle(which, HANDLE(raw)) }.is_err() {
        drop(unsafe { File::from_raw_handle(raw) });
        return;
    }
    let spawned = std::thread::Builder::new()
        .name(format!("debug-{stream}"))
        .spawn(move || pump(reader, original, stream));
    if spawned.is_err()
        && let Some(original) = original
    {
        let _ = unsafe { SetStdHandle(which, HANDLE(original as RawHandle)) };
    }
}

fn pump(mut reader: PipeReader, original: Option<isize>, stream: &'static str) {
    // Borrowed: the process still owns the original handle.
    let mut terminal = original
        .map(|handle| ManuallyDrop::new(unsafe { File::from_raw_handle(handle as RawHandle) }));
    let mut buffer = [0u8; 4096];
    let mut pending = Vec::new();
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        if let Some(terminal) = terminal.as_mut() {
            let _ = terminal.write_all(&buffer[..read]);
        }
        pending.extend_from_slice(&buffer[..read]);
        for line in take_lines(&mut pending) {
            super::push_log(stream, &line);
        }
    }
}

/// Complete lines from `pending`, without their line ending. A partial line stays behind
/// unless it has grown past `MAX_LINE`.
fn take_lines(pending: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
        let rest = pending.split_off(end + 1);
        let line = std::mem::replace(pending, rest);
        lines.push(text_of(&line[..end]));
    }
    if pending.len() > MAX_LINE {
        lines.push(text_of(pending));
        pending.clear();
    }
    lines
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches(['\r', '\n'])
        .to_string()
}

/// `docked devtools` in `docked devtools: the port closed`. Empty when there is no short prefix.
pub fn area_of(text: &str) -> &str {
    let Some((area, _)) = text.split_once(": ") else {
        return "";
    };
    let area = area.trim();
    let plain = !area.is_empty()
        && area.len() <= MAX_AREA
        && !area.contains(['/', '\\', '\'', '"', '(', '[', '{']);
    if plain { area } else { "" }
}

#[cfg(test)]
mod tests {
    use super::{area_of, take_lines};

    #[test]
    fn complete_lines_are_split_and_the_rest_waits() {
        let mut pending = b"one\r\ntwo\nthr".to_vec();
        assert_eq!(take_lines(&mut pending), vec!["one", "two"]);
        assert_eq!(pending, b"thr");
        pending.extend_from_slice(b"ee\n");
        assert_eq!(take_lines(&mut pending), vec!["three"]);
        assert!(pending.is_empty());
    }

    #[test]
    fn a_very_long_partial_line_is_logged() {
        let mut pending = vec![b'x'; super::MAX_LINE + 1];
        let lines = take_lines(&mut pending);
        assert_eq!(lines.len(), 1);
        assert!(pending.is_empty());
    }

    #[test]
    fn invalid_utf8_is_kept_as_replacement_characters() {
        let mut pending = vec![b'a', 0xFF, b'\n'];
        assert_eq!(take_lines(&mut pending), vec!["a\u{FFFD}"]);
    }

    #[test]
    fn the_area_is_the_short_prefix_before_a_colon() {
        assert_eq!(
            area_of("docked devtools: the port closed"),
            "docked devtools"
        );
        assert_eq!(area_of("shortcut DebugHud: failed"), "shortcut DebugHud");
        assert_eq!(area_of("no area here"), "");
        assert_eq!(
            area_of("thread 'main' panicked at src\\lib.rs:1:1: boom"),
            ""
        );
        assert_eq!(area_of(": empty"), "");
        assert_eq!(
            area_of("a very long prefix that is clearly a sentence and not an area: x"),
            ""
        );
    }
}
