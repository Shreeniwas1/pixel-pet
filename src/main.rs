//! Pixel Pet: a tiny always-on-top desktop pet that shows download speed
//! and the occasional cool fact, with a smart todo list. Plain Win32 + GDI, no runtime, no webview.
#![windows_subsystem = "windows"]

mod todo;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU32, Ordering::Relaxed};
use std::sync::Mutex;
use todo::Todo;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// ---- layout (logical px, scaled by DPI) ----
const WIN_W: i32 = 340;
const WIN_H: i32 = 250;
const MARGIN: i32 = 12; // gap from screen corner
const PX: i32 = 6; // one sprite pixel
const TICK_MS: u32 = 500;
const FACT_EVERY: u32 = 120; // ticks (60 s)
const FACT_FOR: u32 = 18; // ticks (9 s)

// ---- palette ----
const KEY: u32 = rgb(255, 0, 255); // transparent colour
const INK: u32 = rgb(0x2b, 0x21, 0x40);
const ORANGE: u32 = rgb(0xf2, 0xa6, 0x5a);
const PINK: u32 = rgb(0xff, 0x8f, 0xa3);
const CREAM: u32 = rgb(0xff, 0xf1, 0xd6);
const GREEN: u32 = rgb(0x7e, 0xe0, 0xa0);
const UP_BLUE: u32 = rgb(0x7e, 0xc8, 0xff);

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

struct Pet {
    name: &'static str,
    face: u8, // fill colour char used when the eyes blink shut
    grid: &'static [&'static [u8]],
}

static PETS: [Pet; 4] = [
    Pet {
        name: "Cat",
        face: b'O',
        grid: &[
            b".KK......KK.",
            b".KOK....KOK.",
            b".KOOKKKKOOK.",
            b"KOOOOOOOOOOK",
            b"KOOEOOOOEOOK",
            b"KOOOOPPOOOOK",
            b"KOWWOOOOWWOK",
            b".KOOOOOOOOK.",
            b".KOOOOOOOOK.",
            b".KWWKOOKWWK.",
            b"..KKK..KKK..",
        ],
    },
    Pet {
        name: "Dog",
        face: b'D',
        grid: &[
            b"..KKKKKKKK..",
            b".KDDDDDDDDK.",
            b"KdKDDDDDDKdK",
            b"KddDEDDEDddK",
            b"KddDDDDDDddK",
            b"KdKDWWWWDKdK",
            b".KKDWKKWDKK.",
            b"..KDWWWWDK..",
            b"..KDDPPDDK..",
            b"...KKKKKKK..",
            b"............",
        ],
    },
    Pet {
        name: "Claude",
        face: b'C',
        grid: &[
            b"............",
            b"..KKKKKKKK..",
            b".KCCCCCCCCK.",
            b".KCCECCECCK.",
            b"KKCCECCECCKK",
            b"KCCCCCCCCCCK",
            b"KKCCCCCCCCKK",
            b".KCCCCCCCCK.",
            b".KCLCCCCLCK.",
            b".KKCKKKKCKK.",
            b"..KK....KK..",
        ],
    },
    Pet {
        name: "Agent",
        face: b'S',
        grid: &[
            b"....KKKKKKKK....",
            b"...KHHHHHHHHK...",
            b"..KHHhhHHHHHHK..",
            b"..KHHHHHHHHHHK..",
            b"..KHSSSSSSSSHK..",
            b"..KSSSSSSSSSSK..",
            b"..KBGBBKKBGBBK..",
            b"..KSBBBSSBBBSK..",
            b"..KSSSSsSSSSSK..",
            b"..KSSSSSSKKSSK..",
            b"...KsSSSSSSsK...",
            b"....KKWWWWKK....",
            b"..KTTWWRRWWTTK..",
            b".KTtTWWrrWWTtTK.",
            b"KTTtTWWWWWWTPTTK",
        ],
    },
];

fn pet_color(c: u8) -> Option<u32> {
    Some(match c {
        b'K' | b'E' => INK,
        b'O' => ORANGE,
        b'P' => PINK,
        b'W' => CREAM,
        b'D' => rgb(0xbf, 0x82, 0x4f),
        b'd' => rgb(0x7a, 0x4b, 0x2a),
        b'C' => rgb(0xd9, 0x77, 0x57),
        b'L' => rgb(0xf0, 0xa5, 0x8a),
        b'S' => rgb(0xf5, 0xc9, 0xa0),
        b'H' => rgb(0x4a, 0x34, 0x26),
        b'T' => rgb(0x3a, 0x3f, 0x5c),
        b'R' => rgb(0xd6, 0x40, 0x4e),
        b'r' => rgb(0x9c, 0x2a, 0x36),
        b'h' => rgb(0x6b, 0x4a, 0x36),
        b's' => rgb(0xd9, 0xa5, 0x77),
        b'B' => rgb(0x0d, 0x0b, 0x14),
        b'G' => rgb(0xdd, 0xe6, 0xff),
        b't' => rgb(0x50, 0x57, 0x80),
        _ => return None,
    })
}

const ARROW: [&[u8]; 7] = [b"...X...", b"...X...", b"...X...", b"XXXXXXX", b".XXXXX.", b"..XXX..", b"...X..."];

// Facts come from data/facts.txt (one per line, bundled into the exe).
static FACTS_TXT: &str = include_str!("../data/facts.txt");

fn fact_count() -> usize {
    FACTS_TXT.lines().count().max(1)
}

fn fact_text(i: usize) -> String {
    format!("Did you know {}", FACTS_TXT.lines().nth(i).unwrap_or("that facts are fun?"))
}

// ======================================================================
// state
// ======================================================================

const SIZES: [f32; 3] = [0.75, 1.0, 1.4]; // small, medium, large

struct State {
    scale: f32, // system DPI scale
    size: usize,
    tick: u32,
    last_in: u32,
    last_out: u32,
    speed: f64, // download, bytes per second
    up: f64,    // upload, bytes per second
    fact: usize,
    fact_left: u32,
    pet: usize,
    facts_on: bool,
    show_up: bool,
    bits: bool,
    hide_fs: bool,
    pos: Option<(i32, i32)>,
    // todo list
    todos: Vec<Todo>,
    next_id: u32,
    alert: Option<u32>,
    view: Vec<u32>, // todo ids in list order
    todo_sel: Option<u32>,
    todo_scroll: usize,
    todo_prev: String,
    // click targets recorded by paint (client coordinates)
    btns: Vec<(RECT, u8)>,
    pet_rect: RECT,
    fact_rect: Option<RECT>,
    // mouse
    down: bool,
    dragging: bool,
    cur0: (i32, i32),
    win0: (i32, i32),
    // visibility
    user_hidden: bool,
    visible: bool,
    // data usage (bytes)
    day_key: u32,
    day_rx: u64,
    day_tx: u64,
    mon_key: u32,
    mon_rx: u64,
    mon_tx: u64,
}

const NO_RECT: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };

static ST: Mutex<State> = Mutex::new(State {
    scale: 1.0,
    size: 1,
    tick: 0,
    last_in: 0,
    last_out: 0,
    speed: 0.0,
    up: 0.0,
    fact: 0,
    fact_left: 0,
    pet: 0,
    facts_on: true,
    show_up: true,
    bits: false,
    hide_fs: true,
    pos: None,
    todos: Vec::new(),
    next_id: 1,
    alert: None,
    view: Vec::new(),
    todo_sel: None,
    todo_scroll: 0,
    todo_prev: String::new(),
    btns: Vec::new(),
    pet_rect: NO_RECT,
    fact_rect: None,
    down: false,
    dragging: false,
    cur0: (0, 0),
    win0: (0, 0),
    user_hidden: false,
    visible: true,
    day_key: 0,
    day_rx: 0,
    day_tx: 0,
    mon_key: 0,
    mon_rx: 0,
    mon_tx: 0,
});

static PET_WND: AtomicIsize = AtomicIsize::new(0);
static TODO_WND: AtomicIsize = AtomicIsize::new(0);
static TODO_EDIT: AtomicIsize = AtomicIsize::new(0);
static TB_CREATED: AtomicU32 = AtomicU32::new(0);

fn hw(a: &AtomicIsize) -> HWND {
    a.load(Relaxed) as HWND
}

fn eff_scale(s: &State) -> f32 {
    s.scale * SIZES[s.size]
}

fn win_dims(scale: f32) -> (i32, i32) {
    ((WIN_W as f32 * scale).round() as i32, (WIN_H as f32 * scale).round() as i32)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn contains(r: &RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

// ======================================================================
// files: settings, usage, todos (all in %APPDATA%)
// ======================================================================

fn data_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join(name))
}

fn load_cfg(s: &mut State) {
    let Some(txt) = data_path("pixel-pet.cfg").and_then(|p| std::fs::read_to_string(p).ok()) else { return };
    for (n, line) in txt.lines().enumerate() {
        // older versions wrote bare lines: pet, facts, upload
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (["pet", "facts", "up"].get(n).copied().unwrap_or(""), line.trim()),
        };
        match k {
            "pet" => {
                if let Ok(i) = v.parse::<usize>() {
                    if i < PETS.len() {
                        s.pet = i;
                    }
                }
            }
            "facts" => s.facts_on = v != "0",
            "up" => s.show_up = v != "0",
            "bits" => s.bits = v == "1",
            "fs" => s.hide_fs = v != "0",
            "size" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.size = i.min(SIZES.len() - 1);
                }
            }
            "x" => {
                if let Ok(x) = v.parse::<i32>() {
                    s.pos = Some((x, s.pos.map_or(0, |p| p.1)));
                }
            }
            "y" => {
                if let Ok(y) = v.parse::<i32>() {
                    s.pos = Some((s.pos.map_or(0, |p| p.0), y));
                }
            }
            _ => {}
        }
    }
}

fn save_cfg(s: &State) {
    if let Some(p) = data_path("pixel-pet.cfg") {
        let mut t = format!(
            "pet={}\nfacts={}\nup={}\nbits={}\nfs={}\nsize={}\n",
            s.pet, s.facts_on as u8, s.show_up as u8, s.bits as u8, s.hide_fs as u8, s.size
        );
        if let Some((x, y)) = s.pos {
            t.push_str(&format!("x={x}\ny={y}\n"));
        }
        let _ = std::fs::write(p, t);
    }
}

fn load_usage(s: &mut State) {
    let Some(txt) = data_path("pixel-pet-usage.txt").and_then(|p| std::fs::read_to_string(p).ok()) else { return };
    let v: Vec<u64> = txt.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if v.len() == 6 {
        s.day_key = v[0] as u32;
        s.day_rx = v[1];
        s.day_tx = v[2];
        s.mon_key = v[3] as u32;
        s.mon_rx = v[4];
        s.mon_tx = v[5];
    }
}

fn save_usage(s: &State) {
    if let Some(p) = data_path("pixel-pet-usage.txt") {
        let _ = std::fs::write(
            p,
            format!("{} {} {} {} {} {}\n", s.day_key, s.day_rx, s.day_tx, s.mon_key, s.mon_rx, s.mon_tx),
        );
    }
}

fn save_todos(s: &State) {
    if let Some(p) = data_path("pixel-pet-todos.txt") {
        todo::save(&p, &s.todos);
    }
}

fn next_fact(s: &mut State) {
    s.fact = (s.fact + 1 + (s.tick as usize * 7) % (fact_count() - 1)) % fact_count();
}

// ======================================================================
// clock, network counters, number formatting
// ======================================================================

/// Local time as (minutes since 1970, yyyymmdd, yyyymm).
unsafe fn local_now() -> (i64, u32, u32) {
    let mut t: SYSTEMTIME = zeroed();
    GetLocalTime(&mut t);
    let m = todo::minutes_from_civil(t.wYear as i64, t.wMonth as i64, t.wDay as i64, t.wHour as i64, t.wMinute as i64);
    let ym = t.wYear as u32 * 100 + t.wMonth as u32;
    (m, ym * 100 + t.wDay as u32, ym)
}

/// Total (received, sent) bytes across physical-ish adapters (wrap at 2^32).
unsafe fn totals() -> (u32, u32) {
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

fn scaled(v: f64) -> String {
    if v >= 100.0 {
        format!("{:.0}", v)
    } else if v >= 10.0 {
        format!("{:.1}", v)
    } else {
        format!("{:.2}", v)
    }
}

/// Auto-ranging rate: B/s, KB/s, MB/s, GB/s (or bps .. Gbps in bit mode).
fn format_rate(bytes_per_s: f64, bits: bool) -> String {
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

fn format_bytes(b: u64) -> String {
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

// ======================================================================
// window placement, monitors
// ======================================================================

unsafe fn monitor_work(mon: HMONITOR) -> RECT {
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(mon, &mut mi);
    mi.rcWork
}

unsafe fn work_at(x: i32, y: i32) -> RECT {
    monitor_work(MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST))
}

unsafe fn primary_work() -> RECT {
    monitor_work(MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY))
}

unsafe extern "system" fn enum_mon(m: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
    (*(lp as *mut Vec<RECT>)).push(monitor_work(m));
    1
}

unsafe fn all_work_areas() -> Vec<RECT> {
    let mut v: Vec<RECT> = Vec::new();
    EnumDisplayMonitors(null_mut(), null(), Some(enum_mon), &mut v as *mut _ as LPARAM);
    v
}

fn clamp_in(wa: &RECT, x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    (x.clamp(wa.left, (wa.right - w).max(wa.left)), y.clamp(wa.top, (wa.bottom - h).max(wa.top)))
}

/// Which corner of its monitor's work area the pet window sits in.
unsafe fn flips(hwnd: HWND) -> (bool, bool) {
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let wa = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    ((wr.left + wr.right) < (wa.left + wa.right), (wr.top + wr.bottom) < (wa.top + wa.bottom))
}

unsafe fn place(hwnd: HWND) {
    let (scale, pos) = {
        let s = ST.lock().unwrap();
        (eff_scale(&s), s.pos)
    };
    let (w, h) = win_dims(scale);
    let (x, y) = match pos {
        Some((x, y)) => {
            let wa = work_at(x + w / 2, y + h / 2);
            clamp_in(&wa, x, y, w, h)
        }
        None => {
            let wa = primary_work();
            let m = (MARGIN as f32 * scale).round() as i32;
            (wa.right - w - m, wa.bottom - h - m)
        }
    };
    SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
}

unsafe fn put_in_corner(hwnd: HWND, right: bool, bottom: bool, wa: RECT) {
    let (w, h, m) = {
        let s = ST.lock().unwrap();
        let (w, h) = win_dims(eff_scale(&s));
        (w, h, (MARGIN as f32 * eff_scale(&s)).round() as i32)
    };
    let x = if right { wa.right - w - m } else { wa.left + m };
    let y = if bottom { wa.bottom - h - m } else { wa.top + m };
    {
        let mut s = ST.lock().unwrap();
        s.pos = Some((x, y));
        save_cfg(&s);
    }
    place(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn next_monitor(hwnd: HWND) {
    let areas = all_work_areas();
    if areas.len() < 2 {
        return;
    }
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let cur = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    let idx = areas.iter().position(|a| a.left == cur.left && a.top == cur.top).unwrap_or(0);
    let next = areas[(idx + 1) % areas.len()];
    let (fx, fy) = flips(hwnd);
    put_in_corner(hwnd, !fx, !fy, next);
}

unsafe fn set_size(hwnd: HWND, idx: usize) {
    let (fx, fy) = flips(hwnd);
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    {
        let mut s = ST.lock().unwrap();
        s.size = idx;
        let (w, h) = win_dims(eff_scale(&s));
        // keep the corner the pet sits in where it is
        let x = if fx { wr.left } else { wr.right - w };
        let y = if fy { wr.top } else { wr.bottom - h };
        s.pos = Some((x, y));
        save_cfg(&s);
    }
    place(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

// ======================================================================
// startup entry, tray icon, single instance
// ======================================================================

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "PixelPet";

unsafe fn autostart_enabled() -> bool {
    let mut k: HKEY = null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_QUERY_VALUE, &mut k) != 0 {
        return false;
    }
    let r = RegQueryValueExW(k, wide(RUN_NAME).as_ptr(), null(), null_mut(), null_mut(), null_mut());
    RegCloseKey(k);
    r == 0
}

unsafe fn set_autostart(on: bool) {
    let mut k: HKEY = null_mut();
    if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut k) != 0 {
        return;
    }
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let v = wide(&format!("\"{}\"", exe.display()));
            RegSetValueExW(k, wide(RUN_NAME).as_ptr(), 0, REG_SZ, v.as_ptr() as *const u8, (v.len() * 2) as u32);
        }
    } else {
        RegDeleteValueW(k, wide(RUN_NAME).as_ptr());
    }
    RegCloseKey(k);
}

const TRAY_MSG: u32 = WM_APP + 1;

/// 16x16 icon drawn from the current pet's sprite.
unsafe fn make_icon(pet: usize) -> HICON {
    let g = PETS[pet].grid;
    let (gh, gw) = (g.len(), g[0].len());
    let mut xor = [0u8; 16 * 16 * 4];
    let mut and = [0xFFu8; 32];
    let (ox, oy) = ((16 - gw.min(16)) / 2, (16 - gh.min(16)) / 2);
    for j in 0..gh.min(16) {
        for i in 0..gw.min(16) {
            if let Some(c) = pet_color(g[j][i]) {
                let (x, y) = (ox + i, oy + j);
                let o = (y * 16 + x) * 4;
                xor[o] = (c >> 16) as u8;
                xor[o + 1] = (c >> 8) as u8;
                xor[o + 2] = c as u8;
                xor[o + 3] = 255;
                and[y * 2 + x / 8] &= !(0x80u8 >> (x % 8));
            }
        }
    }
    CreateIcon(GetModuleHandleW(null()), 16, 16, 1, 32, and.as_ptr(), xor.as_ptr())
}

unsafe fn tray(hwnd: HWND, op: u32) {
    let pet = ST.lock().unwrap().pet;
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = TRAY_MSG;
    nid.hIcon = make_icon(pet);
    for (i, c) in "Pixel Pet".encode_utf16().enumerate() {
        nid.szTip[i] = c;
    }
    Shell_NotifyIconW(op, &nid);
    if !nid.hIcon.is_null() {
        DestroyIcon(nid.hIcon);
    }
}

// ======================================================================
// drawing
// ======================================================================

unsafe fn fill(dc: HDC, x: i32, y: i32, w: i32, h: i32, c: u32) {
    SetDCBrushColor(dc, c);
    let r = RECT { left: x, top: y, right: x + w, bottom: y + h };
    FillRect(dc, &r, GetStockObject(DC_BRUSH));
}

unsafe fn sprite(dc: HDC, rows: &[&[u8]], x: i32, y: i32, px: i32, pal: &dyn Fn(u8) -> Option<u32>) {
    for (j, row) in rows.iter().enumerate() {
        for (i, &ch) in row.iter().enumerate() {
            if let Some(c) = pal(ch) {
                fill(dc, x + i as i32 * px, y + j as i32 * px, px, px, c);
            }
        }
    }
}

const ALERT_ORANGE: u32 = rgb(0xff, 0xb4, 0x54);
const BTN_GREEN: u32 = rgb(0x2e, 0x8b, 0x57);
const BTN_GRAY: u32 = rgb(0xb8, 0xb3, 0xc9);

unsafe fn paint(hwnd: HWND) {
    let (scale, tick, speed, up, show_up, bits, fact, alert, pet) = {
        let s = ST.lock().unwrap();
        let alert = s.alert.and_then(|id| s.todos.iter().find(|t| t.id == id)).map(|t| t.text.clone());
        let fact = if alert.is_none() && s.fact_left > 0 { Some(fact_text(s.fact)) } else { None };
        (eff_scale(&s), s.tick, s.speed, s.up, s.show_up, s.bits, fact, alert, &PETS[s.pet])
    };
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let (w, h) = win_dims(scale);
    let (flipx, flipy) = flips(hwnd);

    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, w, h);
    let old_bmp = SelectObject(dc, bmp);
    SetBkMode(dc, TRANSPARENT as i32);
    fill(dc, 0, 0, w, h, KEY);

    let font = CreateFontW(
        -u(13), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3 /* NONANTIALIASED */, 49, wide("Consolas").as_ptr(),
    );
    let old_font = SelectObject(dc, font);

    // --- pet (bobs every tick, blinks now and then); sits in whichever corner the window is in ---
    let px = u(PX);
    let (gh, gw) = (pet.grid.len(), pet.grid[0].len());
    let (cw, ch) = (gw as i32 * px, gh as i32 * px);
    let cx = if flipx { u(8) } else { w - cw - u(8) };
    let base_y = if flipy { u(8) } else { h - ch - u(8) };
    let bob = if tick % 2 == 0 { 0 } else { -px };
    let cy = base_y + bob;
    let blink = tick % 16 == 0;
    let mut grid: Vec<Vec<u8>> = pet.grid.iter().map(|r| r.to_vec()).collect();
    if blink {
        // eyes drop one row (or close) while blinking
        for j in 0..gh {
            for i in 0..gw {
                if pet.grid[j][i] == b'E' {
                    grid[j][i] = pet.face;
                    if j + 1 < gh && pet.grid[j + 1][i] == pet.face {
                        grid[j + 1][i] = b'E';
                    }
                }
            }
        }
    }
    let rows: Vec<&[u8]> = grid.iter().map(|r| &r[..]).collect();
    sprite(dc, &rows, cx, cy, px, &pet_color);
    let pet_rect = RECT { left: cx, top: cy, right: cx + cw, bottom: cy + ch };

    // --- speed bubble beside the pet (download, plus upload if enabled) ---
    let ap = u(3);
    let (row_h, bd) = (u(26), u(3));
    let nrows = if show_up { 2 } else { 1 };
    let (bw, bh) = (u(142), row_h * nrows + 2 * bd);
    let bx = if flipx { cx + cw + u(12) } else { cx - bw - u(12) };
    let by = if flipy { u(10) } else { h - bh - u(10) };
    fill(dc, bx, by, bw, bh, GREEN);
    fill(dc, bx + bd, by + bd, bw - 2 * bd, bh - 2 * bd, INK);
    let mut lines: Vec<(f64, u32, bool)> = vec![(speed, GREEN, true)];
    if show_up {
        lines.push((up, UP_BLUE, false));
    }
    for (n, (bps, col, down)) in lines.into_iter().enumerate() {
        let ry = by + bd + row_h * n as i32;
        let arrow: Vec<&[u8]> = if down { ARROW.to_vec() } else { ARROW.iter().rev().copied().collect() };
        sprite(dc, &arrow, bx + bd + u(10), ry + (row_h - 7 * ap) / 2, ap, &|c| (c == b'X').then_some(col));
        SetTextColor(dc, col);
        let txt = wide(&format_rate(bps, bits));
        let mut tr = RECT { left: bx + bd + u(40), top: ry, right: bx + bw - bd, bottom: ry + row_h };
        DrawTextW(dc, txt.as_ptr(), -1, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
    }

    // --- message bubble (reminder or fact), above the pet (below it when the pet is at the top) ---
    let mut btns: Vec<(RECT, u8)> = Vec::new();
    let mut fact_rect: Option<RECT> = None;
    let (pad, bdr, sh) = (u(10), u(3), u(4));
    let (fx, fw) = (u(2), w - u(2) - sh - u(2));
    let anchor_y = |bh: i32| -> i32 {
        if flipy { base_y + ch + u(12) } else { (base_y - u(12) - bh).max(u(2)) }
    };
    let measure = |dc: HDC, t: &[u16]| -> i32 {
        let mut m = RECT { left: 0, top: 0, right: fw - 2 * (pad + bdr), bottom: 0 };
        DrawTextW(dc, t.as_ptr(), -1, &mut m, DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX);
        m.bottom
    };
    if let Some(text) = alert {
        let mut shown: String = text.chars().take(90).collect();
        if shown.len() < text.len() {
            shown.push_str("...");
        }
        let t = wide(&shown);
        let th = measure(dc, &t);
        let (head_h, btn_h, gap) = (u(18), u(24), u(8));
        let fh = 2 * (bdr + pad) + head_h + th + gap + btn_h;
        let fy = anchor_y(fh);
        fill(dc, fx + sh, fy + sh, fw, fh, INK);
        fill(dc, fx, fy, fw, fh, ALERT_ORANGE);
        fill(dc, fx + bdr, fy + bdr, fw - 2 * bdr, fh - 2 * bdr, CREAM);
        SetTextColor(dc, INK);
        let head = wide("REMINDER");
        let mut hr = RECT { left: fx + bdr + pad, top: fy + bdr + pad, right: fx + fw, bottom: fy + bdr + pad + head_h };
        DrawTextW(dc, head.as_ptr(), -1, &mut hr, DT_SINGLELINE | DT_NOPREFIX);
        let mut tr = RECT {
            left: fx + bdr + pad,
            top: fy + bdr + pad + head_h,
            right: fx + fw - bdr - pad,
            bottom: fy + bdr + pad + head_h + th,
        };
        DrawTextW(dc, t.as_ptr(), -1, &mut tr, DT_WORDBREAK | DT_NOPREFIX);
        // click options
        let inner = fw - 2 * (bdr + pad);
        let g2 = u(6);
        let bw3 = (inner - 2 * g2) / 3;
        let by0 = fy + fh - bdr - pad - btn_h;
        for (i, (label, bg, fg)) in [("DONE", BTN_GREEN, CREAM), ("+10 MIN", INK, CREAM), ("LATER", BTN_GRAY, INK)]
            .into_iter()
            .enumerate()
        {
            let x0 = fx + bdr + pad + i as i32 * (bw3 + g2);
            fill(dc, x0, by0, bw3, btn_h, bg);
            SetTextColor(dc, fg);
            let l = wide(label);
            let mut r = RECT { left: x0, top: by0, right: x0 + bw3, bottom: by0 + btn_h };
            DrawTextW(dc, l.as_ptr(), -1, &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_NOPREFIX);
            btns.push((RECT { left: x0, top: by0, right: x0 + bw3, bottom: by0 + btn_h }, i as u8));
        }
    } else if let Some(f) = fact {
        let ft = wide(&f);
        let fh = measure(dc, &ft) + 2 * (pad + bdr);
        let fy = anchor_y(fh);
        fill(dc, fx + sh, fy + sh, fw, fh, INK);
        fill(dc, fx, fy, fw, fh, INK);
        fill(dc, fx + bdr, fy + bdr, fw - 2 * bdr, fh - 2 * bdr, CREAM);
        SetTextColor(dc, INK);
        let mut r = RECT { left: fx + bdr + pad, top: fy + bdr + pad, right: fx + fw - bdr - pad, bottom: fy + fh };
        DrawTextW(dc, ft.as_ptr(), -1, &mut r, DT_WORDBREAK | DT_NOPREFIX);
        fact_rect = Some(RECT { left: fx, top: fy, right: fx + fw, bottom: fy + fh });
    }

    {
        let mut s = ST.lock().unwrap();
        s.btns = btns;
        s.pet_rect = pet_rect;
        s.fact_rect = fact_rect;
    }

    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old_font);
    DeleteObject(font);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}

// ======================================================================
// periodic tick: network, usage, facts, reminders, fullscreen check
// ======================================================================

unsafe fn on_tick(hwnd: HWND) {
    let (now_in, now_out) = totals();
    let (now, dkey, mkey) = local_now();
    let mut beep = false;
    let mut vis_change: Option<bool> = None;
    let keep_top;
    {
        let mut s = ST.lock().unwrap();
        s.tick += 1;
        let d_in = now_in.wrapping_sub(s.last_in) as f64;
        let d_out = now_out.wrapping_sub(s.last_out) as f64;
        s.last_in = now_in;
        s.last_out = now_out;
        // ignore bogus jumps (adapter appeared/disappeared)
        if d_in < 1.5e9 && d_out < 1.5e9 {
            let k = 1000.0 / TICK_MS as f64;
            s.speed = s.speed * 0.4 + d_in * k * 0.6;
            s.up = s.up * 0.4 + d_out * k * 0.6;
            if s.day_key != dkey {
                s.day_key = dkey;
                s.day_rx = 0;
                s.day_tx = 0;
            }
            if s.mon_key != mkey {
                s.mon_key = mkey;
                s.mon_rx = 0;
                s.mon_tx = 0;
            }
            s.day_rx += d_in as u64;
            s.day_tx += d_out as u64;
            s.mon_rx += d_in as u64;
            s.mon_tx += d_out as u64;
        }
        if s.speed < 500.0 {
            s.speed = 0.0;
        }
        if s.up < 500.0 {
            s.up = 0.0;
        }

        // facts
        if s.fact_left > 0 {
            s.fact_left -= 1;
        } else if s.facts_on && s.alert.is_none() && s.tick % FACT_EVERY == 10 {
            next_fact(&mut s);
            s.fact_left = FACT_FOR;
        }

        // reminders: first overdue, not-yet-shown item
        if s.alert.is_none() {
            if let Some(t) = s.todos.iter_mut().find(|t| !t.done && !t.fired && t.due.map_or(false, |d| d <= now)) {
                t.fired = true;
                let id = t.id;
                s.alert = Some(id);
                s.fact_left = 0;
                beep = true;
            }
        }

        // hide while a fullscreen app / game / presentation owns the screen
        let mut q: i32 = 0;
        let fs = s.hide_fs && SHQueryUserNotificationState(&mut q) >= 0 && matches!(q, 2 | 3 | 4);
        let want = !s.user_hidden && !fs;
        if want != s.visible {
            s.visible = want;
            vis_change = Some(want);
        }
        keep_top = s.visible && s.tick % 20 == 0;
        if s.tick % 120 == 0 {
            save_usage(&s);
        }
    }
    if beep {
        MessageBeep(MB_ICONASTERISK);
        refresh_list();
    }
    if let Some(v) = vis_change {
        ShowWindow(hwnd, if v { SW_SHOWNOACTIVATE } else { SW_HIDE });
    }
    if keep_top {
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
    InvalidateRect(hwnd, null(), 0);
}

// ======================================================================
// todo window (custom pixel-drawn panel + one real edit box)
// ======================================================================

const TODO_W: i32 = 440;
const TODO_H: i32 = 430;
const ROW_H: i32 = 32;
const ROWS_VISIBLE: usize = 7;
const PANEL: u32 = rgb(0x1a, 0x14, 0x28);
const PANEL_LIGHT: u32 = rgb(0x3d, 0x31, 0x60);
const PANEL_EDGE: u32 = rgb(0x5a, 0x4a, 0x85);
const DIM: u32 = rgb(0x8a, 0x7f, 0xb0);
const HINT: u32 = rgb(0xa8, 0x9c, 0xc8);
const RED: u32 = rgb(0xff, 0x78, 0x78);
const BTN_RED: u32 = rgb(0xa8, 0x32, 0x3e);

static HOVER: AtomicI32 = AtomicI32::new(-1);
static EDIT_FONT: AtomicIsize = AtomicIsize::new(0);

const CHECK: [&[u8]; 5] = [b".....X", b"X...XX", b"XX.XX.", b".XXX..", b"..X..."];
const CROSS: [&[u8]; 7] = [b"X.....X", b".X...X.", b"..X.X..", b"...X...", b"..X.X..", b".X...X.", b"X.....X"];

struct Lay {
    close: RECT,
    input: RECT,
    prev: RECT,
    list: RECT,
    btns: [RECT; 5],
    hint: RECT,
    title_h: i32,
}

fn rc(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT { left: x, top: y, right: x + w, bottom: y + h }
}

fn layout(k: &dyn Fn(i32) -> i32) -> Lay {
    let bw = (412 - 24) / 5;
    let mut btns = [NO_RECT; 5];
    for (i, b) in btns.iter_mut().enumerate() {
        *b = rc(k(14 + i as i32 * (bw + 6)), k(340), k(bw), k(32));
    }
    Lay {
        close: rc(k(TODO_W - 3 - 28), k(3), k(24), k(24)),
        input: rc(k(14), k(42), k(412), k(30)),
        prev: rc(k(14), k(78), k(412), k(18)),
        list: rc(k(14), k(102), k(412), k(228)),
        btns,
        hint: rc(k(14), k(382), k(412), k(40)),
        title_h: k(30),
    }
}

unsafe fn get_text(h: HWND) -> String {
    let n = GetWindowTextLengthW(h);
    if n <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; n as usize + 1];
    let got = GetWindowTextW(h, buf.as_mut_ptr(), n + 1);
    String::from_utf16_lossy(&buf[..got.max(0) as usize])
}

unsafe fn create_todo_window() -> HWND {
    let dpi = ST.lock().unwrap().scale;
    let k = |v: i32| (v as f32 * dpi).round() as i32;
    let wa = primary_work();
    let (ww, wh) = (k(TODO_W), k(TODO_H));
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        wide("PixelTodoWnd").as_ptr(),
        wide("Pixel Pet - Todo").as_ptr(),
        WS_POPUP,
        wa.left + (wa.right - wa.left - ww) / 2,
        wa.top + (wa.bottom - wa.top - wh) / 2,
        ww, wh,
        null_mut(), null_mut(),
        GetModuleHandleW(null()),
        null(),
    );
    TODO_WND.store(hwnd as isize, Relaxed);
    let l = layout(&k);
    let edit = CreateWindowExW(
        0,
        wide("EDIT").as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | 0x80, /* ES_AUTOHSCROLL */
        l.input.left + k(8), l.input.top + k(5), k(412 - 16), k(20),
        hwnd, null_mut(), GetModuleHandleW(null()), null(),
    );
    let f = CreateFontW(-k(15), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    EDIT_FONT.store(f as isize, Relaxed);
    SendMessageW(edit, WM_SETFONT, f as WPARAM, 1);
    TODO_EDIT.store(edit as isize, Relaxed);
    hwnd
}

unsafe fn text_at(dc: HDC, s: &str, r: &mut RECT, flags: u32) {
    let t = wide(s);
    DrawTextW(dc, t.as_ptr(), -1, r, flags | DT_NOPREFIX);
}

unsafe fn pixel_button(dc: HDC, r: &RECT, label: &str, bg: u32, hot: bool) {
    let e = 2;
    let (fill_c, text_c) = if hot { (CREAM, INK) } else { (bg, CREAM) };
    fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, if hot { ORANGE } else { PANEL_EDGE });
    fill(dc, r.left + e, r.top + e, r.right - r.left - 2 * e, r.bottom - r.top - 2 * e, fill_c);
    SetTextColor(dc, text_c);
    let mut rr = *r;
    text_at(dc, label, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
}

unsafe fn todo_paint(hwnd: HWND) {
    let (dpi, items, sel, scroll, prev) = {
        let s = ST.lock().unwrap();
        let items: Vec<(String, Option<i64>, bool)> = s.todos.iter().map(|t| (t.text.clone(), t.due, t.done)).collect();
        let sel = s.todo_sel.and_then(|id| s.todos.iter().position(|t| t.id == id));
        (s.scale, items, sel, s.todo_scroll, s.todo_prev.clone())
    };
    let (now, ..) = local_now();
    let k = |v: i32| (v as f32 * dpi).round() as i32;
    let (w, h) = (k(TODO_W), k(TODO_H));
    let lay = layout(&k);
    let hover = HOVER.load(Relaxed);

    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, w, h);
    let old_bmp = SelectObject(dc, bmp);
    SetBkMode(dc, TRANSPARENT as i32);

    // frame, title bar
    let b = k(3);
    fill(dc, 0, 0, w, h, ORANGE);
    fill(dc, b, b, w - 2 * b, h - 2 * b, INK);
    fill(dc, b, b, w - 2 * b, lay.title_h - b, ORANGE);

    let f14 = CreateFontW(-k(14), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    let f12 = CreateFontW(-k(12), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
    let old_font = SelectObject(dc, f14);

    SetTextColor(dc, INK);
    let open = items.iter().filter(|i| !i.2).count();
    let mut tr = rc(k(14), 0, k(300), lay.title_h);
    text_at(dc, &format!("PIXEL TODO  -  {} OPEN", open), &mut tr, DT_SINGLELINE | DT_VCENTER);
    let c = &lay.close;
    fill(dc, c.left, c.top, c.right - c.left, c.bottom - c.top, if hover == 10 { CREAM } else { INK });
    let cp = k(2);
    let cross: Vec<&[u8]> = CROSS.to_vec();
    sprite(dc, &cross, c.left + (c.right - c.left - 7 * cp) / 2, c.top + (c.bottom - c.top - 7 * cp) / 2, cp,
        &|ch| (ch == b'X').then_some(if hover == 10 { INK } else { ORANGE }));

    // input frame (the real EDIT control sits inside it)
    let i = &lay.input;
    fill(dc, i.left, i.top, i.right - i.left, i.bottom - i.top, GREEN);
    fill(dc, i.left + k(2), i.top + k(2), i.right - i.left - k(4), i.bottom - i.top - k(4), PANEL);

    // preview of what the smart parser understood
    SelectObject(dc, f12);
    SetTextColor(dc, UP_BLUE);
    let mut pr = lay.prev;
    text_at(dc, &prev, &mut pr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);

    // list
    let l = &lay.list;
    fill(dc, l.left, l.top, l.right - l.left, l.bottom - l.top, PANEL_EDGE);
    fill(dc, l.left + k(2), l.top + k(2), l.right - l.left - k(4), l.bottom - l.top - k(4), PANEL);
    SelectObject(dc, f14);
    if items.is_empty() {
        SetTextColor(dc, DIM);
        let mut er = *l;
        text_at(dc, "NOTHING TO DO. ADD SOMETHING ABOVE!", &mut er, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
    }
    let rh = k(ROW_H);
    let end = items.len().min(scroll + ROWS_VISIBLE);
    for idx in scroll.min(items.len())..end {
        let (text, due, done) = &items[idx];
        let y = l.top + k(2) + (idx - scroll) as i32 * rh;
        if sel == Some(idx) {
            fill(dc, l.left + k(2), y, l.right - l.left - k(4), rh, PANEL_LIGHT);
        }
        let overdue = !*done && due.map_or(false, |d| d <= now);
        // pixel checkbox
        let bs = k(18);
        let bx = l.left + k(12);
        let by = y + (rh - bs) / 2;
        fill(dc, bx, by, bs, bs, if *done { GREEN } else { CREAM });
        fill(dc, bx + k(2), by + k(2), bs - k(4), bs - k(4), PANEL);
        if *done {
            let cp = k(2);
            let ck: Vec<&[u8]> = CHECK.to_vec();
            sprite(dc, &ck, bx + (bs - 6 * cp) / 2, by + (bs - 5 * cp) / 2, cp, &|ch| (ch == b'X').then_some(GREEN));
        }
        let tx = bx + bs + k(10);
        let due_w = if due.is_some() { k(128) } else { 0 };
        let right = l.right - k(16);
        let col = if *done { DIM } else if overdue { RED } else { CREAM };
        SetTextColor(dc, col);
        let mut r = RECT { left: tx, top: y, right: right - due_w, bottom: y + rh };
        text_at(dc, text, &mut r, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
        if *done {
            // strike-through
            let mut m = RECT { left: tx, top: y, right: right - due_w, bottom: y + rh };
            let t = wide(text);
            DrawTextW(dc, t.as_ptr(), -1, &mut m, DT_SINGLELINE | DT_CALCRECT | DT_NOPREFIX);
            let len = (m.right - m.left).min(right - due_w - tx);
            fill(dc, tx, y + rh / 2, len, k(2), DIM);
        }
        if let Some(d) = due {
            SetTextColor(dc, if *done { DIM } else if overdue { RED } else { GREEN });
            let mut dr = RECT { left: right - due_w, top: y, right, bottom: y + rh };
            text_at(dc, &todo::fmt_due(*d, now), &mut dr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
        }
    }
    if items.len() > ROWS_VISIBLE {
        let (tx, tw) = (l.right - k(10), k(6));
        let (ty, th) = (l.top + k(4), l.bottom - l.top - k(8));
        fill(dc, tx, ty, tw, th, PANEL_LIGHT);
        let thumb = (th * ROWS_VISIBLE as i32 / items.len() as i32).max(k(16));
        let span = (items.len() - ROWS_VISIBLE) as i32;
        let off = (th - thumb) * scroll as i32 / span.max(1);
        fill(dc, tx, ty + off, tw, thumb, ORANGE);
    }

    // buttons
    SelectObject(dc, f12);
    let labels = [("ADD", BTN_GREEN), ("DONE", PANEL_LIGHT), ("+1 HR", PANEL_LIGHT), ("DELETE", BTN_RED), ("CLEAR", PANEL_LIGHT)];
    for (n, (label, bg)) in labels.iter().enumerate() {
        pixel_button(dc, &lay.btns[n], label, *bg, hover == n as i32);
    }

    SetTextColor(dc, HINT);
    let mut hr = lay.hint;
    text_at(dc, "TRY: call mom tomorrow 5pm | gym fri 6:30pm | stretch in 45m.  Enter adds, click the box to tick, Esc closes.", &mut hr, DT_WORDBREAK);

    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old_font);
    DeleteObject(f14);
    DeleteObject(f12);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}

unsafe fn redraw_todo() {
    let w = hw(&TODO_WND);
    if !w.is_null() {
        InvalidateRect(w, null(), 0);
    }
}

/// Re-sort, remember list order for selection, and repaint.
unsafe fn refresh_list() {
    {
        let mut s = ST.lock().unwrap();
        todo::sort(&mut s.todos);
        s.view = s.todos.iter().map(|t| t.id).collect();
        let n = s.todos.len();
        s.todo_scroll = s.todo_scroll.min(n.saturating_sub(ROWS_VISIBLE));
        if let Some(id) = s.todo_sel {
            if !s.todos.iter().any(|t| t.id == id) {
                s.todo_sel = None;
            }
        }
    }
    redraw_todo();
}

unsafe fn update_preview() {
    let text = get_text(hw(&TODO_EDIT));
    let msg = if text.trim().is_empty() {
        String::new()
    } else {
        let (now, ..) = local_now();
        match todo::parse(&text, now) {
            (title, Some(d)) => format!("> {} | {}", todo::fmt_due(d, now), title),
            (title, None) => format!("> {} (no time set: try 'tomorrow 5pm' or 'in 30m')", title),
        }
    };
    ST.lock().unwrap().todo_prev = msg;
    redraw_todo();
}

unsafe fn add_from_edit() {
    let edit = hw(&TODO_EDIT);
    let text = get_text(edit);
    if text.trim().is_empty() {
        return;
    }
    let (now, ..) = local_now();
    let (title, due) = todo::parse(&text, now);
    {
        let mut s = ST.lock().unwrap();
        let id = s.next_id;
        s.next_id += 1;
        s.todos.push(Todo { id, text: title, due, done: false, fired: false });
        s.todo_sel = Some(id);
        save_todos(&s);
    }
    SetWindowTextW(edit, wide("").as_ptr());
    refresh_list();
    update_preview();
    InvalidateRect(hw(&PET_WND), null(), 0);
}

unsafe fn selected_id() -> Option<u32> {
    ST.lock().unwrap().todo_sel
}

/// op: 0 toggle done, 1 snooze +1h, 2 delete, 3 clear done
unsafe fn todo_op_on(op: u32, id: Option<u32>) {
    let (now, ..) = local_now();
    if op != 3 && id.is_none() {
        return;
    }
    {
        let mut s = ST.lock().unwrap();
        match (op, id) {
            (0, Some(id)) => {
                if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                    t.done = !t.done;
                }
            }
            (1, Some(id)) => {
                if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                    t.due = Some(now + 60);
                    t.fired = false;
                    t.done = false;
                }
            }
            (2, Some(id)) => s.todos.retain(|t| t.id != id),
            _ => s.todos.retain(|t| !t.done),
        }
        if let Some(a) = s.alert {
            if !s.todos.iter().any(|t| t.id == a && !t.done) {
                s.alert = None;
            }
        }
        save_todos(&s);
    }
    refresh_list();
    InvalidateRect(hw(&PET_WND), null(), 0);
}

unsafe fn todo_op(op: u32) {
    todo_op_on(op, selected_id());
}

unsafe fn show_todo() {
    let mut w = hw(&TODO_WND);
    if w.is_null() {
        w = create_todo_window();
    }
    refresh_list();
    update_preview();
    ShowWindow(w, SW_SHOW);
    SetForegroundWindow(w);
    SetFocus(hw(&TODO_EDIT));
}

unsafe fn toggle_todo() {
    let w = hw(&TODO_WND);
    if !w.is_null() && IsWindowVisible(w) != 0 {
        ShowWindow(w, SW_HIDE);
    } else {
        show_todo();
    }
}

unsafe fn todo_hit(x: i32, y: i32) -> i32 {
    let dpi = ST.lock().unwrap().scale;
    let k = |v: i32| (v as f32 * dpi).round() as i32;
    let lay = layout(&k);
    if contains(&lay.close, x, y) {
        return 10;
    }
    lay.btns.iter().position(|b| contains(b, x, y)).map_or(-1, |i| i as i32)
}

unsafe extern "system" fn todo_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_PAINT => {
            todo_paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        0x0133 /* WM_CTLCOLOREDIT */ => {
            let dc = wp as HDC;
            SetTextColor(dc, CREAM);
            SetBkColor(dc, PANEL);
            SetDCBrushColor(dc, PANEL);
            GetStockObject(DC_BRUSH) as LRESULT
        }
        WM_COMMAND => {
            if (wp >> 16) & 0xFFFF == 0x0300 {
                update_preview(); // EN_CHANGE
            }
            0
        }
        WM_NCHITTEST => {
            let mut p = POINT { x: (lp & 0xFFFF) as i16 as i32, y: ((lp >> 16) & 0xFFFF) as i16 as i32 };
            ScreenToClient(hwnd, &mut p);
            let dpi = ST.lock().unwrap().scale;
            let k = |v: i32| (v as f32 * dpi).round() as i32;
            let lay = layout(&k);
            if p.y < lay.title_h && !contains(&lay.close, p.x, p.y) {
                2 // HTCAPTION: drag by the title bar
            } else {
                1 // HTCLIENT
            }
        }
        WM_MOUSEMOVE => {
            let h = todo_hit(mx, my);
            if HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wp >> 16) & 0xFFFF) as i16;
            {
                let mut s = ST.lock().unwrap();
                let max = s.todos.len().saturating_sub(ROWS_VISIBLE);
                s.todo_scroll = if delta > 0 { s.todo_scroll.saturating_sub(1) } else { (s.todo_scroll + 1).min(max) };
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            SetFocus(hw(&TODO_EDIT));
            let hit = todo_hit(mx, my);
            match hit {
                10 => {
                    ShowWindow(hwnd, SW_HIDE);
                }
                0 => add_from_edit(),
                1..=4 => todo_op(hit as u32 - 1),
                _ => {
                    // a click inside the list selects the row; the checkbox ticks it
                    let dpi = ST.lock().unwrap().scale;
                    let k = |v: i32| (v as f32 * dpi).round() as i32;
                    let l = layout(&k).list;
                    if contains(&l, mx, my) {
                        let row = ((my - l.top - k(2)) / k(ROW_H)).max(0) as usize;
                        let (id, on_box) = {
                            let mut s = ST.lock().unwrap();
                            let idx = s.todo_scroll + row;
                            let id = s.todos.get(idx).map(|t| t.id);
                            if id.is_some() {
                                s.todo_sel = id;
                            }
                            (id, mx < l.left + k(12) + k(18) + k(8))
                        };
                        if on_box {
                            todo_op_on(0, id);
                        }
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
            }
            0
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ======================================================================
// pet window: mouse, menu, messages
// ======================================================================

unsafe fn handle_click(hwnd: HWND, x: i32, y: i32) {
    enum Act {
        Alert(u8),
        Pet,
        Fact,
        None,
    }
    let act = {
        let s = ST.lock().unwrap();
        if let Some((_, a)) = s.btns.iter().find(|(r, _)| contains(r, x, y)) {
            Act::Alert(*a)
        } else if contains(&s.pet_rect, x, y) {
            Act::Pet
        } else if s.fact_rect.map_or(false, |r| contains(&r, x, y)) {
            Act::Fact
        } else {
            Act::None
        }
    };
    match act {
        Act::Alert(a) => {
            let (now, ..) = local_now();
            {
                let mut s = ST.lock().unwrap();
                if let Some(id) = s.alert.take() {
                    if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                        match a {
                            0 => t.done = true,
                            1 => {
                                t.due = Some(now + 10);
                                t.fired = false;
                            }
                            _ => {} // later: stays in the list, marked overdue
                        }
                    }
                    save_todos(&s);
                }
            }
            refresh_list();
        }
        Act::Pet => toggle_todo(),
        Act::Fact => ST.lock().unwrap().fact_left = 0,
        Act::None => {}
    }
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn show_menu(hwnd: HWND) {
    let (cur, facts_on, show_up, bits, hide_fs, size, hidden, usage) = {
        let s = ST.lock().unwrap();
        let usage = (
            format!("Today:  \u{2193} {}   \u{2191} {}", format_bytes(s.day_rx), format_bytes(s.day_tx)),
            format!("Month:  \u{2193} {}   \u{2191} {}", format_bytes(s.mon_rx), format_bytes(s.mon_tx)),
        );
        (s.pet, s.facts_on, s.show_up, s.bits, s.hide_fs, s.size, s.user_hidden, usage)
    };
    let chk = |on: bool| if on { MF_STRING | MF_CHECKED } else { MF_STRING };
    let add = |m: HMENU, flags: u32, id: usize, text: &str| {
        AppendMenuW(m, flags, id, wide(text).as_ptr());
    };
    let menu = CreatePopupMenu();
    for (i, p) in PETS.iter().enumerate() {
        add(menu, chk(i == cur), 10 + i, p.name);
    }
    add(menu, MF_SEPARATOR, 0, "");
    add(menu, MF_STRING, 30, "Todo list...");
    add(menu, MF_STRING, 31, if hidden { "Show pet" } else { "Hide pet" });
    add(menu, MF_SEPARATOR, 0, "");
    add(menu, chk(facts_on), 20, "Show facts");
    add(menu, MF_STRING, 21, "Tell me a fact now");
    add(menu, chk(show_up), 22, "Show upload speed");
    add(menu, chk(bits), 23, "Speeds in bits (Mbps)");
    add(menu, MF_SEPARATOR, 0, "");
    let size_menu = CreatePopupMenu();
    for (i, n) in ["Small", "Medium", "Large"].iter().enumerate() {
        add(size_menu, chk(i == size), 40 + i, n);
    }
    add(menu, MF_POPUP, size_menu as usize, "Size");
    let pos_menu = CreatePopupMenu();
    for (i, n) in ["Bottom-right", "Bottom-left", "Top-right", "Top-left", "Move to next monitor"].iter().enumerate() {
        add(pos_menu, MF_STRING, 50 + i, n);
    }
    add(menu, MF_POPUP, pos_menu as usize, "Position (or drag the pet)");
    add(menu, chk(hide_fs), 24, "Hide in fullscreen apps");
    add(menu, chk(autostart_enabled()), 25, "Start with Windows");
    add(menu, MF_SEPARATOR, 0, "");
    add(menu, MF_STRING | MF_GRAYED, 0, &usage.0);
    add(menu, MF_STRING | MF_GRAYED, 0, &usage.1);
    add(menu, MF_SEPARATOR, 0, "");
    add(menu, MF_STRING, 1, "Quit Pixel Pet");

    let mut p: POINT = zeroed();
    GetCursorPos(&mut p);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, null()) as usize;
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);

    match cmd {
        1 => {
            DestroyWindow(hwnd);
        }
        10..=19 => {
            {
                let mut s = ST.lock().unwrap();
                s.pet = (cmd - 10).min(PETS.len() - 1);
                save_cfg(&s);
            }
            tray(hwnd, NIM_MODIFY);
        }
        20 => {
            let mut s = ST.lock().unwrap();
            s.facts_on = !s.facts_on;
            if !s.facts_on {
                s.fact_left = 0;
            }
            save_cfg(&s);
        }
        21 => {
            let mut s = ST.lock().unwrap();
            next_fact(&mut s);
            s.fact_left = FACT_FOR;
        }
        22 => {
            let mut s = ST.lock().unwrap();
            s.show_up = !s.show_up;
            save_cfg(&s);
        }
        23 => {
            let mut s = ST.lock().unwrap();
            s.bits = !s.bits;
            save_cfg(&s);
        }
        24 => {
            let mut s = ST.lock().unwrap();
            s.hide_fs = !s.hide_fs;
            save_cfg(&s);
        }
        25 => set_autostart(!autostart_enabled()),
        30 => show_todo(),
        31 => {
            let mut s = ST.lock().unwrap();
            s.user_hidden = !s.user_hidden;
        }
        40..=42 => set_size(hwnd, cmd - 40),
        50..=53 => {
            let mut wr: RECT = zeroed();
            GetWindowRect(hwnd, &mut wr);
            let wa = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
            let n = cmd - 50;
            put_in_corner(hwnd, n % 2 == 0, n < 2, wa);
        }
        54 => next_monitor(hwnd),
        _ => {}
    }
    InvalidateRect(hwnd, null(), 0);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_CREATE => {
            SetTimer(hwnd, 1, TICK_MS, None);
            0
        }
        WM_TIMER => {
            on_tick(hwnd);
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            place(hwnd);
            0
        }
        WM_LBUTTONDOWN => {
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let mut wr: RECT = zeroed();
            GetWindowRect(hwnd, &mut wr);
            SetCapture(hwnd);
            let mut s = ST.lock().unwrap();
            s.down = true;
            s.dragging = false;
            s.cur0 = (p.x, p.y);
            s.win0 = (wr.left, wr.top);
            0
        }
        WM_MOUSEMOVE => {
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let target = {
                let mut s = ST.lock().unwrap();
                if !s.down {
                    None
                } else {
                    let (dx, dy) = (p.x - s.cur0.0, p.y - s.cur0.1);
                    if !s.dragging && (dx.abs() > 4 || dy.abs() > 4) {
                        s.dragging = true;
                    }
                    if s.dragging {
                        Some((s.win0.0 + dx, s.win0.1 + dy, win_dims(eff_scale(&s))))
                    } else {
                        None
                    }
                }
            };
            if let Some((x, y, (w, h))) = target {
                let wa = work_at(p.x, p.y);
                let (x, y) = clamp_in(&wa, x, y, w, h);
                SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let (was_down, dragged) = {
                let mut s = ST.lock().unwrap();
                let r = (s.down, s.dragging);
                s.down = false;
                s.dragging = false;
                r
            };
            if was_down {
                if dragged {
                    let mut wr: RECT = zeroed();
                    GetWindowRect(hwnd, &mut wr);
                    let mut s = ST.lock().unwrap();
                    s.pos = Some((wr.left, wr.top));
                    save_cfg(&s);
                } else {
                    handle_click(hwnd, mx, my);
                }
            }
            0
        }
        WM_RBUTTONUP => {
            show_menu(hwnd);
            0
        }
        TRAY_MSG => {
            match (lp & 0xFFFF) as u32 {
                WM_LBUTTONUP => show_todo(),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            {
                let s = ST.lock().unwrap();
                save_usage(&s);
                save_todos(&s);
            }
            tray(hwnd, NIM_DELETE);
            PostQuitMessage(0);
            0
        }
        m if m == TB_CREATED.load(Relaxed) && m != 0 => {
            tray(hwnd, NIM_ADD); // explorer restarted
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn main() {
    unsafe {
        // one pet per user session
        let mutex = CreateMutexW(null(), 0, wide("Local\\PixelPetSingleton").as_ptr());
        if mutex.is_null() || GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }

        SetProcessDPIAware();
        let screen = GetDC(null_mut());
        let scale = GetDeviceCaps(screen, LOGPIXELSX as i32) as f32 / 96.0;
        ReleaseDC(null_mut(), screen);
        {
            let mut s = ST.lock().unwrap();
            s.scale = scale;
            load_cfg(&mut s);
            load_usage(&mut s);
            if let Some(p) = data_path("pixel-pet-todos.txt") {
                s.todos = todo::load(&p);
                s.next_id = s.todos.len() as u32 + 1;
            }
            (s.last_in, s.last_out) = totals();
            s.fact = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(0))
                % fact_count();
        }

        let hinst = GetModuleHandleW(null());
        let class = wide("PixelPetWnd");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let todo_class = wide("PixelTodoWnd");
        let tc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(todo_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpszMenuName: null(),
            lpszClassName: todo_class.as_ptr(),
        };
        RegisterClassW(&tc);
        TB_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Relaxed);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("Pixel Pet").as_ptr(),
            WS_POPUP,
            0, 0, 10, 10,
            null_mut(), null_mut(), hinst, null(),
        );
        PET_WND.store(hwnd as isize, Relaxed);
        SetLayeredWindowAttributes(hwnd, KEY, 0, LWA_COLORKEY);
        place(hwnd);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        tray(hwnd, NIM_ADD);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            // Enter in the todo box adds the item
            if msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN as usize && msg.hwnd == hw(&TODO_EDIT) {
                add_from_edit();
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == VK_ESCAPE as usize {
                let t = hw(&TODO_WND);
                if !t.is_null() && IsWindowVisible(t) != 0 && (msg.hwnd == t || msg.hwnd == hw(&TODO_EDIT)) {
                    ShowWindow(t, SW_HIDE);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(test)]
mod autostart_tests {
    use super::*;

    #[test]
    fn autostart_roundtrip() {
        unsafe {
            let before = autostart_enabled();
            set_autostart(true);
            assert!(autostart_enabled(), "value should exist after enabling");
            set_autostart(false);
            assert!(!autostart_enabled(), "value should be gone after disabling");
            if before {
                set_autostart(true);
            }
        }
    }
}
