//! Talking to Windows: clock, network, CPU, memory, battery, idle time, fullscreen detection.

use crate::prelude::*;

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

/// Is a fullscreen app (game, video, slideshow) in front on any monitor?
pub(crate) unsafe fn fullscreen_active(own: HWND) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_null() || fg == own || fg == hw(&TODO_WND) || fg == hw(&STATS_WND) {
        return false;
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(fg, cls.as_mut_ptr(), 64).max(0) as usize;
    let name = String::from_utf16_lossy(&cls[..n]);
    let mut r: RECT = zeroed();
    if GetWindowRect(fg, &mut r) == 0 {
        return false;
    }
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST), &mut mi);
    let rc = |r: RECT| rules::Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    let has_caption = GetWindowLongW(fg, GWL_STYLE) as u32 & WS_CAPTION == WS_CAPTION;
    rules::is_fullscreen_app(rc(r), rc(mi.rcMonitor), IsZoomed(fg) != 0, has_caption, &name)
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
pub(crate) unsafe fn totals() -> (u32, u32) {
    let mut size = 0u32;
    GetIfTable(null_mut(), &mut size, 0);
    if size == 0 {
        return (0, 0);
    }
    let mut buf = vec![0u32; size as usize / 4 + 1];
    let t = buf.as_mut_ptr() as *mut MIB_IFTABLE;
    if GetIfTable(t, &mut size, 0) != 0 {
        return (0, 0);
    }
    let rows = (*t).table.as_ptr();
    let (mut rx, mut tx) = (0u32, 0u32);
    for i in 0..(*t).dwNumEntries as usize {
        let r = &*rows.add(i);
        // ethernet, wifi, ppp, mobile broadband; skips loopback/tunnels
        let kind = matches!(r.dwType, 6 | 71 | 23 | 243 | 244);
        if kind && (r.dwOperStatus == 4 || r.dwOperStatus == 5) {
            rx = rx.wrapping_add(r.dwInOctets);
            tx = tx.wrapping_add(r.dwOutOctets);
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
