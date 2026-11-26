//! Windows, messages and timers.
//!
//! Everything here happens on one thread, apart from posting messages. No reference into `G` is kept
//! while one of the pet's own procedures runs, because those call back in here.

use super::edit::Edit;
use super::gdi::{self, Obj};
use super::peers::{self, Peer, Post};
use super::screen::Screen;
use super::*;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

type Proc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

struct Win {
    id: u32,
    class: String,
    title: String,
    proc: Option<Proc>,
    style: u32,
    ex: u32,
    parent: u32,
    rect: RECT, // on the screen; for a child, in its parent
    visible: bool,
    picture: *mut Obj,
    dirty: Option<RECT>,
    props: HashMap<String, isize>,
    key: Option<u32>,
    alpha: u8,
    edit: Option<Edit>,
    font: isize,
    tracking: bool,
    opening: bool, // to be shown as soon as it is painted
    wants: bool,   // the keyboard, once it is
}

struct Timer {
    win: u32,
    id: usize,
    every: u32,
    due: Instant,
}

struct Globals {
    screen: Screen,
    post: Post,
    classes: HashMap<String, Proc>,
    wins: Vec<Win>,
    next: u32,
    timers: Vec<Timer>,
    quit: Option<i32>,
    focus: u32,
    capture: u32,
    moving: Option<(u32, (i32, i32))>, // a window dragged by its title bar, and where it was taken
    selecting: u32,                    // an edit box the mouse went down in
    keys: [bool; 256],
    typed: String, // what the last key press writes
    others: (Option<Instant>, Vec<Foreign>),
    clip: Vec<u16>,
    drops: Vec<Vec<String>>,
    told: Instant,
}

static mut G: Option<Globals> = None;
static THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);
/// Posted messages, and redraws asked for from other threads.
static QUEUE: Mutex<VecDeque<(usize, u32, usize, isize)>> = Mutex::new(VecDeque::new());

#[allow(static_mut_refs)]
unsafe fn g() -> &'static mut Globals {
    G.as_mut().expect("the screen is open")
}

#[allow(static_mut_refs)]
unsafe fn open() -> bool {
    G.is_some()
}

pub(crate) fn begin(screen: Screen) {
    *THREAD.lock().unwrap() = Some(std::thread::current().id());
    unsafe {
        G = Some(Globals {
            screen,
            post: Post::open(),
            classes: HashMap::new(),
            wins: Vec::new(),
            next: 1,
            timers: Vec::new(),
            quit: None,
            focus: 0,
            capture: 0,
            moving: None,
            selecting: 0,
            keys: [false; 256],
            typed: String::new(),
            others: (None, Vec::new()),
            clip: Vec::new(),
            drops: Vec::new(),
            told: Instant::now(),
        });
    }
}

/// Put away what others can see of us.
pub(crate) fn end() {
    if on_thread() {
        unsafe { G = None };
    }
}

fn on_thread() -> bool {
    *THREAD.lock().unwrap() == Some(std::thread::current().id())
}

// ---- handles: our own windows, those of other pets, and those of other programs ----

const PEER: usize = 1 << 61;
const OTHER: usize = 1 << 62;

#[derive(Clone, Copy)]
enum Who {
    Own(u32),
    Peer(u32, u32),
    Other(u64),
    Nobody,
}

fn who(h: HWND) -> Who {
    let v = h as usize;
    if v & OTHER != 0 {
        Who::Other((v & !OTHER) as u64)
    } else if v & PEER != 0 {
        Who::Peer(((v & !PEER) >> 16) as u32, (v & 0xffff) as u32)
    } else if v != 0 && v < 65536 {
        Who::Own(v as u32)
    } else {
        Who::Nobody
    }
}

fn own(id: u32) -> HWND {
    id as usize as HWND
}

fn of_peer(p: &Peer) -> HWND {
    (PEER | (p.pid as usize) << 16 | p.id as usize) as HWND
}

fn of_other(id: u64) -> HWND {
    (OTHER | id as usize) as HWND
}

unsafe fn win(id: u32) -> Option<&'static mut Win> {
    g().wins.iter_mut().find(|w| w.id == id)
}

unsafe fn mine(h: HWND) -> Option<&'static mut Win> {
    match who(h) {
        Who::Own(id) if open() => win(id),
        _ => None,
    }
}

unsafe fn peer(pid: u32, id: u32) -> Option<Peer> {
    peers::windows_of(pid).into_iter().find(|p| p.id == id)
}

/// Other programs' windows, topmost first; asked for at most a few times a second.
unsafe fn others() -> &'static Vec<Foreign> {
    let s = g();
    if s.others.0.is_none_or(|t| t.elapsed() > Duration::from_millis(250)) {
        s.others = (Some(Instant::now()), s.screen.others());
    }
    &s.others.1
}

unsafe fn other(id: u64) -> Option<&'static Foreign> {
    others().iter().find(|o| o.id == id)
}

unsafe fn call(id: u32, msg: u32, wp: usize, lp: isize) -> isize {
    let Some(w) = win(id) else { return 0 };
    if w.edit.is_some() {
        return edit_proc(id, msg, wp, lp);
    }
    match w.proc {
        Some(p) => p(own(id), msg, wp, lp),
        None => 0,
    }
}

// ---- making and placing ----

#[no_mangle]
pub unsafe extern "system" fn RegisterClassW(c: *const WNDCLASSW) -> u16 {
    if let Some(p) = (*c).lpfnWndProc {
        g().classes.insert(string((*c).lpszClassName), std::mem::transmute::<_, Proc>(p));
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn CreateWindowExW(ex: WINDOW_EX_STYLE, class: *const u16, title: *const u16, style: WINDOW_STYLE, x: i32, y: i32, w: i32, h: i32, parent: HWND, _: HMENU, _: HINSTANCE, _: *const std::ffi::c_void) -> HWND {
    let (class, title) = (string(class), string(title));
    let s = g();
    let id = s.next;
    s.next += 1;
    let child = style & WS_CHILD != 0;
    let rect = RECT { left: x, top: y, right: x + w, bottom: y + h };
    let is_edit = class == "EDIT";
    let proc = s.classes.get(&class).copied();
    s.wins.push(Win {
        id,
        class,
        title: title.clone(),
        proc,
        style,
        ex,
        parent: if child { parent as usize as u32 } else { 0 },
        rect,
        visible: child && style & WS_VISIBLE != 0,
        picture: if child { std::ptr::null_mut() } else { gdi::new_bitmap(w, h) },
        dirty: None,
        props: HashMap::new(),
        key: None,
        alpha: 255,
        edit: is_edit.then(|| Edit::new(style)),
        font: 0,
        tracking: false,
        opening: false,
        wants: false,
    });
    if !child {
        s.screen.create(&Make { id, rect, title, takes_focus: ex & WS_EX_NOACTIVATE == 0, top: ex & WS_EX_TOPMOST != 0, through: ex & WS_EX_TRANSPARENT != 0 });
        call(id, WM_CREATE, 0, 0);
    }
    own(id)
}

#[no_mangle]
pub unsafe extern "system" fn DestroyWindow(h: HWND) -> BOOL {
    let Who::Own(id) = who(h) else { return 0 };
    if win(id).is_none() {
        return 0;
    }
    call(id, WM_DESTROY, 0, 0);
    let s = g();
    s.timers.retain(|t| t.win != id);
    s.wins.retain(|w| {
        if w.id == id || w.parent == id {
            unsafe { gdi::free(w.picture) };
        }
        w.id != id && w.parent != id
    });
    s.screen.destroy(id);
    tell(true);
    1
}

unsafe fn invalidate(id: u32, r: Option<RECT>) {
    if let Some(w) = win(id) {
        let all = RECT { left: 0, top: 0, right: w.rect.right - w.rect.left, bottom: w.rect.bottom - w.rect.top };
        let r = r.unwrap_or(all);
        w.dirty = Some(match w.dirty {
            Some(d) => RECT { left: d.left.min(r.left), top: d.top.min(r.top), right: d.right.max(r.right), bottom: d.bottom.max(r.bottom) },
            None => r,
        });
    }
}

#[no_mangle]
pub unsafe extern "system" fn SetWindowPos(h: HWND, after: HWND, x: i32, y: i32, w: i32, hh: i32, flags: SET_WINDOW_POS_FLAGS) -> BOOL {
    let Who::Own(id) = who(h) else { return 0 };
    let Some(me) = win(id) else { return 0 };
    let was = me.rect;
    let (mut r, size) = (was, (was.right - was.left, was.bottom - was.top));
    let new = if flags & SWP_NOSIZE != 0 { size } else { (w.max(1), hh.max(1)) };
    if flags & SWP_NOMOVE == 0 {
        (r.left, r.top) = (x, y);
    }
    (r.right, r.bottom) = (r.left + new.0, r.top + new.1);
    me.rect = r;
    if me.parent != 0 {
        edit_redraw(id);
        return 1;
    }
    if new != size {
        gdi::free(me.picture);
        me.picture = gdi::new_bitmap(new.0, new.1);
        invalidate(id, None);
    }
    let me = win(id).unwrap();
    if flags & SWP_NOZORDER == 0 {
        match after as isize {
            -1 => me.ex |= WS_EX_TOPMOST,
            -2 => me.ex &= !WS_EX_TOPMOST,
            _ => {}
        }
    }
    if (r.left, r.top, r.right, r.bottom) != (was.left, was.top, was.right, was.bottom) {
        g().screen.place(id, r);
    }
    if flags & SWP_NOZORDER == 0 {
        match after as isize {
            -1 => g().screen.raise(id),
            -2 => g().screen.lower(id),
            _ => if let Who::Own(a) = who(after) { g().screen.below(id, a) },
        }
    }
    if flags & SWP_SHOWWINDOW != 0 {
        ShowWindow(h, SW_SHOWNOACTIVATE);
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn ShowWindow(h: HWND, how: SHOW_WINDOW_CMD) -> BOOL {
    let Who::Own(id) = who(h) else { return 0 };
    let Some(w) = win(id) else { return 0 };
    let (was, show) = (w.visible, how != SW_HIDE);
    w.visible = show;
    if w.parent != 0 || was == show {
        return was as BOOL;
    }
    if show {
        w.opening = true;
        w.wants = how == SW_SHOW;
        invalidate(id, None);
    } else {
        let s = g();
        s.screen.show(id, false);
        if s.capture == id {
            s.capture = 0;
        }
        let focus = s.focus;
        if focus == id || win(focus).is_some_and(|f| f.parent == id) {
            g().focus = 0;
        }
    }
    was as BOOL
}

#[no_mangle]
pub unsafe extern "system" fn SetLayeredWindowAttributes(h: HWND, key: COLORREF, alpha: u8, flags: LAYERED_WINDOW_ATTRIBUTES_FLAGS) -> BOOL {
    let Some(w) = mine(h) else { return 0 };
    if flags & LWA_COLORKEY != 0 {
        w.key = Some((key & 0xff) << 16 | key & 0xff00 | key >> 16 & 0xff);
    }
    w.alpha = if flags & LWA_ALPHA != 0 { alpha } else { 255 };
    let id = w.id;
    present(id);
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetWindowLongW(h: HWND, what: WINDOW_LONG_PTR_INDEX) -> i32 {
    match who(h) {
        Who::Own(_) => mine(h).map_or(0, |w| if what == GWL_EXSTYLE { w.ex as i32 } else { w.style as i32 }),
        // an ordinary window: with a frame, unless it fills the screen
        Who::Other(id) if what == GWL_STYLE => if other(id).is_some_and(|o| o.full) { 0 } else { WS_CAPTION as i32 },
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn SetWindowLongW(h: HWND, what: WINDOW_LONG_PTR_INDEX, v: i32) -> i32 {
    let Some(w) = mine(h) else { return 0 };
    let slot = if what == GWL_EXSTYLE { &mut w.ex } else { &mut w.style };
    let was = std::mem::replace(slot, v as u32);
    let (id, through) = (w.id, w.ex & WS_EX_TRANSPARENT != 0);
    if what == GWL_EXSTYLE && (was ^ v as u32) & WS_EX_TRANSPARENT != 0 {
        g().screen.pass_clicks(id, through);
        present(id);
    }
    was as i32
}

#[no_mangle]
pub unsafe extern "system" fn GetWindowRect(h: HWND, r: *mut RECT) -> BOOL {
    let found = match who(h) {
        Who::Own(id) => win(id).map(|w| if w.parent != 0 { child_rect(id) } else { w.rect }),
        Who::Peer(pid, id) => peer(pid, id).map(|p| RECT { left: p.rect.0, top: p.rect.1, right: p.rect.2, bottom: p.rect.3 }),
        Who::Other(id) => other(id).map(|o| o.rect),
        Who::Nobody => None,
    };
    if let Some(f) = found {
        *r = f;
    }
    found.is_some() as BOOL
}

/// Where a child is on the screen.
unsafe fn child_rect(id: u32) -> RECT {
    let Some(w) = win(id) else { return RECT { left: 0, top: 0, right: 0, bottom: 0 } };
    let r = w.rect;
    let (dx, dy) = win(w.parent).map_or((0, 0), |p| (p.rect.left, p.rect.top));
    RECT { left: r.left + dx, top: r.top + dy, right: r.right + dx, bottom: r.bottom + dy }
}

#[no_mangle]
pub unsafe extern "system" fn ScreenToClient(h: HWND, p: *mut POINT) -> BOOL {
    let mut r = std::mem::zeroed();
    if GetWindowRect(h, &mut r) == 0 {
        return 0;
    }
    (*p).x -= r.left;
    (*p).y -= r.top;
    1
}

#[no_mangle]
pub unsafe extern "system" fn IsWindow(h: HWND) -> BOOL {
    match who(h) {
        Who::Own(id) => win(id).is_some() as BOOL,
        Who::Peer(pid, id) => peers::all().iter().any(|p| p.pid == pid && p.id == id) as BOOL,
        Who::Other(id) => other(id).is_some() as BOOL,
        Who::Nobody => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn IsWindowVisible(h: HWND) -> BOOL {
    match who(h) {
        Who::Own(id) => win(id).is_some_and(|w| w.visible) as BOOL,
        Who::Peer(pid, id) => peer(pid, id).is_some_and(|p| p.visible) as BOOL,
        Who::Other(id) => other(id).is_some() as BOOL, // hidden ones are not listed
        Who::Nobody => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn IsIconic(h: HWND) -> BOOL {
    matches!(who(h), Who::Other(id) if other(id).is_none()) as BOOL
}

#[no_mangle]
pub unsafe extern "system" fn IsZoomed(h: HWND) -> BOOL {
    matches!(who(h), Who::Other(id) if other(id).is_some_and(|o| o.zoomed)) as BOOL
}

#[no_mangle]
pub unsafe extern "system" fn GetAncestor(h: HWND, _: u32) -> HWND {
    match mine(h) {
        Some(w) if w.parent != 0 => own(w.parent),
        _ => h,
    }
}

#[no_mangle]
pub unsafe extern "system" fn GetClassNameW(h: HWND, to: *mut u16, n: i32) -> i32 {
    match who(h) {
        Who::Own(id) => win(id).map_or(0, |w| put(&w.class, to, n)),
        Who::Peer(pid, id) => peer(pid, id).map_or(0, |p| put(&p.class, to, n)),
        Who::Other(id) => other(id).map_or(0, |o| put(&o.class, to, n)),
        Who::Nobody => 0,
    }
}

unsafe fn text_of(h: HWND) -> String {
    match who(h) {
        Who::Own(id) => win(id).map_or(String::new(), |w| w.edit.as_ref().map_or_else(|| w.title.clone(), |e| e.text())),
        Who::Peer(pid, id) => peer(pid, id).map_or(String::new(), |p| p.title),
        Who::Other(id) => other(id).map_or(String::new(), |o| o.title.clone()),
        Who::Nobody => String::new(),
    }
}

#[no_mangle]
pub unsafe extern "system" fn GetWindowTextW(h: HWND, to: *mut u16, n: i32) -> i32 {
    put(&text_of(h), to, n)
}

#[no_mangle]
pub unsafe extern "system" fn InternalGetWindowText(h: HWND, to: *mut u16, n: i32) -> i32 {
    put(&text_of(h), to, n)
}

#[no_mangle]
pub unsafe extern "system" fn GetWindowTextLengthW(h: HWND) -> i32 {
    text_of(h).encode_utf16().count() as i32
}

#[no_mangle]
pub unsafe extern "system" fn SetWindowTextW(h: HWND, t: *const u16) -> BOOL {
    let text = string(t);
    let Some(w) = mine(h) else { return 0 };
    let id = w.id;
    match &mut w.edit {
        Some(e) => {
            e.set(&text);
            let single = !e.is_multi();
            edit_redraw(id);
            if single {
                edit_changed(id);
            }
        }
        None => w.title = text,
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetWindowThreadProcessId(h: HWND, pid: *mut u32) -> u32 {
    let p = match who(h) {
        Who::Own(_) => std::process::id(),
        Who::Peer(pid, _) => pid,
        Who::Other(id) => other(id).map_or(0, |o| o.pid),
        Who::Nobody => 0,
    };
    if !pid.is_null() {
        *pid = p;
    }
    (p != 0) as u32
}

#[no_mangle]
pub unsafe extern "system" fn SetPropW(h: HWND, name: *const u16, v: HANDLE) -> BOOL {
    let Some(w) = mine(h) else { return 0 };
    w.props.insert(string(name), v as isize);
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetPropW(h: HWND, name: *const u16) -> HANDLE {
    let name = string(name);
    let v = match who(h) {
        Who::Own(id) => win(id).and_then(|w| w.props.get(&name).copied()),
        Who::Peer(pid, id) => peer(pid, id).and_then(|p| p.props.get(&name).copied()),
        _ => None,
    };
    v.unwrap_or(0) as HANDLE
}

#[no_mangle]
pub unsafe extern "system" fn EnumWindows(f: WNDENUMPROC, lp: LPARAM) -> BOOL {
    let Some(f) = f else { return 0 };
    let mut list: Vec<HWND> = peers::all().iter().map(of_peer).collect();
    list.extend(g().wins.iter().filter(|w| w.parent == 0).map(|w| own(w.id)));
    list.extend(others().iter().map(|o| of_other(o.id)));
    for h in list {
        if f(h, lp) == 0 {
            return 0;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn WindowFromPoint(p: POINT) -> HWND {
    let inside = |r: &RECT| p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom;
    if let Some(w) = g().wins.iter().find(|w| w.parent == 0 && w.visible && w.key.is_none() && inside(&w.rect)) {
        return own(w.id);
    }
    others().iter().find(|o| inside(&o.rect)).map_or(std::ptr::null_mut(), |o| of_other(o.id))
}

#[no_mangle]
pub unsafe extern "system" fn DwmGetWindowAttribute(h: HWND, what: u32, to: *mut std::ffi::c_void, _: u32) -> i32 {
    match what {
        9 => if GetWindowRect(h, to as *mut RECT) != 0 { 0 } else { -1 },
        14 => {
            *(to as *mut u32) = 0;
            0
        }
        _ => -1,
    }
}

// ---- who is in front, and who is listening ----

#[no_mangle]
pub unsafe extern "system" fn GetForegroundWindow() -> HWND {
    if let Some(id) = g().screen.focused() {
        return own(id);
    }
    match g().screen.active() {
        Some(id) if other(id).is_some() => of_other(id),
        _ => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "system" fn SetForegroundWindow(h: HWND) -> BOOL {
    let Some(w) = mine(h) else { return 0 };
    let id = w.id;
    w.wants = w.opening;
    g().screen.raise(id);
    g().screen.focus(id);
    if win(g().focus).is_none_or(|f| f.id != id && f.parent != id) {
        g().focus = id;
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn SetFocus(h: HWND) -> HWND {
    let Some(w) = mine(h) else { return std::ptr::null_mut() };
    let (id, top) = (w.id, if w.parent != 0 { w.parent } else { w.id });
    let was = std::mem::replace(&mut g().focus, id);
    if let Some(w) = win(top).filter(|w| w.visible) {
        w.wants = w.opening;
        g().screen.focus(top);
    }
    for i in [was, id] {
        if i != id || was != id {
            edit_redraw(i);
        }
    }
    own(was)
}

#[no_mangle]
pub unsafe extern "system" fn SetCapture(h: HWND) -> HWND {
    let was = std::mem::replace(&mut g().capture, h as usize as u32);
    own(was)
}

#[no_mangle]
pub unsafe extern "system" fn ReleaseCapture() -> BOOL {
    g().capture = 0;
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetCursorPos(p: *mut POINT) -> BOOL {
    let (x, y) = g().screen.cursor();
    *p = POINT { x, y };
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetKeyState(vk: i32) -> i16 {
    if g().keys[(vk & 255) as usize] { i16::MIN } else { 0 }
}

#[no_mangle]
pub unsafe extern "system" fn GetAsyncKeyState(vk: i32) -> i16 {
    if g().screen.key_down(vk as u16) { i16::MIN } else { 0 }
}

#[no_mangle]
pub unsafe extern "system" fn GetLastInputInfo(li: *mut LASTINPUTINFO) -> BOOL {
    let idle = g().screen.idle_ms();
    (*li).dwTime = os::GetTickCount().wrapping_sub(idle);
    1
}

#[no_mangle]
pub unsafe extern "system" fn RegisterHotKey(_: HWND, id: i32, _: HOT_KEY_MODIFIERS, vk: u32) -> BOOL {
    g().screen.hotkey(id, Some(vk as u16)) as BOOL
}

#[no_mangle]
pub unsafe extern "system" fn UnregisterHotKey(_: HWND, id: i32) -> BOOL {
    g().screen.hotkey(id, None);
    1
}

#[no_mangle]
pub unsafe extern "system" fn TrackMouseEvent(t: *mut TRACKMOUSEEVENT) -> BOOL {
    if let Some(w) = mine((*t).hwndTrack) {
        w.tracking = true;
    }
    1
}

// ---- monitors ----

unsafe fn monitors() -> Vec<Monitor> {
    g().screen.monitors()
}

fn handle(i: usize) -> HMONITOR {
    (i + 1) as HMONITOR
}

#[no_mangle]
pub unsafe extern "system" fn EnumDisplayMonitors(_: HDC, _: *const RECT, f: MONITORENUMPROC, lp: LPARAM) -> BOOL {
    let Some(f) = f else { return 0 };
    for (i, m) in monitors().iter().enumerate() {
        let mut r = m.full;
        if f(handle(i), std::ptr::null_mut(), &mut r, lp) == 0 {
            break;
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetMonitorInfoW(m: HMONITOR, mi: *mut MONITORINFO) -> BOOL {
    let list = monitors();
    let Some(f) = list.get((m as usize).wrapping_sub(1)).or(list.first()) else { return 0 };
    (*mi).rcMonitor = f.full;
    (*mi).rcWork = f.work;
    1
}

/// The monitor nearest to a point.
pub(crate) fn nearest(list: &[Monitor], x: i32, y: i32) -> usize {
    let far = |m: &Monitor| {
        let dx = (m.full.left - x).max(x - m.full.right + 1).max(0) as i64;
        let dy = (m.full.top - y).max(y - m.full.bottom + 1).max(0) as i64;
        dx * dx + dy * dy
    };
    (0..list.len()).min_by_key(|i| far(&list[*i])).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn MonitorFromPoint(p: POINT, _: MONITOR_FROM_FLAGS) -> HMONITOR {
    handle(nearest(&monitors(), p.x, p.y))
}

#[no_mangle]
pub unsafe extern "system" fn MonitorFromWindow(h: HWND, _: MONITOR_FROM_FLAGS) -> HMONITOR {
    let mut r = std::mem::zeroed();
    GetWindowRect(h, &mut r);
    handle(nearest(&monitors(), (r.left + r.right) / 2, (r.top + r.bottom) / 2))
}

#[no_mangle]
pub unsafe extern "system" fn GetDpiForMonitor(m: HMONITOR, _: i32, x: *mut u32, y: *mut u32) -> i32 {
    let list = monitors();
    let dpi = list.get((m as usize).wrapping_sub(1)).map_or(96, |m| m.dpi);
    (*x, *y) = (dpi, dpi);
    0
}

#[no_mangle]
pub unsafe extern "system" fn GetDpiForWindow(h: HWND) -> u32 {
    let list = monitors();
    list.get((MonitorFromWindow(h, 2) as usize).wrapping_sub(1)).map_or(96, |m| m.dpi)
}

// ---- painting ----

#[no_mangle]
pub unsafe extern "system" fn InvalidateRect(h: HWND, r: *const RECT, _: BOOL) -> BOOL {
    let Who::Own(id) = who(h) else { return 0 };
    let r = if r.is_null() { None } else { Some(*r) };
    if !on_thread() {
        QUEUE.lock().unwrap().push_back((id as usize, u32::MAX, 0, 0));
        return 1;
    }
    if win(id).is_some_and(|w| w.parent != 0) {
        edit_redraw(id);
    } else {
        invalidate(id, r);
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn UpdateWindow(h: HWND) -> BOOL {
    if let Who::Own(id) = who(h) {
        paint(id);
    }
    1
}

/// Have a window painted if it needs it.
unsafe fn paint(id: u32) {
    let wanted = win(id).is_some_and(|w| w.parent == 0 && w.dirty.is_some());
    if wanted {
        call(id, WM_PAINT, 0, 0);
        if let Some(w) = win(id).filter(|w| w.opening && w.visible) {
            w.opening = false;
            let wants = std::mem::take(&mut w.wants);
            g().screen.show(id, true);
            g().screen.raise(id);
            present(id);
            if wants {
                g().screen.focus(id);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn BeginPaint(h: HWND, ps: *mut PAINTSTRUCT) -> HDC {
    let Some(w) = mine(h) else { return std::ptr::null_mut() };
    let all = RECT { left: 0, top: 0, right: w.rect.right - w.rect.left, bottom: w.rect.bottom - w.rect.top };
    let d = w.dirty.take().unwrap_or(all);
    let r = RECT { left: d.left.max(0), top: d.top.max(0), right: d.right.min(all.right), bottom: d.bottom.min(all.bottom) };
    let dc = gdi::dc_on(w.picture, (0, 0), r);
    (*ps).hdc = dc;
    (*ps).rcPaint = r;
    dc
}

#[no_mangle]
pub unsafe extern "system" fn EndPaint(h: HWND, ps: *const PAINTSTRUCT) -> BOOL {
    gdi::DeleteDC((*ps).hdc);
    let Who::Own(id) = who(h) else { return 0 };
    let children: Vec<u32> = g().wins.iter().filter(|w| w.parent == id).map(|w| w.id).collect();
    for c in children {
        edit_draw(c);
    }
    present(id);
    1
}

/// Put a window's picture on the screen.
unsafe fn present(id: u32) {
    let Some(w) = win(id) else { return };
    if !w.visible || w.parent != 0 || w.opening {
        return;
    }
    let (key, alpha, picture) = (w.key, w.alpha, w.picture);
    if let Some(b) = gdi::bitmap(picture) {
        g().screen.present(id, b, key, alpha);
    }
}

// ---- text boxes ----

unsafe fn edit_font(id: u32) -> gdi::Font {
    gdi::font(win(id).map_or(0, |w| w.font) as HFONT)
}

unsafe fn edit_size(id: u32) -> (i32, i32) {
    win(id).map_or((1, 1), |w| (w.rect.right - w.rect.left, w.rect.bottom - w.rect.top))
}

/// Draw it into its parent's picture.
unsafe fn edit_draw(id: u32) {
    let Some(w) = win(id) else { return };
    let (parent, r, readonly) = (w.parent, w.rect, w.edit.as_ref().is_some_and(|e| e.readonly));
    if !w.visible || w.edit.is_none() {
        return;
    }
    let Some(p) = win(parent) else { return };
    let dc = gdi::dc_on(p.picture, (r.left, r.top), r);
    call(parent, if readonly { 0x0138 } else { 0x0133 }, dc as usize, id as isize); // WM_CTLCOLORSTATIC, WM_CTLCOLOREDIT
    let (f, focused) = (edit_font(id), g().focus == id);
    if let Some(e) = win(id).and_then(|w| w.edit.as_ref()) {
        e.draw(dc, f, r.right - r.left, r.bottom - r.top, focused);
    }
    gdi::DeleteDC(dc);
}

unsafe fn edit_redraw(id: u32) {
    let Some(parent) = win(id).filter(|w| w.edit.is_some()).map(|w| w.parent) else { return };
    if win(parent).is_some_and(|p| p.visible) {
        edit_draw(id);
        present(parent);
    }
}

unsafe fn edit_changed(id: u32) {
    if let Some(w) = win(id) {
        call(w.parent, WM_COMMAND, 0x0300 << 16, id as isize); // EN_CHANGE
    }
}

unsafe fn edit_proc(id: u32, msg: u32, wp: usize, lp: isize) -> isize {
    let (f, (w, h)) = (edit_font(id), edit_size(id));
    let (shift, ctrl) = (g().keys[VK_SHIFT as usize], g().keys[VK_CONTROL as usize]);
    let Some(win_) = win(id) else { return 0 };
    let font = win_.font;
    let Some(e) = win_.edit.as_mut() else { return 0 };
    let (mut changed, mut look) = (false, true);
    let (cols, rows) = Edit::size(&f, w, h);
    let (x, y) = ((lp & 0xffff) as i16 as i32, (lp >> 16 & 0xffff) as i16 as i32);
    match msg {
        WM_SETFONT => win_.font = wp as isize,
        WM_GETFONT => return font,
        WM_CHAR => match char::from_u32(wp as u32) {
            Some('\u{1}') => e.select(0, -1),
            Some('\u{3}') => return edit_proc(id, WM_COPY, 0, 0),
            Some('\u{18}') => {
                edit_proc(id, WM_COPY, 0, 0);
                if let Some(e) = win(id).and_then(|w| w.edit.as_mut()).filter(|e| !e.readonly && !e.selected().is_empty()) {
                    changed = e.insert("");
                }
            }
            Some('\u{16}') => {
                let t = g().screen.clipboard();
                if let Some(e) = win(id).and_then(|w| w.edit.as_mut()).filter(|e| !e.readonly) {
                    changed = e.insert(&t);
                }
            }
            Some(c) => changed = e.typed(c),
            None => {}
        },
        WM_KEYDOWN => (look, changed) = e.key(wp as u16, shift, ctrl, cols, rows),
        WM_COPY => {
            let t = e.selected();
            if !t.is_empty() {
                g().screen.set_clipboard(&t);
            }
            return 0;
        }
        0x00B1 => e.select(wp as isize, lp), // EM_SETSEL
        0x00C2 => changed = e.insert(&string(lp as *const u16)), // EM_REPLACESEL
        0x00CF => e.readonly = wp != 0, // EM_SETREADONLY
        0x00C5 => {
            e.limit(wp); // EM_SETLIMITTEXT
            return 0;
        }
        WM_LBUTTONDOWN => {
            let at = e.at(x, y, &f, w, h);
            e.click(at, shift);
            g().selecting = id;
            SetFocus(own(id));
        }
        WM_MOUSEMOVE if g().selecting == id => {
            let at = e.at(x, y, &f, w, h);
            e.click(at, true);
        }
        WM_LBUTTONUP => {
            g().selecting = 0;
            return 0;
        }
        WM_MOUSEWHEEL => {
            e.wheel((wp >> 16) as i16 > 0, &f, w, h);
            edit_redraw(id);
            return 0;
        }
        _ => return 0,
    }
    if look || changed {
        if let Some(e) = win(id).and_then(|w| w.edit.as_mut()) {
            e.follow(&f, w, h);
        }
        edit_redraw(id);
    }
    if changed {
        edit_changed(id);
    }
    0
}

// ---- messages ----

#[no_mangle]
pub unsafe extern "system" fn SendMessageW(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match who(h) {
        Who::Own(id) => call(id, msg, wp, lp),
        Who::Peer(pid, id) => peers::send(pid, id, msg, wp, lp) as LRESULT,
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn PostMessageW(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> BOOL {
    match who(h) {
        Who::Own(id) => {
            QUEUE.lock().unwrap().push_back((id as usize, msg, wp, lp));
            1
        }
        Who::Peer(pid, id) => peers::send(pid, id, msg, wp, lp) as BOOL,
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn PostQuitMessage(code: i32) {
    g().quit = Some(code);
}

#[no_mangle]
pub unsafe extern "system" fn SetTimer(h: HWND, id: usize, ms: u32, _: TIMERPROC) -> usize {
    let Who::Own(win) = who(h) else { return 0 };
    let t = &mut g().timers;
    t.retain(|t| t.win != win || t.id != id);
    t.push(Timer { win, id, every: ms.max(5), due: Instant::now() + Duration::from_millis(ms as u64) });
    id
}

#[no_mangle]
pub unsafe extern "system" fn DefWindowProcW(h: HWND, msg: u32, _: WPARAM, _: LPARAM) -> LRESULT {
    match msg {
        WM_CLOSE => {
            DestroyWindow(h);
            0
        }
        WM_NCHITTEST => 1,
        WM_PAINT => {
            if let Some(w) = mine(h) {
                w.dirty = None;
            }
            0
        }
        _ => 0,
    }
}

/// Let other pets know where things are, now and then.
unsafe fn tell(now: bool) {
    let s = g();
    if !now && s.told.elapsed() < Duration::from_millis(30) {
        return;
    }
    s.told = Instant::now();
    let list: Vec<Peer> = s
        .wins
        .iter()
        .filter(|w| w.parent == 0)
        .map(|w| Peer { pid: std::process::id(), id: w.id, class: w.class.clone(), title: w.title.clone(), visible: w.visible, rect: (w.rect.left, w.rect.top, w.rect.right, w.rect.bottom), props: w.props.clone() })
        .collect();
    s.post.publish(&list);
}

unsafe fn message(m: *mut MSG, id: u32, msg: u32, wp: usize, lp: isize) {
    *m = std::mem::zeroed();
    (*m).hwnd = own(id);
    (*m).message = msg;
    (*m).wParam = wp;
    (*m).lParam = lp;
}

fn pack(x: i32, y: i32) -> isize {
    ((y as u16 as u32) << 16 | x as u16 as u32) as i32 as isize
}

/// Turn something that happened into a message. None if nobody needs one.
unsafe fn translate(e: Ev) -> Option<(u32, u32, usize, isize)> {
    match e {
        Ev::Mouse { win: id, what, x, y, at } => mouse(id, what, x, y, at),
        Ev::Leave(id) => {
            let w = win(id)?;
            std::mem::take(&mut w.tracking).then_some((id, 0x02A3, 0, 0)) // WM_MOUSELEAVE
        }
        Ev::Key { vk, down, text } => {
            let s = g();
            s.keys[(vk & 255) as usize] = down;
            s.typed = text;
            let to = s.focus;
            win(to)?;
            Some((to, if down { WM_KEYDOWN } else { WM_KEYUP }, vk as usize, 0))
        }
        Ev::Hot(id) => {
            let pet = g().wins.first()?.id; // the pet's window is the first there is
            Some((pet, WM_HOTKEY, id as usize, 0))
        }
        Ev::Exposed(id) => {
            present(id);
            None
        }
        Ev::Screens => {
            let pet = g().wins.first()?.id;
            Some((pet, WM_DISPLAYCHANGE, 0, 0))
        }
        Ev::Close(id) => Some((id, WM_CLOSE, 0, 0)),
        Ev::Dropped(id, files) => {
            g().drops.push(files);
            Some((id, WM_DROPFILES, g().drops.len(), 0))
        }
        Ev::Tray(msg) => {
            let pet = g().wins.first()?.id;
            Some((pet, WM_APP + 1, 0, msg as isize))
        }
    }
}

unsafe fn mouse(id: u32, what: Mouse, x: i32, y: i32, at: (i32, i32)) -> Option<(u32, u32, usize, isize)> {
    // a window that is being dragged by its title
    if let Some((w, (gx, gy))) = g().moving {
        match what {
            Mouse::Move => {
                SetWindowPos(own(w), std::ptr::null_mut(), at.0 - gx, at.1 - gy, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
            Mouse::Up(1) => g().moving = None,
            _ => {}
        }
        return None;
    }
    let s = g();
    let to = if s.capture != 0 { s.capture } else { id };
    let me = win(to)?;
    let (mut x, mut y) = if s.capture != 0 && to != id { (at.0 - me.rect.left, at.1 - me.rect.top) } else { (x, y) };
    if what == Mouse::Down(1) && s.capture == 0 {
        if me.ex & WS_EX_NOACTIVATE == 0 {
            SetForegroundWindow(own(to));
        }
        if call(to, WM_NCHITTEST, 0, pack(at.0, at.1)) == 2 {
            let r = win(to)?.rect;
            g().moving = Some((to, (at.0 - r.left, at.1 - r.top)));
            return None;
        }
    }
    // a text box in it
    let mut target = to;
    if s.capture == 0 {
        let inside = g().wins.iter().find(|w| w.parent == to && w.visible && ((x >= w.rect.left && x < w.rect.right && y >= w.rect.top && y < w.rect.bottom) || g().selecting == w.id));
        if let Some(c) = inside {
            target = c.id;
            (x, y) = (x - c.rect.left, y - c.rect.top);
        }
    }
    let (msg, wp) = match what {
        Mouse::Move => (WM_MOUSEMOVE, 0),
        Mouse::Down(1) => (WM_LBUTTONDOWN, 1),
        Mouse::Up(1) => (WM_LBUTTONUP, 0),
        Mouse::Down(3) => (WM_RBUTTONDOWN, 2),
        Mouse::Up(3) => (WM_RBUTTONUP, 0),
        Mouse::Wheel(n) => (WM_MOUSEWHEEL, ((n * 120) as u16 as usize) << 16),
        _ => return None,
    };
    Some((target, msg, wp, pack(x, y)))
}

#[no_mangle]
pub unsafe extern "system" fn GetMessageW(m: *mut MSG, _: HWND, _: u32, _: u32) -> BOOL {
    loop {
        if let Some(code) = g().quit {
            message(m, 0, WM_QUIT, code as usize, 0);
            G = None; // the screen and what others can see of us go away
            return 0;
        }
        let posted = QUEUE.lock().unwrap().pop_front();
        if let Some((id, msg, wp, lp)) = posted {
            if msg == u32::MAX {
                invalidate(id as u32, None);
                continue;
            }
            message(m, id as u32, msg, wp, lp);
            return 1;
        }
        for (id, msg, wp, lp) in g().post.take() {
            QUEUE.lock().unwrap().push_back((id as usize, msg, wp, lp));
        }
        // what happened on the screen
        if let Some(e) = g().screen.next() {
            if let Some((id, msg, wp, lp)) = translate(e) {
                message(m, id, msg, wp, lp);
                return 1;
            }
            continue;
        }
        if !QUEUE.lock().unwrap().is_empty() {
            continue;
        }
        // then painting, then timers
        if let Some(id) = g().wins.iter().find(|w| w.parent == 0 && w.visible && w.dirty.is_some()).map(|w| w.id) {
            message(m, id, WM_PAINT, 0, 0);
            return 1;
        }
        let now = Instant::now();
        if let Some(t) = g().timers.iter_mut().filter(|t| t.due <= now).min_by_key(|t| t.due) {
            t.due = now + Duration::from_millis(t.every as u64);
            let (id, n) = (t.win, t.id);
            message(m, id, WM_TIMER, n, 0);
            return 1;
        }
        tell(false);
        let wait = g().timers.iter().map(|t| t.due.saturating_duration_since(now)).min().unwrap_or(Duration::from_millis(50));
        g().screen.wait(wait.min(Duration::from_millis(50)));
    }
}

#[no_mangle]
pub unsafe extern "system" fn TranslateMessage(m: *const MSG) -> BOOL {
    if (*m).message == WM_KEYDOWN {
        let t = std::mem::take(&mut g().typed);
        let mut q = QUEUE.lock().unwrap();
        for c in t.chars().rev() {
            q.push_front(((*m).hwnd as usize, WM_CHAR, c as usize, 0));
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "system" fn DispatchMessageW(m: *const MSG) -> LRESULT {
    let Who::Own(id) = who((*m).hwnd) else { return 0 };
    if (*m).message == WM_PAINT {
        paint(id);
        return 0;
    }
    call(id, (*m).message, (*m).wParam, (*m).lParam)
}

// ---- clipboard, and things dropped on the pet ----

/// Another number when something else has been copied.
pub(crate) unsafe fn clip_count() -> u32 {
    g().screen.clip_count()
}

/// What was copied, unless it was marked as secret.
pub(crate) unsafe fn clip_text() -> Option<String> {
    let s = &mut g().screen;
    if s.clip_secret() {
        return None;
    }
    Some(s.clipboard()).filter(|t| !t.is_empty())
}

pub(crate) unsafe fn clip_set(t: &str) {
    g().screen.set_clipboard(t);
}

#[no_mangle]
pub unsafe extern "system" fn OpenClipboard(_: HWND) -> BOOL {
    1
}

#[no_mangle]
pub unsafe extern "system" fn CloseClipboard() -> BOOL {
    1
}

#[no_mangle]
pub unsafe extern "system" fn IsClipboardFormatAvailable(f: u32) -> BOOL {
    (f == 13 && !g().screen.clipboard().is_empty()) as BOOL
}

#[no_mangle]
pub unsafe extern "system" fn GetClipboardData(f: u32) -> HANDLE {
    if f != 13 {
        return std::ptr::null_mut();
    }
    let t = g().screen.clipboard();
    g().clip = t.encode_utf16().chain([0]).collect();
    g().clip.as_ptr() as HANDLE
}

#[no_mangle]
pub unsafe extern "system" fn GlobalLock(h: HGLOBAL) -> *mut std::ffi::c_void {
    h
}

#[no_mangle]
pub unsafe extern "system" fn GlobalUnlock(_: HGLOBAL) -> BOOL {
    1
}

#[no_mangle]
pub unsafe extern "system" fn GlobalSize(_: HGLOBAL) -> usize {
    g().clip.len() * 2
}

#[no_mangle]
pub unsafe extern "system" fn DragAcceptFiles(h: HWND, on: BOOL) {
    if let Who::Own(id) = who(h) {
        g().screen.accept_files(id, on != 0);
    }
}

#[no_mangle]
pub unsafe extern "system" fn DragQueryFileW(d: *mut std::ffi::c_void, i: u32, to: *mut u16, n: u32) -> u32 {
    let Some(list) = g().drops.get((d as usize).wrapping_sub(1)) else { return 0 };
    if i == u32::MAX {
        return list.len() as u32;
    }
    list.get(i as usize).map_or(0, |f| put(f, to, n as i32) as u32)
}

#[no_mangle]
pub unsafe extern "system" fn DragFinish(d: *mut std::ffi::c_void) {
    if d as usize == g().drops.len() {
        g().drops.pop();
    }
}

// ---- the icon beside the clock ----

#[no_mangle]
pub unsafe extern "system" fn Shell_NotifyIconW(what: u32, d: *const windows_sys::Win32::UI::Shell::NOTIFYICONDATAW) -> BOOL {
    let tip = String::from_utf16_lossy(&(*d).szTip.iter().copied().take_while(|c| *c != 0).collect::<Vec<_>>());
    g().screen.tray(what != 2, &tip) as BOOL
}

// ---- nothing to do here ----

#[no_mangle]
pub unsafe extern "system" fn LoadCursorW(_: HINSTANCE, _: *const u16) -> HCURSOR {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "system" fn LoadIconW(_: HINSTANCE, _: *const u16) -> HICON {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "system" fn RegisterWindowMessageW(_: *const u16) -> u32 {
    0
}

#[no_mangle]
pub unsafe extern "system" fn SetProcessDPIAware() -> BOOL {
    1
}

#[no_mangle]
pub unsafe extern "system" fn SetProcessDpiAwarenessContext(_: *mut std::ffi::c_void) -> BOOL {
    1
}
