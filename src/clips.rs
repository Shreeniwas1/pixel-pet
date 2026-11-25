//! Clipboard history: the last things that were copied, to have again. Off until it is switched on.
//! It is only ever in memory: nothing is written to the disk, and it is gone when the pet quits. What
//! a password manager marks as secret is not looked at.

use crate::prelude::*;
use crate::tools::{self, close_rect, common, create, fit_edit, kk, painted, toggle, work_on, CLOSE};
use std::sync::atomic::AtomicUsize;

pub(crate) static WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static EDIT: AtomicIsize = AtomicIsize::new(0);
static HOVER: AtomicI32 = AtomicI32::new(-1);
static PICKED: AtomicUsize = AtomicUsize::new(0);
static FIRST: AtomicUsize = AtomicUsize::new(0);
/// Is it keeping anything? (the setting)
pub(crate) static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static LIST: Mutex<Clips> = Mutex::new(Clips { items: Vec::new() });

const MOST: usize = 20;
const MOST_PINS: usize = 10;
/// Bigger things are not for a list.
const LONGEST: usize = 100_000;

const W: i32 = 380;
const H: i32 = 446;
const ROWS: usize = 10;
const ROW: i32 = 30;
const TOP: i32 = 76;
const PIN: i32 = 20;
const DEL: i32 = 40;
const PAUSE: i32 = 60;
const CLEAR: i32 = 61;

// ---- rules (pure, so they can be tested) ----

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
    pub text: String,
    pub pin: bool,
}

/// Newest first.
pub(crate) struct Clips {
    items: Vec<Item>,
}

impl Clips {
    pub fn take(&mut self, text: &str) {
        if text.trim().is_empty() || text.len() > LONGEST {
            return;
        }
        // copied before: it comes to the top again
        let pin = self.items.iter().position(|i| i.text == text).is_some_and(|i| self.items.remove(i).pin);
        self.items.insert(0, Item { text: text.to_string(), pin });
        let mut n = 0;
        self.items.retain(|i| {
            n += !i.pin as usize;
            i.pin || n <= MOST
        });
    }

    /// The numbers of those with these words in them.
    pub fn find(&self, words: &str) -> Vec<usize> {
        let w = words.trim().to_lowercase();
        let mut l: Vec<usize> = (0..self.items.len()).filter(|i| w.is_empty() || self.items[*i].text.to_lowercase().contains(&w)).collect();
        l.sort_by_key(|i| !self.items[*i].pin); // pinned ones first
        l
    }

    pub fn pin(&mut self, i: usize) {
        let pins = self.items.iter().filter(|i| i.pin).count();
        if let Some(it) = self.items.get_mut(i) {
            it.pin = !it.pin && pins < MOST_PINS;
        }
    }

    pub fn clear(&mut self) {
        self.items.retain(|i| i.pin);
    }
}

/// One line of it.
pub(crate) fn preview(text: &str) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let lines = text.trim().lines().count();
    let short: String = line.chars().take(60).collect();
    if lines > 1 { format!("{short} [{lines} lines]") } else { short }
}

// ---- the clipboard itself ----

#[cfg(windows)]
mod board {
    use super::*;
    use windows_sys::Win32::System::DataExchange::*;
    use windows_sys::Win32::System::Memory::*;
    const UNICODE_TEXT: u32 = 13;

    pub unsafe fn count() -> u32 {
        GetClipboardSequenceNumber()
    }

    /// Whoever copied it asked for it not to be kept. The clipboard is open.
    unsafe fn hidden() -> bool {
        for name in ["ExcludeClipboardContentFromMonitorProcessing", "Clipboard Viewer Ignore"] {
            if IsClipboardFormatAvailable(RegisterClipboardFormatW(wide(name).as_ptr())) != 0 {
                return true;
            }
        }
        let may = RegisterClipboardFormatW(wide("CanIncludeInClipboardHistory").as_ptr());
        if IsClipboardFormatAvailable(may) == 0 {
            return false;
        }
        let h = GetClipboardData(may);
        let p = if h.is_null() { null() } else { GlobalLock(h) as *const u32 };
        if p.is_null() {
            return true;
        }
        let no = GlobalSize(h) >= 4 && *p == 0;
        GlobalUnlock(h);
        no
    }

    pub unsafe fn read(owner: HWND) -> Option<String> {
        if IsClipboardFormatAvailable(UNICODE_TEXT) == 0 || OpenClipboard(owner) == 0 {
            return None;
        }
        let mut text = None;
        if !hidden() {
            let h = GetClipboardData(UNICODE_TEXT);
            let p = if h.is_null() { null() } else { GlobalLock(h) as *const u16 };
            if !p.is_null() {
                let s = std::slice::from_raw_parts(p, GlobalSize(h) / 2);
                let len = s.iter().position(|c| *c == 0).unwrap_or(s.len());
                text = (len <= LONGEST).then(|| String::from_utf16_lossy(&s[..len]));
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        text
    }

    pub unsafe fn write(owner: HWND, text: &str) -> bool {
        let w = wide(text);
        let h = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
        if h.is_null() {
            return false;
        }
        let p = GlobalLock(h) as *mut u16;
        if !p.is_null() {
            std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
            GlobalUnlock(h);
        }
        if !p.is_null() && OpenClipboard(owner) != 0 {
            EmptyClipboard();
            let ok = !SetClipboardData(UNICODE_TEXT, h).is_null();
            CloseClipboard();
            if ok {
                return true; // it is Windows' now
            }
        }
        GlobalFree(h);
        false
    }
}

#[cfg(not(windows))]
mod board {
    use super::*;
    use crate::port::win;

    pub unsafe fn count() -> u32 {
        win::clip_count()
    }

    pub unsafe fn read(_: HWND) -> Option<String> {
        win::clip_text().filter(|t| t.len() <= LONGEST)
    }

    pub unsafe fn write(_: HWND, text: &str) -> bool {
        win::clip_set(text);
        true
    }
}

/// Has something been copied? Asked twice a second, which costs nothing.
pub(crate) unsafe fn look(hwnd: HWND) {
    static SEEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = board::count();
    if SEEN.swap(n, Relaxed) == n || !ON.load(Relaxed) {
        return;
    }
    if let Some(t) = board::read(hwnd) {
        LIST.lock().unwrap().take(&t);
        InvalidateRect(hw(&WND), null(), 0);
    }
}

/// How many are kept.
pub(crate) fn len() -> usize {
    LIST.lock().unwrap().items.len()
}

// ---- the window ----

fn edit_rect() -> RECT {
    rc(kk(22), kk(45), kk(W - 44), kk(20))
}

fn row_rect(i: usize) -> RECT {
    rc(kk(14), kk(TOP + i as i32 * ROW), kk(W - 28), kk(ROW - 4))
}

fn pin_rect(i: usize) -> RECT {
    let r = row_rect(i);
    RECT { left: r.right - kk(56), right: r.right - kk(30), ..r }
}

fn del_rect(i: usize) -> RECT {
    let r = row_rect(i);
    RECT { left: r.right - kk(28), ..r }
}

fn button_rect(i: i32) -> RECT {
    rc(kk(14 + (i - PAUSE) * 180), kk(H - 60), kk(172), kk(26))
}

unsafe fn shown() -> Vec<usize> {
    LIST.lock().unwrap().find(&get_text(hw(&EDIT)))
}

fn first(n: usize) -> usize {
    FIRST.load(Relaxed).min(n.saturating_sub(ROWS))
}

fn hit(x: i32, y: i32) -> i32 {
    if contains(&close_rect(W), x, y) {
        return CLOSE;
    }
    if let Some(b) = [PAUSE, CLEAR].into_iter().find(|b| contains(&button_rect(*b), x, y)) {
        return b;
    }
    match (0..ROWS).find(|i| contains(&row_rect(*i), x, y)) {
        Some(i) if contains(&del_rect(i), x, y) => DEL + i as i32,
        Some(i) if contains(&pin_rect(i), x, y) => PIN + i as i32,
        Some(i) => i as i32,
        None => -1,
    }
}

pub(crate) unsafe fn show() {
    if hw(&WND).is_null() {
        work_on(null_mut());
        let (w, e) = create("PixelClipWnd", "Pixel Pet - Clipboard", W, H, edit_rect(), 0x80 /* ES_AUTOHSCROLL */, 14);
        WND.store(w as isize, Relaxed);
        EDIT.store(e as isize, Relaxed);
    }
    SetWindowTextW(hw(&EDIT), wide("").as_ptr());
    PICKED.store(0, Relaxed);
    FIRST.store(0, Relaxed);
    toggle(hw(&WND), hw(&EDIT));
}

/// Put it on the clipboard again, and get out of the way.
unsafe fn give(row: usize) {
    let l = shown();
    let text = l.get(first(l.len()) + row).and_then(|i| LIST.lock().unwrap().items.get(*i).map(|it| it.text.clone()));
    if let Some(t) = text {
        if board::write(hw(&WND), &t) {
            ShowWindow(hw(&WND), SW_HIDE);
            say_text(&mut ST.lock().unwrap(), "Copied. Paste away!", 6);
        }
    }
}

fn set_on(on: bool) {
    ON.store(on, Relaxed);
    let mut s = ST.lock().unwrap();
    s.clips_on = on;
    save_cfg(&s);
}

unsafe fn press(h: i32) {
    let l = shown();
    let at = |r: i32| l.get(first(l.len()) + r as usize).copied();
    match h {
        PAUSE => set_on(!ON.load(Relaxed)),
        CLEAR => LIST.lock().unwrap().clear(),
        0..=9 => return give(h as usize),
        PIN..=29 => {
            if let Some(i) = at(h - PIN) {
                LIST.lock().unwrap().pin(i);
            }
        }
        DEL..=49 => {
            if let Some(i) = at(h - DEL) {
                LIST.lock().unwrap().items.remove(i);
            }
        }
        _ => {}
    }
    SetFocus(hw(&EDIT));
    InvalidateRect(hw(&WND), null(), 0);
}

/// Up, down and Enter, while its box has the keys. True if it was one of them.
pub(crate) unsafe fn key(k: u16) -> bool {
    let n = shown().len();
    let p = PICKED.load(Relaxed);
    match k {
        VK_UP => PICKED.store(p.saturating_sub(1), Relaxed),
        VK_DOWN => PICKED.store((p + 1).min(n.saturating_sub(1)), Relaxed),
        VK_RETURN => {
            let p = p.saturating_sub(first(n));
            give(p);
            return true;
        }
        _ => return false,
    }
    let p = PICKED.load(Relaxed);
    let f = first(n);
    FIRST.store(if p < f { p } else if p >= f + ROWS { p + 1 - ROWS } else { f }, Relaxed);
    InvalidateRect(hw(&WND), null(), 0);
    true
}

unsafe fn draw(hwnd: HWND) {
    let hover = HOVER.load(Relaxed);
    let l = shown();
    let f = first(l.len());
    let rows: Vec<Item> = {
        let list = LIST.lock().unwrap();
        l.iter().skip(f).take(ROWS).filter_map(|i| list.items.get(*i).cloned()).collect()
    };
    let picked = PICKED.load(Relaxed).min(l.len().saturating_sub(1));
    let on = ON.load(Relaxed);
    painted(hwnd, W, H, "CLIPBOARD", hover, |dc, big, small| {
        tools::framed(dc, &rc(kk(14), kk(40), kk(W - 28), kk(30)), UP_BLUE);
        SelectObject(dc, big);
        for (i, it) in rows.iter().enumerate() {
            let r = row_rect(i);
            let lit = hover == i as i32 || f + i == picked;
            fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, if lit { theme().panel_light } else { theme().panel });
            SetTextColor(dc, CREAM);
            let mut tr = RECT { left: r.left + kk(8), right: r.right - kk(62), ..r };
            text_at(dc, &preview(&it.text), &mut tr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
            let pr = pin_rect(i);
            let p = kk(2);
            let c = if it.pin { rgb(0xff, 0xe0, 0x66) } else if hover == PIN + i as i32 { CREAM } else { theme().dim };
            sprite(dc, &ICON_PIN, (pr.left + pr.right - 7 * p) / 2, (pr.top + pr.bottom - 7 * p) / 2, p, &|ch| (ch == b'X').then_some(c));
            let dr = del_rect(i);
            let c = if hover == DEL + i as i32 { RED } else { theme().dim };
            sprite(dc, &CROSS, (dr.left + dr.right - 7 * p) / 2, (dr.top + dr.bottom - 7 * p) / 2, p, &|ch| (ch == b'X').then_some(c));
        }
        SelectObject(dc, small);
        if rows.is_empty() {
            SetTextColor(dc, theme().hint);
            let mut r = rc(kk(24), kk(TOP + 20), kk(W - 48), kk(160));
            let t = if !on {
                "Switched on, I remember the last 20 texts you copy, so you can have them again. They stay in memory only: never on the disk, and gone when I quit. What password managers mark as secret I leave alone."
            } else if get_text(hw(&EDIT)).trim().is_empty() {
                "Copy something and it shows up here. Click it to have it on the clipboard again; the star keeps it."
            } else {
                "Nothing with those words in it."
            };
            text_at(dc, t, &mut r, DT_WORDBREAK);
        }
        SelectObject(dc, big);
        pixel_button(dc, &button_rect(PAUSE), if on { "PAUSE" } else { "SWITCH ON" }, if on { theme().panel_light } else { BTN_GREEN }, hover == PAUSE);
        pixel_button(dc, &button_rect(CLEAR), "CLEAR ALL", BTN_RED, hover == CLEAR);
        SelectObject(dc, small);
        SetTextColor(dc, theme().hint);
        let mut r = rc(kk(14), kk(H - 28), kk(W - 28), kk(20));
        text_at(dc, "TYPE TO SEARCH. ENTER COPIES.", &mut r, DT_SINGLELINE | DT_VCENTER);
        SetTextColor(dc, if on { GREEN } else { theme().dim });
        let mut r = rc(kk(14), kk(H - 28), kk(W - 28), kk(20));
        text_at(dc, if on { "ON" } else { "OFF" }, &mut r, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
    });
}

pub(crate) unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    work_on(hwnd);
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_PAINT => draw(hwnd),
        WM_DPICHANGED => {
            resize_for_dpi(hwnd, lp);
            fit_edit(hw(&EDIT), edit_rect(), 14);
        }
        WM_COMMAND if (wp >> 16) & 0xFFFF == 0x0300 /* EN_CHANGE */ => {
            PICKED.store(0, Relaxed);
            FIRST.store(0, Relaxed);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_MOUSEMOVE => {
            let h = hit(mx, my);
            if HOVER.swap(h, Relaxed) != h {
                InvalidateRect(hwnd, null(), 0);
            }
        }
        WM_MOUSEWHEEL => {
            let up = ((wp >> 16) & 0xFFFF) as i16 > 0;
            let f = first(shown().len());
            FIRST.store(if up { f.saturating_sub(2) } else { f + 2 }, Relaxed);
            InvalidateRect(hwnd, null(), 0);
        }
        WM_LBUTTONDOWN => match hit(mx, my) {
            CLOSE => {
                ShowWindow(hwnd, SW_HIDE);
            }
            h => press(h),
        },
        _ => return common(hwnd, msg, wp, lp, W).unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp)),
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(l: &[&str]) -> Clips {
        let mut c = Clips { items: Vec::new() };
        for t in l {
            c.take(t);
        }
        c
    }

    fn texts(c: &Clips) -> Vec<&str> {
        c.items.iter().map(|i| i.text.as_str()).collect()
    }

    #[test]
    fn the_newest_is_on_top_and_nothing_is_there_twice() {
        let c = list(&["a", "b", "  ", "", "a", "c"]);
        assert_eq!(texts(&c), ["c", "a", "b"]);
    }

    #[test]
    fn the_oldest_go_but_pinned_ones_stay() {
        let mut c = list(&["keep"]);
        c.pin(0);
        for i in 0..50 {
            c.take(&format!("n{i}"));
        }
        assert_eq!(c.items.len(), MOST + 1);
        assert!(c.items.iter().any(|i| i.text == "keep" && i.pin));
        assert_eq!(c.items[0].text, "n49");
        assert_eq!(c.items[c.find("")[0]].text, "keep", "pinned ones are shown first");
        c.take("keep");
        assert!(c.items[0].pin, "copied again it stays pinned");
        c.clear();
        assert_eq!(texts(&c), ["keep"]);
        c.pin(0);
        c.clear();
        assert!(c.items.is_empty());
    }

    #[test]
    fn it_is_found_by_its_words() {
        let c = list(&["Hello World", "cargo test", "hello again"]);
        assert_eq!(c.find("HELLO").len(), 2);
        assert_eq!(c.find(" test ").len(), 1);
        assert!(c.find("nothing").is_empty());
    }

    #[test]
    fn long_and_odd_things_are_shown_in_one_short_line() {
        assert_eq!(preview("  a\r\n\tb  c "), "a b c [2 lines]");
        assert_eq!(preview(&"x".repeat(500)).len(), 60);
        assert_eq!(preview("\u{20ac}"), "\u{20ac}");
        let mut c = list(&[]);
        c.take(&"x".repeat(LONGEST + 1));
        assert!(c.items.is_empty());
    }

    #[test]
    fn pins_are_limited() {
        let mut c = list(&[]);
        for i in 0..MOST {
            c.take(&format!("{i}"));
        }
        for i in 0..MOST {
            c.pin(i);
        }
        assert_eq!(c.items.iter().filter(|i| i.pin).count(), MOST_PINS);
    }

    #[test]
    fn nothing_in_the_window_overlaps() {
        let mut all: Vec<RECT> = (0..ROWS).map(row_rect).collect();
        all.extend([button_rect(PAUSE), button_rect(CLEAR), close_rect(W), edit_rect()]);
        for (i, a) in all.iter().enumerate() {
            assert!(a.left >= 0 && a.top >= 0 && a.right <= kk(W) && a.bottom <= kk(H), "{i}");
            for b in &all[..i] {
                assert!(a.left >= b.right || b.left >= a.right || a.top >= b.bottom || b.top >= a.bottom, "{i} overlaps");
            }
        }
    }
}
