//! Pixel Pet: a tiny always-on-top desktop pet that shows download speed
//! and the occasional cool fact. Plain Win32 + GDI, no runtime, no webview.
#![windows_subsystem = "windows"]

use std::ptr::{null, null_mut};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// ---- layout (logical px, scaled by DPI) ----
const WIN_W: i32 = 340;
const WIN_H: i32 = 230;
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

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

struct Pet {
    name: &'static str,
    face: u8, // fill colour char used when the eyes blink shut
    grid: &'static [&'static [u8]],
}

const PETS: [Pet; 4] = [
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

struct State {
    scale: f32,
    tick: u32,
    last_in: u32,
    speed: f64, // bytes per second
    fact: usize,
    fact_left: u32,
    pet: usize,
    facts_on: bool,
}

static ST: Mutex<State> = Mutex::new(State {
    scale: 1.0,
    tick: 0,
    last_in: 0,
    speed: 0.0,
    fact: 0,
    fact_left: 0,
    pet: 0,
    facts_on: true,
});

// ---- settings (tiny text file: pet index, facts on/off) ----
fn cfg_path() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join("pixel-pet.cfg"))
}

fn load_cfg(s: &mut State) {
    if let Some(txt) = cfg_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        let mut it = txt.lines();
        if let Some(i) = it.next().and_then(|l| l.trim().parse::<usize>().ok()) {
            if i < PETS.len() {
                s.pet = i;
            }
        }
        if let Some(l) = it.next() {
            s.facts_on = l.trim() != "0";
        }
    }
}

fn save_cfg(s: &State) {
    if let Some(p) = cfg_path() {
        let _ = std::fs::write(p, format!("{}\n{}\n", s.pet, s.facts_on as u8));
    }
}

fn next_fact(s: &mut State) {
    s.fact = (s.fact + 1 + (s.tick as usize * 7) % (fact_count() - 1)) % fact_count();
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Total received bytes across physical-ish adapters (wraps at 2^32).
unsafe fn total_in() -> u32 {
    let mut size = 0u32;
    GetIfTable(null_mut(), &mut size, 0);
    if size == 0 {
        return 0;
    }
    let mut buf = vec![0u32; size as usize / 4 + 1];
    let t = buf.as_mut_ptr() as *mut MIB_IFTABLE;
    if GetIfTable(t, &mut size, 0) != 0 {
        return 0;
    }
    let rows = (*t).table.as_ptr();
    let mut sum = 0u32;
    for i in 0..(*t).dwNumEntries as usize {
        let r = &*rows.add(i);
        // ethernet, wifi, ppp, mobile broadband; skips loopback/tunnels
        let kind = matches!(r.dwType, 6 | 71 | 23 | 243 | 244);
        if kind && (r.dwOperStatus == 4 || r.dwOperStatus == 5) {
            sum = sum.wrapping_add(r.dwInOctets);
        }
    }
    sum
}

fn format_speed(bps: f64) -> String {
    if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else {
        format!("{:.0} KB/s", bps / 1_000.0)
    }
}

unsafe fn place(hwnd: HWND, scale: f32) {
    let mut wa: RECT = std::mem::zeroed();
    SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut wa as *mut _ as *mut _, 0);
    let s = |v: i32| (v as f32 * scale).round() as i32;
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        wa.right - s(WIN_W + MARGIN),
        wa.bottom - s(WIN_H + MARGIN),
        s(WIN_W),
        s(WIN_H),
        SWP_NOACTIVATE,
    );
}

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

unsafe fn paint(hwnd: HWND) {
    let (scale, tick, speed, fact, pet) = {
        let s = ST.lock().unwrap();
        let f = if s.fact_left > 0 { Some(fact_text(s.fact)) } else { None };
        (s.scale, s.tick, s.speed, f, &PETS[s.pet])
    };
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let (w, h) = (u(WIN_W), u(WIN_H));

    let mut ps: PAINTSTRUCT = std::mem::zeroed();
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

    // --- cat (bobs every tick, blinks now and then) ---
    let px = u(PX);
    let (gh, gw) = (pet.grid.len(), pet.grid[0].len());
    let (cw, ch) = (gw as i32 * px, gh as i32 * px);
    let cx = w - cw - u(8);
    let bob = if tick % 2 == 0 { 0 } else { -px };
    let cy = h - ch - u(8) + bob;
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

    // --- speed bubble, left of the cat ---
    let txt = wide(&format_speed(speed));
    let (bw, bh, bd) = (u(124), u(30), u(3));
    let bx = cx - bw - u(10);
    let by = h - bh - u(10);
    fill(dc, bx, by, bw, bh, GREEN);
    fill(dc, bx + bd, by + bd, bw - 2 * bd, bh - 2 * bd, INK);
    let ap = u(3);
    sprite(dc, &ARROW, bx + u(10), by + (bh - 7 * ap) / 2, ap, &|c| (c == b'X').then_some(GREEN));
    SetTextColor(dc, GREEN);
    let mut tr = RECT { left: bx + u(30), top: by, right: bx + bw - bd, bottom: by + bh };
    DrawTextW(dc, txt.as_ptr(), -1, &mut tr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);

    // --- fact bubble, above the cat ---
    if let Some(f) = fact {
        let ft = wide(&f);
        let (pad, bd, sh) = (u(10), u(3), u(4));
        let fw = w - sh - u(4);
        let mut m = RECT { left: 0, top: 0, right: fw - 2 * (pad + bd), bottom: 0 };
        DrawTextW(dc, ft.as_ptr(), -1, &mut m, DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX);
        let fh = m.bottom + 2 * (pad + bd);
        let fx = u(2);
        let fy = (cy - u(12) - fh).max(u(2));
        fill(dc, fx + sh, fy + sh, fw, fh, INK);
        fill(dc, fx, fy, fw, fh, INK);
        fill(dc, fx + bd, fy + bd, fw - 2 * bd, fh - 2 * bd, CREAM);
        SetTextColor(dc, INK);
        let mut r = RECT { left: fx + bd + pad, top: fy + bd + pad, right: fx + fw - bd - pad, bottom: fy + fh };
        DrawTextW(dc, ft.as_ptr(), -1, &mut r, DT_WORDBREAK | DT_NOPREFIX);
    }

    BitBlt(screen, 0, 0, w, h, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old_font);
    DeleteObject(font);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}

unsafe fn on_tick(hwnd: HWND) {
    let now = total_in();
    {
        let mut s = ST.lock().unwrap();
        s.tick += 1;
        let delta = now.wrapping_sub(s.last_in) as f64;
        s.last_in = now;
        // ignore bogus jumps (adapter appeared/disappeared)
        if delta < 1.5e9 {
            let inst = delta * 1000.0 / TICK_MS as f64;
            s.speed = s.speed * 0.4 + inst * 0.6;
        }
        if s.speed < 500.0 {
            s.speed = 0.0;
        }
        if s.fact_left > 0 {
            s.fact_left -= 1;
        } else if s.facts_on && s.tick % FACT_EVERY == 10 {
            next_fact(&mut s);
            s.fact_left = FACT_FOR;
        }
        // keep above other topmost windows
        if s.tick % 20 == 0 {
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
    InvalidateRect(hwnd, null(), 0);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
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
            let scale = ST.lock().unwrap().scale;
            place(hwnd, scale);
            0
        }
        WM_RBUTTONUP => {
            // Right-click the pet: choose pet, toggle facts, quit.
            let (cur, facts_on) = {
                let s = ST.lock().unwrap();
                (s.pet, s.facts_on)
            };
            let menu = CreatePopupMenu();
            for (i, p) in PETS.iter().enumerate() {
                let flag = if i == cur { MF_STRING | MF_CHECKED } else { MF_STRING };
                AppendMenuW(menu, flag, 10 + i, wide(p.name).as_ptr());
            }
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            let flag = if facts_on { MF_STRING | MF_CHECKED } else { MF_STRING };
            AppendMenuW(menu, flag, 20, wide("Show facts").as_ptr());
            AppendMenuW(menu, MF_STRING, 21, wide("Tell me a fact now").as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            AppendMenuW(menu, MF_STRING, 1, wide("Quit Pixel Pet").as_ptr());
            let mut p: POINT = std::mem::zeroed();
            GetCursorPos(&mut p);
            SetForegroundWindow(hwnd);
            let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, hwnd, null());
            DestroyMenu(menu);
            match cmd {
                1 => {
                    DestroyWindow(hwnd);
                }
                10..=19 => {
                    let mut s = ST.lock().unwrap();
                    s.pet = ((cmd - 10) as usize).min(PETS.len() - 1);
                    save_cfg(&s);
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
                _ => {}
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_DESTROY => {
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
            s.last_in = total_in();
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

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("Pixel Pet").as_ptr(),
            WS_POPUP,
            0, 0, 10, 10,
            null_mut(), null_mut(), hinst, null(),
        );
        SetLayeredWindowAttributes(hwnd, KEY, 0, LWA_COLORKEY);
        place(hwnd, scale);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
