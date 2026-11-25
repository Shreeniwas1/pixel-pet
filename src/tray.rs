//! The notification-area icon: hover for live numbers, left click hides or shows the pet, right
//! click opens the hub. It costs nothing while idle because the tip is only built on hover.

use crate::prelude::*;
use std::sync::atomic::AtomicU32;
use windows_sys::Win32::UI::Shell::*;

/// What the icon sends to the pet window.
pub(crate) const WM_TRAY: u32 = WM_APP + 1;
const TIP_MAX: usize = 127; // plus the terminating NUL

/// Sent to every window when Explorer restarts; the icon has to be added again.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static LAST_TIP: Mutex<String> = Mutex::new(String::new());

unsafe fn data(hwnd: HWND, tip: &str) -> NOTIFYICONDATAW {
    let mut d: NOTIFYICONDATAW = zeroed();
    d.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    d.hWnd = hwnd;
    d.uID = 1;
    d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    d.uCallbackMessage = WM_TRAY;
    // icon 1 from assets/pixel-pet.rc; a build without a resource compiler has none
    d.hIcon = LoadIconW(GetModuleHandleW(null()), 1 as *const u16);
    if d.hIcon.is_null() {
        d.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
    }
    for (to, from) in d.szTip.iter_mut().zip(tip.encode_utf16().take(TIP_MAX)) {
        *to = from;
    }
    d
}

pub(crate) unsafe fn add(hwnd: HWND) {
    if std::env::var_os("PIXEL_PET_TEST").is_some() {
        return; // test pets get killed, which would leave dead icons behind
    }
    if TASKBAR_CREATED.load(Relaxed) == 0 {
        TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Relaxed);
    }
    Shell_NotifyIconW(NIM_ADD, &data(hwnd, "Pixel Pet"));
}

pub(crate) unsafe fn remove(hwnd: HWND) {
    Shell_NotifyIconW(NIM_DELETE, &data(hwnd, ""));
}

pub(crate) fn is_taskbar_created(msg: u32) -> bool {
    msg != 0 && msg == TASKBAR_CREATED.load(Relaxed)
}

pub(crate) unsafe fn on_message(hwnd: HWND, lp: LPARAM) {
    match lp as u32 & 0xFFFF {
        WM_LBUTTONUP => toggle_hidden(hwnd),
        WM_RBUTTONUP => toggle_hub(hwnd),
        WM_MOUSEMOVE => {
            let tip = tip_now();
            let mut last = LAST_TIP.lock().unwrap();
            if *last != tip {
                Shell_NotifyIconW(NIM_MODIFY, &data(hwnd, &tip));
                *last = tip;
            }
        }
        _ => {}
    }
}

fn tip_now() -> String {
    let s = ST.lock().unwrap();
    let open = s.todos.iter().filter(|t| !t.done).count();
    let timer = s.focus.then(|| (s.pomo_phase, s.pomo_running, pomo_remaining(&s)));
    tip_text(&format_rate(s.speed, s.bits), &format_rate(s.up, s.bits), s.cpu, s.ram, open, timer)
}

/// `timer` is (phase, running, seconds left) while focus mode is on.
fn tip_text(down: &str, up: &str, cpu: u32, ram: u32, open: usize, timer: Option<(u8, bool, u64)>) -> String {
    let mut t = format!("Down {down}  Up {up}\nCPU {cpu}%  RAM {ram}%");
    if open > 0 {
        t.push_str(&format!("\n{open} open task{}", if open == 1 { "" } else { "s" }));
    }
    if let Some((phase, running, left)) = timer {
        let name = if phase == 0 { "Focus" } else { "Break" };
        t.push_str(&format!("\n{name} {:02}:{:02}{}", left / 60, left % 60, if running { "" } else { " (paused)" }));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tip_shows_only_what_there_is() {
        assert_eq!(tip_text("0 B/s", "0 B/s", 3, 40, 0, None), "Down 0 B/s  Up 0 B/s\nCPU 3%  RAM 40%");
        let t = tip_text("0 B/s", "0 B/s", 3, 40, 1, Some((0, false, 1500)));
        assert!(t.ends_with("\n1 open task\nFocus 25:00 (paused)"), "{t}");
        assert!(tip_text("a", "b", 3, 40, 2, Some((1, true, 65))).ends_with("\n2 open tasks\nBreak 01:05"));
    }

    #[test]
    fn the_longest_tip_fits_the_windows_limit() {
        let t = tip_text("99.9 Mbps", "99.9 Mbps", 100, 100, 9999, Some((1, false, 99 * 60)));
        assert!(t.encode_utf16().count() <= TIP_MAX, "{}", t.len());
    }
}
