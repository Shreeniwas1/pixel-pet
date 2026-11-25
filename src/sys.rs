//! Talking to Windows: clock, network, CPU, memory, battery, idle time, fullscreen detection.

use crate::prelude::*;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};

/// Overall CPU load in percent since the previous call, or None on the first call.
pub(crate) unsafe fn cpu_percent(prev: &mut (u64, u64)) -> Option<u32> {
    let (mut i, mut k, mut u): (FILETIME, FILETIME, FILETIME) = zeroed();
    if GetSystemTimes(&mut i, &mut k, &mut u) == 0 {
        return None;
    }
    let q = |f: FILETIME| (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64;
    let (idle, total) = (q(i), q(k) + q(u)); // kernel time includes idle time
    let (di, dt) = (idle.wrapping_sub(prev.0), total.wrapping_sub(prev.1));
    let first = prev.1 == 0;
    *prev = (idle, total);
    if first || dt == 0 {
        return None;
    }
    Some((100 - (di * 100 / dt).min(100)) as u32)
}

pub(crate) unsafe fn ram_percent() -> u32 {
    let mut m: MEMORYSTATUSEX = zeroed();
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    if GlobalMemoryStatusEx(&mut m) == 0 {
        return 0;
    }
    m.dwMemoryLoad
}

/// (percent, plugged in) or None on a machine without a battery.
pub(crate) unsafe fn battery() -> Option<(u32, bool)> {
    let mut p: SYSTEM_POWER_STATUS = zeroed();
    if GetSystemPowerStatus(&mut p) == 0 || p.BatteryFlag & 128 != 0 || p.BatteryLifePercent > 100 {
        return None;
    }
    Some((p.BatteryLifePercent as u32, p.ACLineStatus == 1))
}

/// The full path of a running program.
pub(crate) unsafe fn process_path(pid: u32) -> String {
    let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if h.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 520];
    let mut n = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n) != 0;
    CloseHandle(h);
    if ok { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() }
}

unsafe fn is_ours(w: HWND) -> bool {
    let mut pid = 0u32;
    GetWindowThreadProcessId(w, &mut pid);
    pid == GetCurrentProcessId()
}

/// Does this window fill its monitor the way games and films do?
unsafe fn fills_screen(w: HWND) -> bool {
    let mut cls = [0u16; 64];
    let n = GetClassNameW(w, cls.as_mut_ptr(), 64).max(0) as usize;
    let name = String::from_utf16_lossy(&cls[..n]);
    let mut r: RECT = zeroed();
    if GetWindowRect(w, &mut r) == 0 {
        return false;
    }
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromWindow(w, MONITOR_DEFAULTTONEAREST), &mut mi);
    let rc = |r: RECT| rules::Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    let has_caption = GetWindowLongW(w, GWL_STYLE) as u32 & WS_CAPTION == WS_CAPTION;
    rules::is_fullscreen_app(rc(r), rc(mi.rcMonitor), IsZoomed(w) != 0, has_caption, &name)
}

/// What the user is busy with: should the pet hide, and which program is in front.
pub(crate) unsafe fn busy_with(list: &[String]) -> (bool, String) {
    let fg = GetForegroundWindow();
    let (mut hide, mut app) = (false, String::new());
    if !fg.is_null() && !is_ours(fg) {
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, &mut pid);
        let path = process_path(pid);
        let exe = path.rsplit('\\').next().unwrap_or("").to_string();
        let mut t = [0u16; 256];
        let n = GetWindowTextW(fg, t.as_mut_ptr(), 256).max(0) as usize;
        let title = String::from_utf16_lossy(&t[..n]);
        hide = fills_screen(fg) || rules::is_game(&path) || rules::is_player(&exe) || rules::is_streaming(&exe, &title) || rules::is_listed(list, &exe);
        app = exe;
    }
    // a game or film may go on playing on another monitor while the user clicks elsewhere
    if !hide {
        for s in screens() {
            let (w, h) = (s.full.right - s.full.left, s.full.bottom - s.full.top);
            for (a, b) in [(1, 1), (2, 2), (3, 3)] {
                let at = WindowFromPoint(POINT { x: s.full.left + w * a / 4, y: s.full.top + h * b / 4 });
                let root = if at.is_null() { at } else { GetAncestor(at, GA_ROOT) };
                hide |= !root.is_null() && root != fg && !is_ours(root) && fills_screen(root);
            }
        }
    }
    // full-screen 3D and presentations, as Windows itself sees them
    let mut state = 0;
    if !hide && SHQueryUserNotificationState(&mut state) >= 0 {
        hide = state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE;
    }
    (hide, app)
}

/// Seconds since the user last touched the mouse or keyboard.
pub(crate) unsafe fn idle_secs() -> u64 {
    let mut li = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if GetLastInputInfo(&mut li) == 0 {
        return 0;
    }
    (GetTickCount().wrapping_sub(li.dwTime) / 1000) as u64
}

// ======================================================================
// clock, network counters, number formatting
// ======================================================================

/// Local time as (minutes since 1970, yyyymmdd, yyyymm).
pub(crate) unsafe fn local_now() -> (i64, u32, u32) {
    let mut t: SYSTEMTIME = zeroed();
    GetLocalTime(&mut t);
    let m = todo::minutes_from_civil(t.wYear as i64, t.wMonth as i64, t.wDay as i64, t.wHour as i64, t.wMinute as i64);
    let ym = t.wYear as u32 * 100 + t.wMonth as u32;
    (m, ym * 100 + t.wDay as u32, ym)
}

/// Total (received, sent) bytes across physical-ish adapters (wrap at 2^32).
/// Interface indexes worth counting (ethernet, wifi, ppp, mobile broadband), refreshed rarely:
/// walking the whole interface table costs milliseconds, reading one row costs microseconds.
static IFACES: Mutex<Option<(Vec<u32>, Instant)>> = Mutex::new(None);

unsafe fn scan_ifaces() -> Vec<u32> {
    let mut size = 0u32;
    GetIfTable(null_mut(), &mut size, 0);
    if size == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u32; size as usize / 4 + 1];
    let t = buf.as_mut_ptr() as *mut MIB_IFTABLE;
    if GetIfTable(t, &mut size, 0) != 0 {
        return Vec::new();
    }
    let rows = (*t).table.as_ptr();
    let mut v = Vec::new();
    for i in 0..(*t).dwNumEntries as usize {
        let r = &*rows.add(i);
        // skips loopback/tunnels
        if matches!(r.dwType, 6 | 71 | 23 | 243 | 244) && (r.dwOperStatus == 4 || r.dwOperStatus == 5) {
            v.push(r.dwIndex);
        }
    }
    v
}

pub(crate) unsafe fn totals() -> (u32, u32) {
    let ids = {
        let mut g = IFACES.lock().unwrap();
        if g.as_ref().map_or(true, |(_, at)| at.elapsed() > Duration::from_secs(20)) {
            *g = Some((scan_ifaces(), Instant::now()));
        }
        g.as_ref().unwrap().0.clone()
    };
    let (mut rx, mut tx) = (0u32, 0u32);
    for id in ids {
        let mut row: MIB_IFROW = zeroed();
        row.dwIndex = id;
        if GetIfEntry(&mut row) == 0 && (row.dwOperStatus == 4 || row.dwOperStatus == 5) {
            rx = rx.wrapping_add(row.dwInOctets);
            tx = tx.wrapping_add(row.dwOutOctets);
        }
    }
    (rx, tx)
}

pub(crate) fn scaled(v: f64) -> String {
    if v >= 100.0 {
        format!("{:.0}", v)
    } else if v >= 10.0 {
        format!("{:.1}", v)
    } else {
        format!("{:.2}", v)
    }
}

/// Auto-ranging rate: B/s, KB/s, MB/s, GB/s (or bps .. Gbps in bit mode).
pub(crate) fn format_rate(bytes_per_s: f64, bits: bool) -> String {
    let (mut v, units) = if bits {
        (bytes_per_s * 8.0, ["bps", "Kbps", "Mbps", "Gbps"])
    } else {
        (bytes_per_s, ["B/s", "KB/s", "MB/s", "GB/s"])
    };
    let mut i = 0;
    while v >= 1000.0 && i < 3 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{:.0} {}", v, units[0])
    } else {
        format!("{} {}", scaled(v), units[i])
    }
}

pub(crate) fn format_bytes(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < 4 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{} B", b)
    } else {
        format!("{} {}", scaled(v), U[i])
    }
}
