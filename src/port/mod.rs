//! Linux and macOS. The pet is written against a small part of the Windows API; here that part is
//! answered by something else, so that none of the pet itself has to know:
//!
//! - `gdi.rs` draws into memory, `edit.rs` is a text box,
//! - `win.rs` has the windows, their messages and timers,
//! - `peers.rs` lets two pets find and talk to each other,
//! - `os.rs` knows the clock, the load, the battery and the like,
//! - `x11.rs` and `mac.rs` put windows on the screen and say what the mouse and keyboard do.
//!
//! The names and types still come from `windows-sys`, which only declares them; the functions here
//! carry the same names, and the linker brings the two together.
#![allow(non_snake_case, clippy::missing_safety_doc)]

pub(crate) mod edit;
#[cfg(any(target_os = "macos", test))]
mod fine;
pub(crate) mod gdi;
mod os;
mod peers;
mod win;

#[cfg(target_os = "macos")]
#[path = "mac.rs"]
mod screen;
#[cfg(not(target_os = "macos"))]
#[path = "x11.rs"]
mod screen;

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::*;

/// What somebody did. Not every system has all of it.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum Ev {
    Mouse { win: u32, what: Mouse, x: i32, y: i32, at: (i32, i32) },
    Leave(u32),
    Key { vk: u16, down: bool, text: String },
    Hot(i32),
    Exposed(u32),
    Screens,
    Close(u32),
    Dropped(u32, Vec<String>),
    Tray(u32), // a mouse message over the icon
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Mouse {
    Move,
    Down(u8), // 1 left, 2 middle, 3 right
    Up(u8),
    Wheel(i32),
}

/// A connected monitor, in the pixels windows are placed in.
#[derive(Clone, Copy)]
pub(crate) struct Monitor {
    pub full: RECT,
    pub work: RECT,
    pub dpi: u32,
}

/// A window of some other program, as far as it matters.
#[derive(Clone)]
pub(crate) struct Foreign {
    pub id: u64,
    pub pid: u32,
    pub rect: RECT,
    pub title: String,
    pub class: String,
    pub zoomed: bool,
    pub full: bool,
}

/// How a window should be made.
#[allow(dead_code)]
pub(crate) struct Make {
    pub id: u32,
    pub rect: RECT,
    pub title: String,
    pub takes_focus: bool,
    pub top: bool,
    pub through: bool,
}

/// The text a UTF-16 pointer points at.
pub(crate) unsafe fn string(p: *const u16) -> String {
    if (p as usize) < 65536 {
        return String::new(); // nothing, or an atom
    }
    let n = (0..).take_while(|i| *p.add(*i) != 0).count();
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

/// Copy text into a buffer of `n`, with a zero at its end. How much went in.
pub(crate) unsafe fn put(text: &str, to: *mut u16, n: i32) -> i32 {
    if n <= 0 || to.is_null() {
        return 0;
    }
    let w: Vec<u16> = text.encode_utf16().take(n as usize - 1).collect();
    std::ptr::copy_nonoverlapping(w.as_ptr(), to, w.len());
    *to.add(w.len()) = 0;
    w.len() as i32
}

static SCALE: AtomicU32 = AtomicU32::new(100);

pub(crate) fn scale() -> f32 {
    SCALE.load(Relaxed) as f32 / 100.0
}

extern "C" {
    fn signal(which: i32, what: usize) -> usize;
}

/// Where the settings go. Set once, before anything reads it.
fn home() -> Option<std::path::PathBuf> {
    let dir = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    if cfg!(target_os = "macos") {
        Some(dir("HOME")?.join("Library/Application Support/Pixel Pet"))
    } else {
        Some(dir("XDG_CONFIG_HOME").or_else(|| Some(dir("HOME")?.join(".config")))?.join("pixel-pet"))
    }
}

/// Held while the program runs. When it goes, so does what others can see of us.
pub(crate) struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        win::end();
    }
}

/// Before anything else.
#[must_use]
pub(crate) fn start() -> Open {
    if std::env::var_os("APPDATA").is_none() {
        if let Some(d) = home() {
            let _ = std::fs::create_dir_all(&d);
            std::env::set_var("APPDATA", d);
        }
    }
    if std::env::var_os("USERPROFILE").is_none() {
        if let Some(h) = std::env::var_os("HOME") {
            std::env::set_var("USERPROFILE", h);
        }
    }
    unsafe {
        // programs it started may end without being asked about
        signal(if cfg!(target_os = "macos") { 20 } else { 17 }, 1);
    }
    match screen::Screen::open() {
        Ok(s) => {
            let dpi = s.monitors().first().map_or(96, |m| m.dpi);
            SCALE.store(dpi * 100 / 96, Relaxed);
            win::begin(s);
            Open
        }
        Err(e) => {
            eprintln!("Pixel Pet cannot open the screen: {e}");
            std::process::exit(1);
        }
    }
}
