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
    class: &'static str,
    found: Option<HWND>,
}

unsafe extern "system" fn visit(h: HWND, lp: LPARAM) -> BOOL {
    let s = &mut *(lp as *mut Search);
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, &mut pid);
    if pid == s.pid {
        let mut cls = [0u16; 32];
        let n = GetClassNameW(h, cls.as_mut_ptr(), 32).max(0) as usize;
        if String::from_utf16_lossy(&cls[..n]) == s.class {
            s.found = Some(h);
            return 0;
        }
    }
    1
}

fn find_window(pid: u32) -> Option<HWND> {
    find_class(pid, "PixelPetWnd")
}

fn find_class(pid: u32, class: &'static str) -> Option<HWND> {
    let mut s = Search { pid, class, found: None };
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
    // the pet's own heartbeat may put it back up between the push and the look, so try a few times
    let pushed = (0..5).any(|_| {
        unsafe {
            SetWindowPos(w, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        !is_topmost(w)
    });
    assert!(pushed, "test setup failed: window is still topmost");
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

fn window_size(w: HWND) -> (i32, i32) {
    let mut r = windows_sys::Win32::Foundation::RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(w, &mut r) };
    (r.right - r.left, r.bottom - r.top)
}

/// Left-click the pet in the bottom-right corner of its (default-placed) window.
fn click_pet(w: HWND) {
    let (width, height) = window_size(w);
    // default placement puts the pet in the bottom-right corner: ~44 px in from both edges
    let lp = (((height - 41) as isize) << 16) | ((width - 44) as isize & 0xFFFF);
    unsafe {
        PostMessageW(w, WM_LBUTTONDOWN, 1, lp);
        PostMessageW(w, WM_LBUTTONUP, 0, lp);
    }
}

#[test]
fn left_click_opens_and_closes_the_pixel_hub() {
    let pet = Pet::start("hub");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1200)); // first paint records where the pet is
    let (_, h0) = window_size(w);
    click_pet(w);
    assert!(wait_for(3, || window_size(w).1 > h0), "left click did not open the hub (window did not grow)");
    assert!(is_topmost(w) && is_visible(w));
    click_pet(w);
    assert!(wait_for(3, || window_size(w).1 == h0), "second click did not close the hub");
}

#[test]
fn right_click_opens_the_same_pixel_hub_not_a_native_menu() {
    let pet = Pet::start("rclick");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    std::thread::sleep(Duration::from_millis(1200));
    let (width, height) = window_size(w);
    let lp = (((height - 41) as isize) << 16) | ((width - 44) as isize & 0xFFFF);
    let (_, h0) = window_size(w);
    unsafe { PostMessageW(w, WM_RBUTTONUP, 0, lp) };
    // a native popup menu would block the pet's message loop and never resize the window
    assert!(wait_for(3, || window_size(w).1 > h0), "right click did not open the pixel hub");
}

#[test]
fn stats_window_opens_as_its_own_pixel_window_and_closes() {
    let pet = Pet::start("stats");
    let w = pet.window();
    assert!(wait_for(5, || is_visible(w)));
    let pid = pet.child.id();
    assert!(find_class(pid, "PixelStatsWnd").is_none(), "stats window should not exist before it is asked for");
    unsafe { PostMessageW(w, WM_HOTKEY, 3, 0) }; // what Ctrl+Alt+S sends
    let mut sw = None;
    assert!(
        wait_for(4, || {
            sw = find_class(pid, "PixelStatsWnd").filter(|h| is_visible(*h));
            sw.is_some()
        }),
        "stats window did not appear"
    );
    let sw = sw.unwrap();
    let style = unsafe { GetWindowLongW(sw, GWL_STYLE) as u32 };
    assert_eq!(style & WS_CAPTION, 0, "stats window must be borderless (pixel-drawn), not a native frame");
    unsafe { PostMessageW(sw, WM_CLOSE, 0, 0) };
    assert!(wait_for(3, || !is_visible(sw)), "stats window did not close");
}
