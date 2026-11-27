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

/// What the user is busy with: should the pet hide, the path of the program in front, and why it hides.
pub(crate) unsafe fn busy_with(list: &[String]) -> (bool, String, String) {
    let fg = GetForegroundWindow();
    let (mut hide, mut app, mut why) = (false, String::new(), String::new());
    if !fg.is_null() && !is_ours(fg) {
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, &mut pid);
        let path = process_path(pid);
        let exe = path.rsplit('\\').next().unwrap_or("").to_string();
        let mut t = [0u16; 256];
        let n = GetWindowTextW(fg, t.as_mut_ptr(), 256).max(0) as usize;
        let title = String::from_utf16_lossy(&t[..n]);
        let name = exe.trim_end_matches(".exe").trim_end_matches(".EXE").to_string();
        why = if rules::is_listed(list, &exe) {
            format!("{name} is on my list of apps to hide for")
        } else if rules::is_game(&path) {
            format!("{name} looks like a game")
        } else if rules::is_player(&exe) {
            format!("{name} plays films")
        } else if rules::is_streaming(&exe, &title) {
            "a film site is open".to_string()
        } else if fills_screen(fg) {
            format!("{name} fills the whole screen")
        } else {
            String::new()
        };
        hide = !why.is_empty();
        app = path;
    }
    // a game or film may go on playing on another monitor while the user clicks elsewhere
    if !hide {
        for s in screens() {
            let (w, h) = (s.full.right - s.full.left, s.full.bottom - s.full.top);
            for (a, b) in [(1, 1), (2, 2), (3, 3)] {
                let at = WindowFromPoint(POINT { x: s.full.left + w * a / 4, y: s.full.top + h * b / 4 });
                let root = if at.is_null() { at } else { GetAncestor(at, GA_ROOT) };
                if !hide && !root.is_null() && root != fg && !is_ours(root) && fills_screen(root) {
                    hide = true;
                    why = "a window fills a whole screen".to_string();
                }
            }
        }
    }
    // full-screen 3D and presentations, as Windows itself sees them
    let mut state = 0;
    if !hide && SHQueryUserNotificationState(&mut state) >= 0 {
        hide = state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE;
        if hide {
            why = if state == QUNS_PRESENTATION_MODE { "Windows is in presentation mode" } else { "Windows says a full-screen game is running" }.to_string();
        }
    }
    (hide, app, why)
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
    let mut found = Vec::new();
    for i in 0..(*t).dwNumEntries as usize {
        let r = &*rows.add(i);
        // skips loopback/tunnels
        if matches!(r.dwType, 6 | 71 | 23 | 243 | 244) && (r.dwOperStatus == 4 || r.dwOperStatus == 5) {
            let mac = r.bPhysAddr[..(r.dwPhysAddrLen as usize).min(8)].to_vec();
            let descr = String::from_utf8_lossy(&r.bDescr[..(r.dwDescrLen as usize).min(256)]).trim_end_matches('\0').to_string();
            found.push(Iface { index: r.dwIndex, mac, descr });
        }
    }
    one_per_adapter(&found)
}

/// An interface as the table lists it.
pub(crate) struct Iface {
    pub index: u32,
    pub mac: Vec<u8>,
    pub descr: String,
}

/// Windows lists each adapter again for every filter driver bound to it ("Intel Wi-Fi-QoS
/// Packet Scheduler-0000", "...-WFP Native MAC Layer LightWeight Filter-0000", antivirus
/// filters...), each repeating the adapter's counters. Counting them all multiplied the speed
/// and the data usage. Keep one per adapter: drop any whose description is another's plus
/// "-...", and any that shares a hardware address with one already kept.
pub(crate) fn one_per_adapter(found: &[Iface]) -> Vec<u32> {
    let mut kept: Vec<&Iface> = Vec::new();
    let mut by_index: Vec<&Iface> = found.iter().collect();
    by_index.sort_by_key(|f| f.index);
    for f in by_index {
        let layer = found.iter().any(|o| o.index != f.index && !o.descr.is_empty() && f.descr.starts_with(&format!("{}-", o.descr)));
        let same_mac = !f.mac.is_empty() && f.mac.iter().any(|&b| b != 0) && kept.iter().any(|k| k.mac == f.mac);
        if !layer && !same_mac {
            kept.push(f);
        }
    }
    kept.iter().map(|f| f.index).collect()
}

/// Each counted interface that is up: (index, bytes received, bytes sent). The counters are
/// 32-bit and wrap.
unsafe fn read_ifaces() -> Vec<(u32, u32, u32)> {
    let ids = {
        let mut g = IFACES.lock().unwrap();
        if g.as_ref().map_or(true, |(_, at)| at.elapsed() > Duration::from_secs(20)) {
            *g = Some((scan_ifaces(), Instant::now()));
        }
        g.as_ref().unwrap().0.clone()
    };
    let mut v = Vec::new();
    for id in ids {
        let mut row: MIB_IFROW = zeroed();
        row.dwIndex = id;
        if GetIfEntry(&mut row) == 0 && (row.dwOperStatus == 4 || row.dwOperStatus == 5) {
            v.push((id, row.dwInOctets, row.dwOutOctets));
        }
    }
    v
}

/// The most any one interface is believed to move per second (25 Gbit/s); a bigger step is a
/// counter that was reset, not traffic.
const MOST_PER_S: f64 = 3.2e9;

/// Bytes (received, sent) between two readings. Each interface is compared only with itself,
/// so one that appears or comes back (whose whole counter would otherwise look like traffic)
/// or goes away is left out until it has two readings, and a counter reset is skipped.
pub(crate) fn net_delta(old: &[(u32, u32, u32)], new: &[(u32, u32, u32)], secs: f64) -> (u64, u64) {
    let most = MOST_PER_S * secs.max(1.0);
    let (mut rx, mut tx) = (0u64, 0u64);
    for &(id, r, t) in new {
        if let Some(&(_, or, ot)) = old.iter().find(|o| o.0 == id) {
            let (dr, dt) = (r.wrapping_sub(or) as f64, t.wrapping_sub(ot) as f64);
            if dr <= most && dt <= most {
                rx += dr as u64;
                tx += dt as u64;
            }
        }
    }
    (rx, tx)
}

/// The last reading of every interface, and when it was taken.
static LAST_NET: Mutex<Option<(Vec<(u32, u32, u32)>, Instant)>> = Mutex::new(None);

/// Bytes (received, sent) since the last call, and the seconds that really passed (timer ticks
/// arrive late, and not at all while the PC sleeps). None on the first call.
pub(crate) unsafe fn net_sample() -> Option<(u64, u64, f64)> {
    let now = read_ifaces();
    let (old, at) = LAST_NET.lock().unwrap().replace((now.clone(), Instant::now()))?;
    let secs = at.elapsed().as_secs_f64();
    let (rx, tx) = net_delta(&old, &now, secs);
    Some((rx, tx, secs))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(index: u32, mac: &[u8], descr: &str) -> Iface {
        Iface { index, mac: mac.to_vec(), descr: descr.into() }
    }

    #[test]
    fn filter_layers_are_not_counted_again() {
        // this PC's table: one Wi-Fi adapter listed seven times, and WAN miniports with filters
        let wifi = [0x8c, 0x1d, 0x96, 1, 2, 3];
        let base = "Intel(R) Wi-Fi 6E AX211 160MHz";
        let mut t = vec![iface(5, &wifi, base), iface(9, &[], "WAN Miniport (IPv6)"), iface(12, &[], "WAN Miniport (IP)")];
        for (i, f) in ["WFP Native MAC Layer LightWeight Filter", "Virtual WiFi Filter Driver", "Native WiFi Filter Driver", "Kaspersky Lab NDIS 6 Filter", "QoS Packet Scheduler", "WFP 802.3 MAC Layer LightWeight Filter"].iter().enumerate() {
            t.push(iface(19 + i as u32, &wifi, &format!("{base}-{f}-0000")));
        }
        t.push(iface(35, &[], "WAN Miniport (IP)-QoS Packet Scheduler-0000"));
        assert_eq!(one_per_adapter(&t), vec![5, 9, 12]);
        // two real adapters both count
        let two = [iface(3, &[1, 1, 1, 1, 1, 1], "Realtek PCIe GbE"), iface(5, &wifi, base)];
        assert_eq!(one_per_adapter(&two), vec![3, 5]);
        // a filter layer listed before its adapter still goes
        let early = [iface(2, &wifi, &format!("{base}-QoS Packet Scheduler-0000")), iface(5, &wifi, base)];
        assert_eq!(one_per_adapter(&early), vec![5]);
        // the Linux/macOS port's single stand-in interface
        assert_eq!(one_per_adapter(&[iface(1, &[], "")]), vec![1]);
    }

    #[test]
    fn each_interface_counts_against_itself() {
        // wifi (7) moved 1000/200 bytes in a second, ethernet (3) 50/0
        let old = [(7, 5_000, 900), (3, 100, 100)];
        let new = [(7, 6_000, 1_100), (3, 150, 100)];
        assert_eq!(net_delta(&old, &new, 1.0), (1_050, 200));
    }

    #[test]
    fn an_interface_coming_or_going_is_not_traffic() {
        // a VPN (9) appears with 800 MB already on its counter: not a burst of speed
        let old = [(7, 5_000, 900)];
        let new = [(7, 5_500, 900), (9, 800_000_000, 40_000_000)];
        assert_eq!(net_delta(&old, &new, 1.0), (500, 0));
        // and when it goes away, the rest still count
        assert_eq!(net_delta(&new, &old[..], 1.0), (0, 0));
        assert_eq!(net_delta(&new, &[(7, 5_600, 950)], 1.0), (100, 50));
    }

    #[test]
    fn wrapping_counters_and_resets() {
        // a 32-bit counter rolling over still gives the bytes moved
        assert_eq!(net_delta(&[(1, u32::MAX - 99, 0)], &[(1, 400, 0)], 1.0), (500, 0));
        // a counter reset to near zero looks like ~4 GB in one second: skipped
        assert_eq!(net_delta(&[(1, 900_000_000, 0)], &[(1, 10, 0)], 1.0), (0, 0));
    }
}
