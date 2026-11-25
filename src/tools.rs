//! Quick tools opened from the hub: a calculator window, a notes window, and VS Code.

use crate::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use windows_sys::Win32::UI::Shell::ShellExecuteW;

pub(crate) static CALC_WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static CALC_EDIT: AtomicIsize = AtomicIsize::new(0);
pub(crate) static NOTE_WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static NOTE_EDIT: AtomicIsize = AtomicIsize::new(0);

static CALC_HOVER: AtomicI32 = AtomicI32::new(-1);
static NOTE_HOVER: AtomicI32 = AtomicI32::new(-1);
static NOTE_PAGE: AtomicUsize = AtomicUsize::new(0);
static NOTE_DIRTY: AtomicBool = AtomicBool::new(false);
/// Earlier sums, newest last.
static TAPE: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

const CLOSE: i32 = 100;
const CALC_W: i32 = 272;
const CALC_H: i32 = 404;
const NOTE_W: i32 = 400;
const NOTE_H: i32 = 380;
const NOTE_PAGES: usize = 5;
const EN_CHANGE: usize = 0x0300;
const EM_SETSEL: u32 = 0x00B1;
const EM_REPLACESEL: u32 = 0x00C2;
const EM_SETLIMITTEXT: u32 = 0x00C5;

pub(crate) const KEYS: [&str; 20] = ["C", "(", ")", "DEL", "7", "8", "9", "/", "4", "5", "6", "*", "1", "2", "3", "-", "0", ".", "%", "+"];
const EQUALS: i32 = 20;
const COPY: i32 = 21;

fn kk(v: i32) -> i32 {
    (v as f32 * ST.lock().unwrap().ui_scale).round() as i32
}

fn close_rect(w: i32) -> RECT {
    rc(kk(w - 31), kk(3), kk(24), kk(24))
}

/// Where each calculator key is.
fn key_rect(i: usize) -> RECT {
    let (bw, bh, gap) = (56, 34, 6);
    rc(kk(14 + (i as i32 % 4) * (bw + gap)), kk(170 + (i as i32 / 4) * (bh + gap)), kk(bw), kk(bh))
}

fn wide_key(i: i32) -> RECT {
    rc(kk(14 + (i - EQUALS) * 124), kk(370), kk(118), kk(26))
}

fn tab_rect(i: usize) -> RECT {
    rc(kk(14 + i as i32 * 44), kk(40), kk(38), kk(24))
}

unsafe fn font(px: i32) -> HFONT {
    CreateFontW(-px, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr())
}

/// Register the window classes (once, at startup).
pub(crate) unsafe fn register(hinst: HINSTANCE) {
    for (name, proc) in [("PixelCalcWnd", calc_proc as WndFn), ("PixelNoteWnd", note_proc as WndFn)] {
        let class = wide(name);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(proc),
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
    }
}

type WndFn = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Create a tool window with one edit box, beside the pet if there is room.
unsafe fn create(class: &str, title: &str, w: i32, h: i32, edit: RECT, style: u32, px: i32) -> (HWND, HWND) {
    let mut pr: RECT = zeroed();
    GetWindowRect(hw(&PET_WND), &mut pr);
    let wa = work_at((pr.left + pr.right) / 2, (pr.top + pr.bottom) / 2);
    let (ww, wh) = (kk(w), kk(h));
    let (x, y) = clamp_in(&wa, (pr.left + pr.right - ww) / 2, pr.bottom - wh - kk(130), ww, wh);
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        wide(class).as_ptr(),
        wide(title).as_ptr(),
        WS_POPUP,
        x, y, ww, wh,
        null_mut(), null_mut(), GetModuleHandleW(null()), null(),
    );
    let e = CreateWindowExW(
        0,
        wide("EDIT").as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        edit.left, edit.top, edit.right - edit.left, edit.bottom - edit.top,
        hwnd, null_mut(), GetModuleHandleW(null()), null(),
    );
    SendMessageW(e, WM_SETFONT, font(kk(px)) as WPARAM, 1);
    (hwnd, e)
}

/// The shared look: frame, title bar, close button. Calls `body` to draw the rest.
unsafe fn painted(hwnd: HWND, w: i32, h: i32, title: &str, hover: i32, body: impl FnOnce(HDC, HFONT, HFONT)) {
    let mut ps: PAINTSTRUCT = zeroed();
    let screen = BeginPaint(hwnd, &mut ps);
    let (pw, ph) = (kk(w), kk(h));
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, pw, ph);
    let old_bmp = SelectObject(dc, bmp);
    SetBkMode(dc, TRANSPARENT as i32);
    let b = kk(3);
    fill(dc, 0, 0, pw, ph, theme().accent);
    fill(dc, b, b, pw - 2 * b, ph - 2 * b, theme().ink);
    fill(dc, b, b, pw - 2 * b, kk(30) - b, theme().accent);
    let (big, small) = (font(kk(14)), font(kk(12)));
    let old = SelectObject(dc, big);
    SetTextColor(dc, theme().ink);
    let mut tr = rc(kk(14), 0, kk(w - 50), kk(30));
    text_at(dc, title, &mut tr, DT_SINGLELINE | DT_VCENTER);
    let c = close_rect(w);
    let hot = hover == CLOSE;
    fill(dc, c.left, c.top, c.right - c.left, c.bottom - c.top, if hot { CREAM } else { theme().ink });
    let cp = kk(2);
    sprite(dc, &CROSS, c.left + (c.right - c.left - 7 * cp) / 2, c.top + (c.bottom - c.top - 7 * cp) / 2, cp, &|ch| (ch == b'X').then_some(if hot { theme().ink } else { theme().accent }));
    body(dc, big, small);
    BitBlt(screen, 0, 0, pw, ph, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old);
    DeleteObject(big);
    DeleteObject(small);
    SelectObject(dc, old_bmp);
    DeleteObject(bmp);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}

unsafe fn framed(dc: HDC, r: &RECT, edge: u32) {
    let e = kk(2);
    fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, edge);
    fill(dc, r.left + e, r.top + e, r.right - r.left - 2 * e, r.bottom - r.top - 2 * e, theme().panel);
}

/// What both windows do the same way. None = not handled.
unsafe fn common(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, w: i32) -> Option<LRESULT> {
    match msg {
        WM_ERASEBKGND => Some(1),
        0x0133 | 0x0138 /* WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC */ => {
            let dc = wp as HDC;
            SetTextColor(dc, CREAM);
            SetBkColor(dc, theme().panel);
            SetDCBrushColor(dc, theme().panel);
            Some(GetStockObject(DC_BRUSH) as LRESULT)
        }
        WM_NCHITTEST => {
            let mut p = POINT { x: (lp & 0xFFFF) as i16 as i32, y: ((lp >> 16) & 0xFFFF) as i16 as i32 };
            ScreenToClient(hwnd, &mut p);
            Some(if p.y < kk(30) && !contains(&close_rect(w), p.x, p.y) { 2 } else { 1 })
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            Some(0)
        }
        _ => None,
    }
}

unsafe fn toggle(w: HWND, edit: HWND) {
    if IsWindowVisible(w) != 0 && GetForegroundWindow() == w {
        SendMessageW(w, WM_CLOSE, 0, 0);
        return;
    }
    ShowWindow(w, SW_SHOW);
    SetForegroundWindow(w);
    SetFocus(edit);
}

// ======================================================================
// calculator
// ======================================================================

pub(crate) unsafe fn show_calc() {
    if hw(&CALC_WND).is_null() {
        let edit = rc(kk(22), kk(118), kk(CALC_W - 44), kk(22));
        let (w, e) = create("PixelCalcWnd", "Pixel Pet - Calculator", CALC_W, CALC_H, edit, 0x80 | 0x2 /* ES_AUTOHSCROLL | ES_RIGHT */, 17);
        CALC_WND.store(w as isize, Relaxed);
        CALC_EDIT.store(e as isize, Relaxed);
    }
    toggle(hw(&CALC_WND), hw(&CALC_EDIT));
}

/// Enter or `=`: put the result in the box and the sum on the tape.
pub(crate) unsafe fn calc_equals() {
    let edit = hw(&CALC_EDIT);
    let text = get_text(edit);
    if let Some(v) = calc::eval(&text) {
        let shown = calc::show(v);
        if shown != text.trim() {
            let mut tape = TAPE.lock().unwrap();
            tape.push((text.trim().to_string(), shown.clone()));
            let extra = tape.len().saturating_sub(20);
            tape.drain(..extra);
        }
        SetWindowTextW(edit, wide(&shown.replace(' ', "")).as_ptr());
        SendMessageW(edit, EM_SETSEL, 0xFFFF, 0xFFFF);
    }
    InvalidateRect(hw(&CALC_WND), null(), 0);
}

unsafe fn calc_press(i: i32) {
    let edit = hw(&CALC_EDIT);
    match i {
        EQUALS => calc_equals(),
        COPY => {
            calc_equals();
            SendMessageW(edit, EM_SETSEL, 0, -1);
            SendMessageW(edit, WM_COPY, 0, 0);
        }
        0..=19 => match KEYS[i as usize] {
            "C" => {
                SetWindowTextW(edit, wide("").as_ptr());
            }
            "DEL" => {
                SendMessageW(edit, WM_CHAR, 8, 0);
            }
            k => {
                SendMessageW(edit, EM_REPLACESEL, 1, wide(k).as_ptr() as LPARAM);
            }
        },
        _ => {}
    }
    SetFocus(edit);
}

fn calc_hit(x: i32, y: i32) -> i32 {
    if contains(&close_rect(CALC_W), x, y) {
        return CLOSE;
    }
    if let Some(i) = [EQUALS, COPY].into_iter().find(|i| contains(&wide_key(*i), x, y)) {
        return i;
    }
    (0..KEYS.len()).find(|i| contains(&key_rect(*i), x, y)).map_or(-1, |i| i as i32)
}

unsafe fn calc_paint(hwnd: HWND) {
    let hover = CALC_HOVER.load(Relaxed);
    let text = get_text(hw(&CALC_EDIT));
    let tape: Vec<(String, String)> = TAPE.lock().unwrap().iter().rev().take(4).cloned().collect();
    painted(hwnd, CALC_W, CALC_H, "CALCULATOR", hover, |dc, big, small| {
        // the tape: the last few sums, newest at the bottom
        SelectObject(dc, small);
        for (i, (sum, res)) in tape.iter().enumerate() {
            SetTextColor(dc, if i == 0 { theme().hint } else { theme().dim });
            let mut r = rc(kk(16), kk(90 - i as i32 * 17), kk(CALC_W - 32), kk(17));
            text_at(dc, &format!("{sum} = {res}"), &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT | DT_END_ELLIPSIS);
        }
        framed(dc, &rc(kk(14), kk(112), kk(CALC_W - 28), kk(34)), GREEN);
        SelectObject(dc, big);
        let mut r = rc(kk(16), kk(148), kk(CALC_W - 32), kk(18));
        match calc::eval(&text) {
            Some(v) => {
                SetTextColor(dc, UP_BLUE);
                text_at(dc, &format!("= {}", calc::show(v)), &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            }
            None if !text.trim().is_empty() => {
                SetTextColor(dc, theme().dim);
                text_at(dc, "...", &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            }
            None => {
                SetTextColor(dc, theme().dim);
                SelectObject(dc, small);
                text_at(dc, "TYPE A SUM, ENTER GIVES THE RESULT", &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            }
        }
        SelectObject(dc, big);
        for (i, label) in KEYS.iter().enumerate() {
            let bg = match *label {
                "C" | "DEL" => BTN_RED,
                "/" | "*" | "-" | "+" => theme().panel_edge,
                _ => theme().panel_light,
            };
            pixel_button(dc, &key_rect(i), label, bg, hover == i as i32);
        }
        pixel_button(dc, &wide_key(EQUALS), "=", BTN_GREEN, hover == EQUALS);
        pixel_button(dc, &wide_key(COPY), "COPY", theme().panel_light, hover == COPY);
    });
}

unsafe extern "system" fn calc_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_PAINT => calc_paint(hwnd),
        WM_COMMAND if (wp >> 16) & 0xFFFF == EN_CHANGE => {
            InvalidateRect(hwnd, null(), 0);
        }
        WM_MOUSEMOVE => {
            let h = calc_hit(mx, my);
            if CALC_HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
        }
        WM_LBUTTONDOWN => match calc_hit(mx, my) {
            CLOSE => {
                ShowWindow(hwnd, SW_HIDE);
            }
            i => calc_press(i),
        },
        _ => return common(hwnd, msg, wp, lp, CALC_W).unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp)),
    }
    0
}

// ======================================================================
// notes
// ======================================================================

fn note_path(page: usize) -> Option<std::path::PathBuf> {
    data_path(&format!("pixel-pet-note{}.txt", page + 1))
}

fn read_note(page: usize) -> String {
    note_path(page).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default()
}

/// Write the open note to disk if it changed.
pub(crate) unsafe fn save_note() {
    if !NOTE_DIRTY.swap(false, Relaxed) {
        return;
    }
    let text = get_text(hw(&NOTE_EDIT));
    if let Some(p) = note_path(NOTE_PAGE.load(Relaxed)) {
        // first to a spare file, so a crash cannot leave half a note
        let tmp = p.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &p).is_err() {
            NOTE_DIRTY.store(true, Relaxed);
        }
    }
}

unsafe fn open_note(page: usize) {
    save_note();
    NOTE_PAGE.store(page, Relaxed);
    // the edit box wants Windows line ends
    let text = read_note(page).replace("\r\n", "\n").replace('\n', "\r\n");
    SetWindowTextW(hw(&NOTE_EDIT), wide(&text).as_ptr());
    NOTE_DIRTY.store(false, Relaxed); // filling it is not a change
    SendMessageW(hw(&NOTE_EDIT), EM_SETSEL, 0x7FFF_FFFF, 0x7FFF_FFFF);
    InvalidateRect(hw(&NOTE_WND), null(), 0);
}

pub(crate) unsafe fn show_notes() {
    if hw(&NOTE_WND).is_null() {
        let edit = rc(kk(20), kk(76), kk(NOTE_W - 40), kk(NOTE_H - 112));
        // ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL
        let (w, e) = create("PixelNoteWnd", "Pixel Pet - Notes", NOTE_W, NOTE_H, edit, 0x4 | 0x40 | 0x1000 | WS_VSCROLL, 15);
        NOTE_WND.store(w as isize, Relaxed);
        NOTE_EDIT.store(e as isize, Relaxed);
        SendMessageW(e, EM_SETLIMITTEXT, 200_000, 0);
        SetTimer(w, 1, 800, None); // saves while typing
        open_note(0);
    }
    toggle(hw(&NOTE_WND), hw(&NOTE_EDIT));
}

fn note_hit(x: i32, y: i32) -> i32 {
    if contains(&close_rect(NOTE_W), x, y) {
        return CLOSE;
    }
    (0..NOTE_PAGES).find(|i| contains(&tab_rect(*i), x, y)).map_or(-1, |i| i as i32)
}

unsafe fn note_paint(hwnd: HWND) {
    let hover = NOTE_HOVER.load(Relaxed);
    let page = NOTE_PAGE.load(Relaxed);
    let used: Vec<bool> = (0..NOTE_PAGES).map(|i| i == page || note_path(i).and_then(|p| std::fs::metadata(p).ok()).is_some_and(|m| m.len() > 0)).collect();
    let letters = GetWindowTextLengthW(hw(&NOTE_EDIT));
    painted(hwnd, NOTE_W, NOTE_H, "NOTES", hover, |dc, _, small| {
        for (i, used) in used.iter().enumerate() {
            let label = format!("{}{}", i + 1, if *used { "" } else { "-" });
            pixel_button(dc, &tab_rect(i), &label, if i == page { BTN_GREEN } else { theme().panel_light }, hover == i as i32);
        }
        framed(dc, &rc(kk(14), kk(70), kk(NOTE_W - 28), kk(NOTE_H - 100)), GREEN);
        SelectObject(dc, small);
        SetTextColor(dc, theme().hint);
        let mut r = rc(kk(14), kk(NOTE_H - 26), kk(NOTE_W - 28), kk(20));
        text_at(dc, "SAVED AS YOU TYPE.  ESC CLOSES.", &mut r, DT_SINGLELINE | DT_VCENTER);
        SetTextColor(dc, theme().dim);
        let mut r = rc(kk(14), kk(NOTE_H - 26), kk(NOTE_W - 28), kk(20));
        text_at(dc, &format!("{letters} CHARACTERS"), &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
    });
}

unsafe extern "system" fn note_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_PAINT => note_paint(hwnd),
        WM_COMMAND if (wp >> 16) & 0xFFFF == EN_CHANGE => {
            NOTE_DIRTY.store(true, Relaxed);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_TIMER => save_note(),
        WM_MOUSEMOVE => {
            let h = note_hit(mx, my);
            if NOTE_HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
        }
        WM_LBUTTONDOWN => match note_hit(mx, my) {
            CLOSE => {
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            -1 => {}
            i => {
                open_note(i as usize);
                SetFocus(hw(&NOTE_EDIT));
            }
        },
        WM_CLOSE => {
            save_note();
            ShowWindow(hwnd, SW_HIDE);
        }
        _ => return common(hwnd, msg, wp, lp, NOTE_W).unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp)),
    }
    0
}

/// Keys the edit boxes do not handle themselves. Returns true if it was one.
pub(crate) unsafe fn key(msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN {
        return false;
    }
    let (calc_e, note_e) = (hw(&CALC_EDIT), hw(&NOTE_EDIT));
    let in_calc = !calc_e.is_null() && (msg.hwnd == calc_e || msg.hwnd == hw(&CALC_WND));
    let in_note = !note_e.is_null() && (msg.hwnd == note_e || msg.hwnd == hw(&NOTE_WND));
    let k = msg.wParam as u16;
    if in_calc && k == VK_RETURN {
        calc_equals();
        return true;
    }
    if (in_calc || in_note) && k == VK_ESCAPE {
        SendMessageW(if in_calc { hw(&CALC_WND) } else { hw(&NOTE_WND) }, WM_CLOSE, 0, 0);
        return true;
    }
    if (in_calc || in_note) && k == b'A' as u16 && GetKeyState(VK_CONTROL as i32) < 0 {
        SendMessageW(msg.hwnd, EM_SETSEL, 0, -1);
        return true;
    }
    false
}

// ======================================================================
// VS Code
// ======================================================================

/// Places VS Code (or its Insiders build) is installed, best first.
fn code_paths() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for (var, sub) in [("LOCALAPPDATA", "Programs"), ("ProgramFiles", ""), ("ProgramFiles(x86)", "")] {
        if let Some(base) = std::env::var_os(var) {
            let base = std::path::PathBuf::from(base).join(sub);
            v.push(base.join("Microsoft VS Code").join("Code.exe"));
            v.push(base.join("Microsoft VS Code Insiders").join("Code - Insiders.exe"));
        }
    }
    // a custom folder still has its "bin" on the PATH
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path).filter(|d| d.join("code.cmd").exists()) {
            if let Some(home) = dir.parent() {
                v.push(home.join("Code.exe"));
            }
        }
    }
    v
}

/// Open VS Code. False if it cannot be found.
pub(crate) unsafe fn open_code() -> bool {
    let Some(exe) = code_paths().into_iter().find(|p| p.exists()) else { return false };
    if std::env::var_os("PIXEL_PET_TEST").is_some() {
        return true;
    }
    ShellExecuteW(null_mut(), wide("open").as_ptr(), wide(&exe.to_string_lossy()).as_ptr(), null(), null(), SW_SHOWNORMAL) as isize > 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculator_keys_do_not_overlap_and_stay_in_the_window() {
        let mut all: Vec<RECT> = (0..KEYS.len()).map(key_rect).collect();
        all.extend([wide_key(EQUALS), wide_key(COPY), close_rect(CALC_W)]);
        for (i, a) in all.iter().enumerate() {
            assert!(a.left >= 0 && a.top >= 0 && a.right <= kk(CALC_W) && a.bottom <= kk(CALC_H), "{i}");
            for b in &all[..i] {
                assert!(a.left >= b.right || b.left >= a.right || a.top >= b.bottom || b.top >= a.bottom, "{i} overlaps");
            }
        }
    }

    #[test]
    fn every_key_can_be_part_of_a_sum() {
        for k in KEYS.iter().filter(|k| !matches!(**k, "C" | "DEL")) {
            assert!("0123456789+-*/.%()".contains(k), "{k}");
        }
    }
}
