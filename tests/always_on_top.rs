//! End-to-end checks on the real exe: the pet window must exist, be visible, and stay on top.
//! Each test starts its own pet with a private APPDATA, so the user's settings are untouched,
//! and PIXEL_PET_TEST=1 so the do-not-disturb logic can't depend on what is on screen.
#![cfg(windows)]

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

struct Pet {
    child: Child,
    dir: PathBuf,
}

impl Pet {
    fn start(tag: &str) -> Pet {
        let dir = std::env::temp_dir().join(format!("pixel-pet-it-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_pixel-pet"))
            .env("APPDATA", &dir)
            .env("PIXEL_PET_TEST", "1")
            .spawn()
            .expect("could not start pixel-pet");
        Pet { child, dir }
    }

    /// The pet's main window, once it exists.
    fn window(&self) -> HWND {
        let mut found = None;
        wait_for(10, || {
            found = find_window(self.child.id());
            found.is_some()
        });
        found.expect("the pet never created its window")
    }
}

impl Drop for Pet {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Search {
    pid: u32,
    found: Option<HWND>,
}

unsafe extern "system" fn visit(h: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp as *mut Search);
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, &mut pid);
    if pid == s.pid {
        let mut cls = [0u16; 32];
        let n = GetClassNameW(h, cls.as_mut_ptr(), 32).max(0) as usize;
        if String::from_utf16_lossy(&cls[..n]) == "PixelPetWnd" {
            s.found = Some(h);
            return 0;
        }
    }
    1
}

fn find_window(pid: u32) -> Option<HWND> {
    let mut s = Search { pid, found: None };
    unsafe { EnumWindows(Some(visit), &mut s as *mut _ as LPARAM) };
    s.found
}

fn is_topmost(h: HWND) -> bool {
    unsafe { GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0 }
}

fn is_visible(h: HWND) -> bool {
    unsafe { IsWindowVisible(h) != 0 }
}

/// Poll `cond` until it is true or `secs` pass. Returns the last result.
fn wait_for(secs: u64, mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn pet_window_is_visible_and_topmost_at_startup() {
    let pet = Pet::start("startup");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)), "pet window is not visible");
    assert!(is_topmost(w), "pet window lacks WS_EX_TOPMOST");
}

#[test]
fn pet_regains_topmost_after_being_pushed_down() {
    // Other topmost windows (the taskbar, flyouts) can sit above us; the pet must climb back.
    let pet = Pet::start("regain");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)), "pet window is not visible");
    unsafe {
        SetWindowPos(w, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
    assert!(!is_topmost(w), "test setup failed: window is still topmost");
    assert!(wait_for(4, || is_topmost(w)), "pet did not return to the topmost band within 4 s");
    assert!(is_visible(w), "pet window disappeared");
}

#[test]
fn pet_does_not_steal_focus() {
    let pet = Pet::start("focus");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1500));
    let fg = unsafe { GetForegroundWindow() };
    assert_ne!(fg, w, "pet window became the foreground window");
}
