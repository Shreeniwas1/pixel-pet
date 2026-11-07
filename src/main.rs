//! Pixel Pet: a tiny always-on-top desktop pet that shows download speed
//! and the occasional cool fact, with a smart todo list. Plain Win32 + GDI, no runtime, no webview.
#![windows_subsystem = "windows"]

mod care;
mod mood;
mod stats;
mod todo;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use care::{Coach, Watch};
use mood::{Act as Pose, Ev, Mood};
use stats::Stats;
use todo::Todo;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetTickCount, GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::GetSystemTimes;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// ---- layout (logical px, scaled by DPI) ----
const WIN_W: i32 = 340;
const WIN_H: i32 = 250;
const FOCUS_H: i32 = 440; // taller window in focus mode (timer + tasks)
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

const HEART: [&[u8]; 5] = [b".X.X.", b"XXXXX", b"XXXXX", b".XXX.", b"..X.."];
const SPARK: [&[u8]; 3] = [b".X.", b"XXX", b".X."];
const DROP: [&[u8]; 3] = [b".X.", b"XXX", b".X."];
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
    // pet inner life
    mood: Mood,
    stats: Stats,
    coach: Coach,
    watch: Watch,
    care_on: bool,
    dnd_on: bool,     // hide + stay quiet while a fullscreen app/game is in front
    quiet: bool,      // currently in that state
    sys_on: bool,
    cpu_prev: (u64, u64), // (idle, total) cpu time
    cpu: u32,
    msg: String, // pet's own speech bubble
    msg_left: u32,
    say_n: u32,
    stroke: i32, // accumulated mouse travel over the pet (petting)
    last_mouse: Option<(i32, i32)>,
    pet: usize,
    facts_on: bool,
    show_up: bool,
    bits: bool,
    pos: Option<(i32, i32)>,
    // focus mode + pomodoro
    focus: bool,
    pomo_preset: usize,
    pomo_phase: u8, // 0 focus, 1 short break, 2 long break
    pomo_running: bool,
    pomo_end: Option<Instant>,
    pomo_left: u64, // seconds left while paused (u64::MAX = full length)
    pomo_done: u32,
    frects: Vec<(RECT, FAct)>,
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
    // data usage (bytes)
    day_key: u32,
    day_rx: u64,
    day_tx: u64,
    mon_key: u32,
    mon_rx: u64,
    mon_tx: u64,
}

#[derive(Clone, Copy)]
enum FAct {
    Toggle,
    Reset,
    Skip,
    Exit,
    Task(u32),
}

/// (focus, short break, long break) minutes
const POMO: [(u64, u64, u64); 3] = [(25, 5, 15), (50, 10, 20), (15, 3, 10)];
const POMO_NAMES: [&str; 3] = ["25 / 5 min", "50 / 10 min", "15 / 3 min"];

fn pomo_len(s: &State) -> u64 {
    let p = POMO[s.pomo_preset];
    60 * match s.pomo_phase {
        0 => p.0,
        1 => p.1,
        _ => p.2,
    }
}

fn pomo_remaining(s: &State) -> u64 {
    if s.pomo_running {
        s.pomo_end.map_or(0, |e| e.saturating_duration_since(Instant::now()).as_secs_f64().ceil() as u64)
    } else if s.pomo_left == u64::MAX {
        pomo_len(s)
    } else {
        s.pomo_left
    }
}

fn pomo_reset(s: &mut State) {
    s.pomo_running = false;
    s.pomo_end = None;
    s.pomo_left = u64::MAX;
}

fn pomo_toggle(s: &mut State) {
    if s.pomo_running {
        s.pomo_left = pomo_remaining(s);
        s.pomo_running = false;
        s.pomo_end = None;
    } else {
        let r = pomo_remaining(s);
        s.pomo_end = Some(Instant::now() + Duration::from_secs(r));
        s.pomo_running = true;
    }
}

/// Move to the next phase (paused, waiting for START). Only a finished focus round counts.
fn pomo_advance(s: &mut State, finished: bool) {
    if s.pomo_phase == 0 {
        if finished {
            s.pomo_done += 1;
        }
        s.pomo_phase = if finished && s.pomo_done % 4 == 0 { 2 } else { 1 };
    } else {
        s.pomo_phase = 0;
    }
    pomo_reset(s);
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
    mood: Mood::new(),
    stats: Stats::new(),
    coach: Coach::new(),
    watch: Watch::new(),
    care_on: true,
    dnd_on: true,
    quiet: false,
    sys_on: true,
    cpu_prev: (0, 0),
    cpu: 0,
    msg: String::new(),
    msg_left: 0,
    say_n: 0,
    stroke: 0,
    last_mouse: None,
    pet: 0,
    facts_on: true,
    show_up: true,
    bits: false,
    pos: None,
    focus: false,
    pomo_preset: 0,
    pomo_phase: 0,
    pomo_running: false,
    pomo_end: None,
    pomo_left: u64::MAX,
    pomo_done: 0,
    frects: Vec::new(),
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

fn hw(a: &AtomicIsize) -> HWND {
    a.load(Relaxed) as HWND
}

fn eff_scale(s: &State) -> f32 {
    s.scale * SIZES[s.size]
}

fn win_dims(scale: f32, focus: bool) -> (i32, i32) {
    let h = if focus { FOCUS_H } else { WIN_H };
    ((WIN_W as f32 * scale).round() as i32, (h as f32 * scale).round() as i32)
}

fn win_dims_s(s: &State) -> (i32, i32) {
    win_dims(eff_scale(s), s.focus)
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
            "focus" => s.focus = v == "1",
            "care" => s.care_on = v != "0",
            "dnd" => s.dnd_on = v != "0",
            "sys" => s.sys_on = v != "0",
            "goal" => {
                if let Ok(g) = v.parse::<u32>() {
                    s.stats.goal = g.clamp(1, 20);
                }
            }
            "happy" => {
                if let Ok(f) = v.parse::<f32>() {
                    s.mood.happy = f.clamp(0.0, 100.0);
                }
            }
            "energy" => {
                if let Ok(f) = v.parse::<f32>() {
                    s.mood.energy = f.clamp(0.0, 100.0);
                }
            }
            "lastrun" => {
                if let Ok(m) = v.parse::<i64>() {
                    s.mood.last_run = m;
                }
            }
            "pomo" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.pomo_preset = i.min(POMO.len() - 1);
                }
            }
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
            "pet={}\nfacts={}\nup={}\nbits={}\nsize={}\nfocus={}\npomo={}\nhappy={:.1}\nenergy={:.1}\nlastrun={}\ncare={}\nsys={}\ngoal={}\ndnd={}\n",
            s.pet,
            s.facts_on as u8,
            s.show_up as u8,
            s.bits as u8,
            s.size,
            s.focus as u8,
            s.pomo_preset,
            s.mood.happy,
            s.mood.energy,
            s.mood.last_run,
            s.care_on as u8,
            s.sys_on as u8,
            s.stats.goal,
            s.dnd_on as u8
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

/// The pet says something in its own words (picked by species and mood event).
fn say(s: &mut State, ev: Ev) {
    s.say_n += 1;
    s.msg = mood::line(s.pet, ev, s.say_n).to_string();
    s.msg_left = 14;
    s.fact_left = 0;
}

fn save_stats(s: &State) {
    if let Some(p) = data_path("pixel-pet-focus.txt") {
        let _ = std::fs::write(p, s.stats.to_text());
    }
}

/// A plain-text message from the pet (reminders, alerts).
fn say_text(s: &mut State, text: &str, ticks: u32) {
    s.msg = text.to_string();
    s.msg_left = ticks;
    s.fact_left = 0;
}

/// Overall CPU load in percent since the previous call, or None on the first call.
unsafe fn cpu_percent(prev: &mut (u64, u64)) -> Option<u32> {
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

unsafe fn ram_percent() -> u32 {
    let mut m: MEMORYSTATUSEX = zeroed();
    m.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    if GlobalMemoryStatusEx(&mut m) == 0 {
        return 0;
    }
    m.dwMemoryLoad
}

/// (percent, plugged in) or None on a machine without a battery.
unsafe fn battery() -> Option<(u32, bool)> {
    let mut p: SYSTEM_POWER_STATUS = zeroed();
    if GetSystemPowerStatus(&mut p) == 0 || p.BatteryFlag & 128 != 0 || p.BatteryLifePercent > 100 {
        return None;
    }
    Some((p.BatteryLifePercent as u32, p.ACLineStatus == 1))
}

/// Is a fullscreen app (game, video, slideshow) in front on any monitor?
unsafe fn fullscreen_active(own: HWND) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_null() || fg == own || fg == hw(&TODO_WND) {
        return false;
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(fg, cls.as_mut_ptr(), 64).max(0) as usize;
    let name = String::from_utf16_lossy(&cls[..n]);
    if matches!(name.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return false; // the desktop and the taskbar cover the whole screen too
    }
    let mut r: RECT = zeroed();
    if GetWindowRect(fg, &mut r) == 0 {
        return false;
    }
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST), &mut mi);
    let m = mi.rcMonitor;
    r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
}

/// Seconds since the user last touched the mouse or keyboard.
unsafe fn idle_secs() -> u64 {
    let mut li = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if GetLastInputInfo(&mut li) == 0 {
        return 0;
    }
    (GetTickCount().wrapping_sub(li.dwTime) / 1000) as u64
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
    let (scale, pos, focus) = {
        let s = ST.lock().unwrap();
        (eff_scale(&s), s.pos, s.focus)
    };
    let (w, h) = win_dims(scale, focus);
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
        let (w, h) = win_dims_s(&s);
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

/// Change a setting that alters the window size, keeping the pet's corner where it is.
unsafe fn reanchor(hwnd: HWND, change: impl FnOnce(&mut State)) {
    let (fx, fy) = flips(hwnd);
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    {
        let mut s = ST.lock().unwrap();
        change(&mut s);
        let (w, h) = win_dims_s(&s);
        let x = if fx { wr.left } else { wr.right - w };
        let y = if fy { wr.top } else { wr.bottom - h };
        s.pos = Some((x, y));
        save_cfg(&s);
    }
    place(hwnd);
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn set_size(hwnd: HWND, idx: usize) {
    reanchor(hwnd, |s| s.size = idx);
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
const TOMATO: u32 = rgb(0xff, 0x6b, 0x6b);

unsafe fn paint(hwnd: HWND) {
    let (scale, tick, speed, up, show_up, bits, fact, alert, pet, pose) = {
        let s = ST.lock().unwrap();
        let alert = s.alert.and_then(|id| s.todos.iter().find(|t| t.id == id)).map(|t| t.text.clone());
        let fact = if alert.is_some() {
            None
        } else if s.msg_left > 0 {
            Some(s.msg.clone())
        } else if s.fact_left > 0 && !s.focus {
            Some(fact_text(s.fact))
        } else {
            None
        };
        (eff_scale(&s), s.tick, s.speed, s.up, s.show_up, s.bits, fact, alert, &PETS[s.pet], s.mood.act())
    };
    struct FocusSnap {
        focus: bool,
        tasks: Vec<(u32, String, Option<i64>)>,
        more: usize,
        phase: u8,
        remain: u64,
        total: u64,
        running: bool,
        done: u32,
        goal: u32,
    }
    let fs = {
        let s = ST.lock().unwrap();
        let open: Vec<&Todo> = s.todos.iter().filter(|t| !t.done).collect();
        FocusSnap {
            focus: s.focus,
            tasks: open.iter().take(5).map(|t| (t.id, t.text.clone(), t.due)).collect(),
            more: open.len().saturating_sub(5),
            phase: s.pomo_phase,
            remain: pomo_remaining(&s),
            total: pomo_len(&s),
            running: s.pomo_running,
            done: s.stats.today((local_now().0 / 1440) as u32).0,
            goal: s.stats.goal,
        }
    };
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let (w, h) = win_dims(scale, fs.focus);
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
    // body language: how the pet moves depends on what it is doing
    let bob = match pose {
        Pose::Idle if fs.focus => 0, // holds still while you work
        Pose::Idle => if tick % 2 == 0 { 0 } else { -px },
        Pose::Sleep => if (tick / 3) % 2 == 0 { 0 } else { -px / 3 }, // slow breathing
        Pose::Cheer => if tick % 2 == 0 { -2 * px } else { 0 },       // jumping for joy
        Pose::Love => if tick % 2 == 0 { -px / 2 } else { 0 },
        Pose::Stretch => -px,
        Pose::Tired | Pose::Sad => px / 3, // slumped
    };
    let cy = base_y + bob;
    let face_c = pet_color(pet.face).unwrap_or(CREAM);
    let closed = matches!(pose, Pose::Sleep | Pose::Cheer | Pose::Love | Pose::Stretch);
    let droopy = matches!(pose, Pose::Tired | Pose::Sad);
    let blink = tick % 16 == 0 && !fs.focus && pose == Pose::Idle;
    let mut grid: Vec<Vec<u8>> = pet.grid.iter().map(|r| r.to_vec()).collect();
    // eyes follow the mouse while the pet is awake
    if pose == Pose::Idle || droopy {
        let mut cp: POINT = zeroed();
        GetCursorPos(&mut cp);
        let mut wr: RECT = zeroed();
        GetWindowRect(hwnd, &mut wr);
        let dx = cp.x - (wr.left + cx + cw / 2);
        let dir: i32 = if dx > u(120) { 1 } else if dx < -u(120) { -1 } else { 0 };
        if dir != 0 {
            let cols: Vec<usize> = if dir > 0 { (0..gw).rev().collect() } else { (0..gw).collect() };
            for j in 0..gh {
                for &i in &cols {
                    let t = i as i32 + dir;
                    if grid[j][i] == b'E' && t >= 0 && (t as usize) < gw && grid[j][t as usize] == pet.face {
                        grid[j][i] = pet.face;
                        grid[j][t as usize] = b'E';
                    }
                }
            }
        }
    }
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
    // eyelids: closed eyes are a thin line, tired eyes get a heavy upper lid
    for j in 0..gh {
        for i in 0..gw {
            if grid[j][i] != b'E' {
                continue;
            }
            let (ex, ey) = (cx + i as i32 * px, cy + j as i32 * px);
            if closed {
                fill(dc, ex, ey, px, px, face_c);
                fill(dc, ex, ey + px * 2 / 3, px, (px / 3).max(1), INK);
            } else if droopy {
                fill(dc, ex, ey, px, px / 2, face_c);
            }
        }
    }
    // little extras that show the mood: z's, hearts, sparkles, a tear
    {
        let ap = u(3).max(2);
        let off = u(2) + if tick % 4 < 2 { 0 } else { u(4) };
        let ox = cx + cw / 2;
        let oy = if flipy { cy + ch + u(4) + off } else { cy - u(4) - off };
        let up_s = |h: i32| if flipy { oy } else { oy - h };
        match pose {
            Pose::Sleep => {
                SetTextColor(dc, UP_BLUE);
                for (k, z) in ["z", "Z"].iter().enumerate() {
                    let zy = if flipy { cy + ch + u(4) + off / 2 + k as i32 * u(10) } else { cy - u(18) - off / 2 - k as i32 * u(10) };
                    let mut zr = RECT { left: ox + u(14) + k as i32 * u(12), top: zy, right: ox + u(40) + k as i32 * u(12), bottom: zy + u(16) };
                    text_at(dc, z, &mut zr, DT_SINGLELINE);
                }
            }
            Pose::Love => {
                let h = 5 * ap;
                for k in 0..2 {
                    let hx = ox - u(14) + k * u(22);
                    sprite(dc, &HEART, hx, up_s(h) - k * u(6), ap, &|c| (c == b'X').then_some(PINK));
                }
            }
            Pose::Cheer => {
                let col = rgb(0xff, 0xe0, 0x66);
                for k in 0..3 {
                    let sx = cx + k * (cw / 2) - u(2);
                    let sy = if tick % 2 == 0 { up_s(3 * ap) } else { up_s(3 * ap) - u(5) };
                    let sy = sy + if k == 1 { -u(6) } else { 0 };
                    sprite(dc, &SPARK, sx, sy, ap, &|c| (c == b'X').then_some(col));
                }
            }
            Pose::Sad | Pose::Tired => {
                let drop_c = if pose == Pose::Sad { UP_BLUE } else { rgb(0xb8, 0xe6, 0xff) };
                let dy = (tick % 4) as i32 * u(3);
                let dxp = if pose == Pose::Sad { cx + cw / 4 } else { cx + cw - u(10) };
                let base = if pose == Pose::Sad { cy + 5 * px } else { cy + px };
                sprite(dc, &DROP, dxp, base + dy, ap, &|c| (c == b'X').then_some(drop_c));
            }
            _ => {}
        }
    }
    let pet_rect = RECT { left: cx, top: cy, right: cx + cw, bottom: cy + ch };

    // --- speed bubble beside the pet (download, plus upload if enabled); hidden in focus mode ---
    if !fs.focus {
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
    }

    // --- focus mode: pomodoro timer card + open tasks card, stacked next to the pet ---
    let mut frects: Vec<(RECT, FAct)> = Vec::new();
    let mut focus_top: Option<i32> = None; // where the tasks card starts (reminders cover it, not the timer)
    if fs.focus {
        let (fpad, fb) = (u(8), u(3));
        let (x0, pw) = (u(2), w - u(2) - u(6));
        let gap = u(8);
        let tc_h = u(118);
        let n_rows = fs.tasks.len().max(1) as i32;
        let tk_h = 2 * (fb + fpad) + u(22) + n_rows * u(26) + if fs.more > 0 { u(18) } else { 0 };
        let (tc_y, tk_y) = if flipy {
            let t = base_y + ch + u(12);
            (t, t + tc_h + gap)
        } else {
            let t = base_y - u(12) - tc_h;
            (t, t - gap - tk_h)
        };
        focus_top = Some(tk_y);
        let iw = pw - 2 * (fb + fpad);
        let ix = x0 + fb + fpad;
        let accent = if fs.phase == 0 { TOMATO } else { GREEN };

        // timer card
        fill(dc, x0, tc_y, pw, tc_h, accent);
        fill(dc, x0 + fb, tc_y + fb, pw - 2 * fb, tc_h - 2 * fb, INK);
        let y0 = tc_y + fb + fpad;
        let name = ["FOCUS", "SHORT BREAK", "LONG BREAK"][fs.phase as usize];
        SetTextColor(dc, accent);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(16) };
        text_at(dc, name, &mut r, DT_SINGLELINE);
        SetTextColor(dc, HINT);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(16) };
        text_at(dc, &format!("{}/{} TODAY", fs.done, fs.goal), &mut r, DT_SINGLELINE | DT_RIGHT);
        let big = CreateFontW(-u(30), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
        SelectObject(dc, big);
        SetTextColor(dc, CREAM);
        let mut r = RECT { left: ix, top: y0 + u(16), right: ix + iw, bottom: y0 + u(52) };
        text_at(dc, &format!("{:02}:{:02}", fs.remain / 60, fs.remain % 60), &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
        SelectObject(dc, font);
        DeleteObject(big);
        let (segs, sg) = (20i32, u(2));
        let sw = (iw - (segs - 1) * sg) / segs;
        let filled = if fs.total == 0 { 0 } else { (fs.total.saturating_sub(fs.remain) * segs as u64 / fs.total) as i32 };
        for i in 0..segs {
            fill(dc, ix + i * (sw + sg), y0 + u(56), sw, u(10), if i < filled { accent } else { PANEL_LIGHT });
        }
        let (by, bh, g) = (y0 + u(70), u(24), u(6));
        let bw = (iw - 3 * g) / 4;
        let buttons = [
            (if fs.running { "PAUSE" } else { "START" }, FAct::Toggle, BTN_GREEN),
            ("RESET", FAct::Reset, PANEL_LIGHT),
            ("SKIP", FAct::Skip, PANEL_LIGHT),
            ("EXIT", FAct::Exit, BTN_RED),
        ];
        for (i, (label, act, bg)) in buttons.into_iter().enumerate() {
            let bx = ix + i as i32 * (bw + g);
            fill(dc, bx, by, bw, bh, PANEL_EDGE);
            fill(dc, bx + u(2), by + u(2), bw - u(4), bh - u(4), bg);
            SetTextColor(dc, CREAM);
            let mut rr = RECT { left: bx, top: by, right: bx + bw, bottom: by + bh };
            text_at(dc, label, &mut rr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
            frects.push((RECT { left: bx, top: by, right: bx + bw, bottom: by + bh }, act));
        }

        // tasks card
        fill(dc, x0, tk_y, pw, tk_h, UP_BLUE);
        fill(dc, x0 + fb, tk_y + fb, pw - 2 * fb, tk_h - 2 * fb, INK);
        let y0 = tk_y + fb + fpad;
        SetTextColor(dc, UP_BLUE);
        let mut r = RECT { left: ix, top: y0, right: ix + iw, bottom: y0 + u(20) };
        text_at(dc, &format!("TASKS ({})", fs.tasks.len() + fs.more), &mut r, DT_SINGLELINE | DT_VCENTER);
        let (now, ..) = local_now();
        if fs.tasks.is_empty() {
            SetTextColor(dc, DIM);
            let mut r = RECT { left: ix, top: y0 + u(22), right: ix + iw, bottom: y0 + u(48) };
            text_at(dc, "NO OPEN TASKS. ADD ONE!", &mut r, DT_SINGLELINE | DT_VCENTER);
        }
        for (i, (id, text, due)) in fs.tasks.iter().enumerate() {
            let ry = y0 + u(22) + i as i32 * u(26);
            let bs = u(16);
            let cby = ry + (u(26) - bs) / 2;
            fill(dc, ix, cby, bs, bs, CREAM);
            fill(dc, ix + u(2), cby + u(2), bs - u(4), bs - u(4), PANEL);
            frects.push((RECT { left: ix - u(2), top: ry, right: ix + bs + u(6), bottom: ry + u(26) }, FAct::Task(*id)));
            let overdue = due.map_or(false, |d| d <= now);
            let due_w = if due.is_some() { u(118) } else { 0 };
            SetTextColor(dc, if overdue { RED } else { CREAM });
            let mut tr = RECT { left: ix + bs + u(8), top: ry, right: ix + iw - due_w, bottom: ry + u(26) };
            text_at(dc, text, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
            if let Some(d) = due {
                SetTextColor(dc, if overdue { RED } else { GREEN });
                let mut dr = RECT { left: ix + iw - due_w, top: ry, right: ix + iw, bottom: ry + u(26) };
                text_at(dc, &todo::fmt_due(*d, now), &mut dr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            }
        }
        if fs.more > 0 {
            SetTextColor(dc, DIM);
            let ry = y0 + u(22) + n_rows * u(26);
            let mut r = RECT { left: ix, top: ry, right: ix + iw, bottom: ry + u(18) };
            text_at(dc, &format!("+{} MORE (CLICK THE PET)", fs.more), &mut r, DT_SINGLELINE | DT_VCENTER);
        }
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
        let fy = focus_top.unwrap_or_else(|| anchor_y(fh));
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
        s.frects = frects;
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
    let idle = idle_secs();
    let (ram, bat) = (ram_percent(), battery());
    let mut cpu_prev = ST.lock().unwrap().cpu_prev;
    let cpu_now = cpu_percent(&mut cpu_prev);
    let mut beep = false;
    let keep_top;
    let dnd_on = ST.lock().unwrap().dnd_on;
    let full = dnd_on && fullscreen_active(hwnd); // checked every tick: it is cheap
    let was_quiet;
    {
        let mut s = ST.lock().unwrap();
        was_quiet = s.quiet;
        s.quiet = full;
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

        // the pet's inner life: mood, sleep, and speech
        s.msg_left = s.msg_left.saturating_sub(1);
        let focusing = s.focus && s.pomo_running;
        if let Some(ev) = s.mood.tick(idle, ((now / 60) % 24) as u32, focusing) {
            if !full {
                say(&mut s, ev);
            }
        }
        s.mood.last_run = now;
        s.cpu_prev = cpu_prev;
        if let Some(c) = cpu_now {
            s.cpu = c;
        }

        // break coach (never interrupts a running focus round) and system alerts
        let deep_focus = s.focus && s.pomo_running && s.pomo_phase == 0;
        let care = s.coach.tick(idle, deep_focus);
        let cpu_now_pct = s.cpu;
        let alert_sys = s.watch.tick(cpu_now_pct, ram, bat);
        if s.alert.is_none() && !full {
            if let (Some(c), true) = (care, s.care_on) {
                say_text(&mut s, care::care_text(c), 24);
            } else if let (Some(a), true) = (alert_sys, s.sys_on) {
                say_text(&mut s, &care::sys_text(a), 24);
            }
        }

        // facts
        if s.fact_left > 0 {
            s.fact_left -= 1;
        } else if s.facts_on && !full && !s.focus && s.alert.is_none() && s.msg_left == 0 && s.tick % FACT_EVERY == 10 {
            next_fact(&mut s);
            s.fact_left = FACT_FOR;
        }

        // pomodoro finished?
        if s.pomo_running && pomo_remaining(&s) == 0 {
            let was_focus = s.pomo_phase == 0;
            pomo_advance(&mut s, true);
            if was_focus {
                let mins = (pomo_len(&s) / 60) as u32;
                let ev = s.mood.on_focus_done();
                say(&mut s, ev);
                if s.stats.record((now / 1440) as u32, mins) {
                    let g = s.stats.goal;
                    say_text(&mut s, &format!("Daily goal reached: {g} focus sessions! Amazing."), 24);
                }
                save_stats(&s);
            }
            beep = true;
        }

        // reminders: first overdue, not-yet-shown item
        if s.alert.is_none() && !full {
            if let Some(t) = s.todos.iter_mut().find(|t| !t.done && !t.fired && t.due.map_or(false, |d| d <= now)) {
                t.fired = true;
                let id = t.id;
                s.alert = Some(id);
                s.fact_left = 0;
                beep = true;
            }
        }

        keep_top = s.tick % 20 == 0;
        if s.tick % 120 == 0 {
            save_cfg(&s);
            save_usage(&s);
        }
    }
    if full != was_quiet {
        // slip away while a game or video is fullscreen, come back when it ends
        ShowWindow(hwnd, if full { SW_HIDE } else { SW_SHOWNOACTIVATE });
    }
    if beep && !full {
        MessageBeep(MB_ICONASTERISK);
        refresh_list();
    }
    if keep_top && !full {
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
        match todo::parse_full(&text, now) {
            (title, Some(d), _) => format!("> {} | {}", todo::fmt_due(d, now), title),
            (title, None, _) => format!("> {} (no time set: try 'tomorrow 5pm', 'in 30m' or 'every monday 9am')", title),
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
    let (title, due, repeat) = todo::parse_full(&text, now);
    {
        let mut s = ST.lock().unwrap();
        let id = s.next_id;
        s.next_id += 1;
        s.todos.push(Todo { id, text: title, due, done: false, fired: false, repeat });
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
                let mut completed = false;
                if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                    if t.done {
                        t.done = false; // un-complete
                    } else {
                        completed = todo::complete(t, now);
                    }
                }
                if completed {
                    let ev = s.mood.on_task_done();
                    say(&mut s, ev);
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
        Focus(FAct),
        Alert(u8),
        Pet,
        Fact,
        None,
    }
    let act = {
        let s = ST.lock().unwrap();
        if let Some((_, a)) = s.frects.iter().find(|(r, _)| contains(r, x, y)) {
            Act::Focus(*a)
        } else if let Some((_, a)) = s.btns.iter().find(|(r, _)| contains(r, x, y)) {
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
        Act::Focus(a) => match a {
            FAct::Toggle => pomo_toggle(&mut ST.lock().unwrap()),
            FAct::Reset => pomo_reset(&mut ST.lock().unwrap()),
            FAct::Skip => pomo_advance(&mut ST.lock().unwrap(), false),
            FAct::Exit => reanchor(hwnd, |s| s.focus = false),
            FAct::Task(id) => {
                {
                    let mut s = ST.lock().unwrap();
                    let (now, ..) = local_now();
                    if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                        todo::complete(t, now);
                    }
                    if s.alert == Some(id) {
                        s.alert = None;
                    }
                    let ev = s.mood.on_task_done();
                    say(&mut s, ev);
                    save_todos(&s);
                }
                refresh_list();
            }
        },
        Act::Alert(a) => {
            let (now, ..) = local_now();
            {
                let mut s = ST.lock().unwrap();
                if let Some(id) = s.alert.take() {
                    if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                        match a {
                            0 => {
                                todo::complete(t, now);
                                let ev = s.mood.on_task_done();
                                say(&mut s, ev);
                            }
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
        Act::Fact => {
            let mut s = ST.lock().unwrap();
            s.fact_left = 0;
            s.msg_left = 0;
        }
        Act::None => {}
    }
    InvalidateRect(hwnd, null(), 0);
}

unsafe fn show_menu(hwnd: HWND) {
    let (cur, facts_on, show_up, bits, size, usage, mood_line, goal, care_on, sys_on, focus_line, dnd_on) = {
        let s = ST.lock().unwrap();
        let day = (local_now().0 / 1440) as u32;
        let (n, m) = s.stats.today(day);
        let focus_line = format!("Focus today: {n}/{} sessions, {m} min   Streak: {} day(s)", s.stats.goal, s.stats.streak(day));
        let mood_line = format!(
            "Mood: {} ({:.0}%)   Energy: {:.0}%",
            s.mood.label(),
            s.mood.happy,
            s.mood.energy
        );
        let usage = (
            format!("Today:  \u{2193} {}   \u{2191} {}", format_bytes(s.day_rx), format_bytes(s.day_tx)),
            format!("Month:  \u{2193} {}   \u{2191} {}", format_bytes(s.mon_rx), format_bytes(s.mon_tx)),
        );
        (s.pet, s.facts_on, s.show_up, s.bits, s.size, usage, mood_line, s.stats.goal, s.care_on, s.sys_on, focus_line, s.dnd_on)
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
    let (focus_on, preset) = {
        let s = ST.lock().unwrap();
        (s.focus, s.pomo_preset)
    };
    add(menu, chk(focus_on), 32, "Focus mode (tasks + pomodoro)");
    let pomo_menu = CreatePopupMenu();
    for (i, n) in POMO_NAMES.iter().enumerate() {
        add(pomo_menu, chk(i == preset), 60 + i, n);
    }
    add(menu, MF_POPUP, pomo_menu as usize, "Pomodoro length");
    add(menu, MF_SEPARATOR, 0, "");
    add(menu, MF_STRING, 33, "Weekly focus recap...");
    let goal_menu = CreatePopupMenu();
    for (i, g) in [2u32, 4, 6, 8].iter().enumerate() {
        add(goal_menu, chk(*g == goal), 70 + i, &format!("{g} sessions"));
    }
    add(menu, MF_POPUP, goal_menu as usize, "Daily focus goal");
    add(menu, chk(care_on), 34, "Break coach (eyes, water, stretch)");
    add(menu, chk(sys_on), 35, "System alerts (CPU, RAM, battery)");
    add(menu, chk(dnd_on), 36, "Hide during fullscreen apps/games");
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
    add(menu, MF_SEPARATOR, 0, "");
    add(menu, MF_STRING | MF_GRAYED, 0, &mood_line);
    add(menu, MF_STRING | MF_GRAYED, 0, &focus_line);
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
        30 => show_todo(),
        32 => reanchor(hwnd, |s| s.focus = !s.focus),
        33 => {
            let text = {
                let s = ST.lock().unwrap();
                s.stats.recap((local_now().0 / 1440) as u32)
            };
            MessageBoxW(hwnd, wide(&text).as_ptr(), wide("Pixel Pet - weekly recap").as_ptr(), MB_OK | MB_ICONINFORMATION);
        }
        34 => {
            let mut s = ST.lock().unwrap();
            s.care_on = !s.care_on;
            save_cfg(&s);
        }
        35 => {
            let mut s = ST.lock().unwrap();
            s.sys_on = !s.sys_on;
            save_cfg(&s);
        }
        36 => {
            let mut s = ST.lock().unwrap();
            s.dnd_on = !s.dnd_on;
            save_cfg(&s);
        }
        70..=73 => {
            let mut s = ST.lock().unwrap();
            s.stats.goal = [2, 4, 6, 8][cmd - 70];
            save_cfg(&s);
        }
        60..=62 => {
            let mut s = ST.lock().unwrap();
            s.pomo_preset = (cmd - 60).min(POMO.len() - 1);
            pomo_reset(&mut s);
            save_cfg(&s);
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
        WM_HOTKEY => {
            match wp {
                1 => show_todo(),
                2 => reanchor(hwnd, |s| s.focus = !s.focus),
                _ => {}
            }
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
                    // stroking the pet with the mouse pets it
                    if contains(&s.pet_rect, mx, my) {
                        if let Some((lx, ly)) = s.last_mouse {
                            s.stroke += (mx - lx).abs() + (my - ly).abs();
                        }
                        s.last_mouse = Some((mx, my));
                        if s.stroke > (220.0 * s.scale) as i32 {
                            s.stroke = 0;
                            if let Some(ev) = s.mood.on_petted() {
                                say(&mut s, ev);
                            }
                        }
                    } else {
                        s.last_mouse = None;
                        s.stroke = 0;
                    }
                    None
                } else {
                    let (dx, dy) = (p.x - s.cur0.0, p.y - s.cur0.1);
                    if !s.dragging && (dx.abs() > 4 || dy.abs() > 4) {
                        s.dragging = true;
                    }
                    if s.dragging {
                        Some((s.win0.0 + dx, s.win0.1 + dy, win_dims_s(&s)))
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
        WM_DESTROY => {
            {
                let s = ST.lock().unwrap();
                save_cfg(&s);
                save_usage(&s);
                save_todos(&s);
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn main() {
    unsafe {
        SetProcessDPIAware();
        let screen = GetDC(null_mut());
        let scale = GetDeviceCaps(screen, LOGPIXELSX as i32) as f32 / 96.0;
        ReleaseDC(null_mut(), screen);
        {
            let mut s = ST.lock().unwrap();
            s.scale = scale;
            load_cfg(&mut s);
            load_usage(&mut s);
            if let Some(t) = data_path("pixel-pet-focus.txt").and_then(|p| std::fs::read_to_string(p).ok()) {
                s.stats.load_text(&t);
            }
            if let Some(p) = data_path("pixel-pet-todos.txt") {
                s.todos = todo::load(&p);
                todo::sort(&mut s.todos);
                s.next_id = s.todos.len() as u32 + 1;
            }
            (s.last_in, s.last_out) = totals();
            let (now, ..) = local_now();
            if let Some(ev) = s.mood.on_launch(now) {
                say(&mut s, ev);
            }
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
        // Ctrl+Alt+T: quick-add a task, Ctrl+Alt+F: toggle focus mode (silently skipped if taken)
        RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'T' as u32);
        RegisterHotKey(hwnd, 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'F' as u32);

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
