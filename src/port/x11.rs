//! Windows on an X server: Linux and the BSDs, and Wayland desktops through XWayland.

use super::gdi::Bitmap;
use super::*;
use std::collections::VecDeque;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{Atom, ConnectionExt as _, *};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        _NET_WM_NAME, _NET_WM_PID, _NET_WM_STATE, _NET_WM_STATE_HIDDEN, _NET_WM_STATE_FULLSCREEN, _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_WINDOW_TYPE, _NET_WM_WINDOW_TYPE_DOCK, _NET_WM_WINDOW_TYPE_UTILITY, _NET_WM_DESKTOP, _NET_CURRENT_DESKTOP,
        _NET_WM_WINDOW_OPACITY, _NET_ACTIVE_WINDOW, _NET_CLIENT_LIST_STACKING, _NET_CLIENT_LIST, _NET_FRAME_EXTENTS, _NET_WORKAREA,
        UTF8_STRING, CLIPBOARD, TARGETS, PIXEL_PET_CLIP,
        XdndAware, XdndEnter, XdndPosition, XdndStatus, XdndDrop, XdndFinished, XdndActionCopy, XdndSelection,
        URI_LIST: b"text/uri-list",
    }
}

struct Own {
    id: u32,
    x: Window,
    rect: RECT,
    shown: bool,
    through: bool,
    argb: bool,
    last: Vec<u32>, // what is on the screen now
    size: (i32, i32),
    shape: u64,
}

pub(crate) struct Screen {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
    argb: Option<(Visualid, Colormap)>,
    depth: u8,
    gcs: [Option<Gcontext>; 2],
    own: Vec<Own>,
    queue: VecDeque<Ev>,
    keys: (u8, u8, Vec<Keysym>), // first keycode, symbols for each, all of them
    hot: Vec<(i32, Keycode)>,
    clip: String,
    mons: std::cell::RefCell<(Option<Instant>, Vec<Monitor>)>,
    dpi: u32,
    drag: (Window, u32), // who is dragging something over us, and over which window
    idle: bool,
}

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

extern "C" {
    fn poll(fds: *mut PollFd, n: usize, ms: i32) -> i32;
}

fn r(left: i32, top: i32, w: i32, h: i32) -> RECT {
    RECT { left, top, right: left + w, bottom: top + h }
}

/// The visible part of a picture as few rectangles: rows that look the same share theirs.
pub(crate) fn spans(px: &[u32], w: i32, key: u32) -> Vec<(i32, i32, i32, i32)> {
    let mut out: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut open: Vec<(i32, i32, i32)> = Vec::new(); // (left, right, first row) of the rows above
    let h = px.len() as i32 / w.max(1);
    for y in 0..=h {
        let mut row = Vec::new();
        if y < h {
            let line = &px[(y * w) as usize..((y + 1) * w) as usize];
            let mut x = 0;
            while x < w {
                if line[x as usize] == key {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < w && line[x as usize] != key {
                    x += 1;
                }
                row.push((start, x));
            }
        }
        let same = open.len() == row.len() && open.iter().zip(&row).all(|(o, n)| (o.0, o.1) == *n);
        if !same {
            out.extend(open.iter().map(|(l, rr, t)| (*l, *t, rr - l, y - t)));
            open = row.iter().map(|(l, rr)| (*l, *rr, y)).collect();
        }
    }
    out
}

fn hash(v: &[(i32, i32, i32, i32)], extra: u64) -> u64 {
    v.iter().fold(0xcbf2_9ce4_8422_2325 ^ extra, |h, r| [r.0, r.1, r.2, r.3].iter().fold(h, |h, n| (h ^ *n as u32 as u64).wrapping_mul(0x0100_0000_01b3)))
}

/// The file names in what a file manager drops.
pub(crate) fn files(list: &str) -> Vec<String> {
    let hex = |b: u8| (b as char).to_digit(16);
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let Some(p) = l.strip_prefix("file://") else { return l.to_string() };
            let p = &p[p.find('/').unwrap_or(0)..];
            let (b, mut out, mut i) = (p.as_bytes(), Vec::new(), 0);
            while i < b.len() {
                match (b[i], b.get(i + 1).and_then(|c| hex(*c)), b.get(i + 2).and_then(|c| hex(*c))) {
                    (b'%', Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    (c, ..) => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            String::from_utf8_lossy(&out).into_owned()
        })
        .collect()
}

/// What Windows calls the key with this symbol.
pub(crate) fn vk_of(sym: Keysym) -> u16 {
    match sym {
        0x30..=0x39 => sym as u16,
        0x41..=0x5a => sym as u16,
        0x61..=0x7a => sym as u16 - 32,
        0x20 => VK_SPACE,
        0xff08 => VK_BACK,
        0xff09 | 0xfe20 => VK_TAB,
        0xff0d | 0xff8d => VK_RETURN,
        0xff1b => VK_ESCAPE,
        0xff50 | 0xff95 => VK_HOME,
        0xff51 | 0xff96 => VK_LEFT,
        0xff52 | 0xff97 => VK_UP,
        0xff53 | 0xff98 => VK_RIGHT,
        0xff54 | 0xff99 => VK_DOWN,
        0xff55 | 0xff9a => VK_PRIOR,
        0xff56 | 0xff9b => VK_NEXT,
        0xff57 | 0xff9c => VK_END,
        0xffff | 0xff9f => VK_DELETE,
        0xffb0..=0xffb9 => VK_NUMPAD0 + (sym - 0xffb0) as u16,
        0xffe1 | 0xffe2 => VK_SHIFT,
        0xffe3 | 0xffe4 => VK_CONTROL,
        0xffe9 | 0xffea | 0xffe7 | 0xffe8 => VK_MENU,
        _ => 0,
    }
}

/// The character a key symbol stands for.
pub(crate) fn char_of(sym: Keysym) -> Option<char> {
    match sym {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(sym),
        0x0100_0100..=0x0110_ffff => char::from_u32(sym - 0x0100_0000),
        0xffb0..=0xffb9 => char::from_u32(sym - 0xffb0 + 0x30),
        0xffaa => Some('*'),
        0xffab => Some('+'),
        0xffad => Some('-'),
        0xffae | 0xffac => Some('.'),
        0xffaf => Some('/'),
        0x20ac => Some('\u{20ac}'),
        _ => None,
    }
}

impl Screen {
    pub fn open() -> Result<Screen, String> {
        let (conn, n) = RustConnection::connect(None).map_err(|e| format!("{e}. Is DISPLAY set?"))?;
        let setup = conn.setup().clone();
        let root = setup.roots[n].root;
        let err = |e: &dyn std::fmt::Display| e.to_string();
        let atoms = Atoms::new(&conn).map_err(|e| err(&e))?.reply().map_err(|e| err(&e))?;
        // see-through windows need somebody to put them together
        let cm = conn.intern_atom(false, format!("_NET_WM_CM_S{n}").as_bytes()).ok().and_then(|c| c.reply().ok()).map_or(0, |r| r.atom);
        let composited = conn.get_selection_owner(cm).ok().and_then(|c| c.reply().ok()).is_some_and(|r| r.owner != 0);
        let visual = setup.roots[n].allowed_depths.iter().find(|d| d.depth == 32).and_then(|d| d.visuals.iter().find(|v| v.class == VisualClass::TRUE_COLOR && v.red_mask == 0xff0000)).map(|v| v.visual_id);
        let argb = visual.filter(|_| composited).and_then(|v| {
            let c = conn.generate_id().ok()?;
            conn.create_colormap(ColormapAlloc::NONE, c, root, v).ok()?;
            Some((v, c))
        });
        let _ = conn.randr_select_input(root, x11rb::protocol::randr::NotifyMask::SCREEN_CHANGE);
        let idle = conn.extension_information(x11rb::protocol::screensaver::X11_EXTENSION_NAME).ok().flatten().is_some();
        let mut s = Screen { conn, root, atoms, argb, depth: setup.roots[n].root_depth, gcs: [None; 2], own: Vec::new(), queue: VecDeque::new(), keys: (0, 0, Vec::new()), hot: Vec::new(), clip: String::new(), mons: Default::default(), dpi: 96, drag: (0, 0), idle };
        s.load_keys();
        s.dpi = s.wanted_dpi();
        Ok(s)
    }

    /// The text size somebody chose for their desktop.
    fn wanted_dpi(&self) -> u32 {
        let res = self.prop(self.root, AtomEnum::RESOURCE_MANAGER.into(), AtomEnum::STRING.into()).map(|v| String::from_utf8_lossy(&v).into_owned()).unwrap_or_default();
        let dpi = res.lines().find_map(|l| l.strip_prefix("Xft.dpi:")?.trim().parse::<f32>().ok());
        dpi.map_or(96, |d| d.round().clamp(72.0, 384.0) as u32)
    }

    fn load_keys(&mut self) {
        let (lo, hi) = (self.conn.setup().min_keycode, self.conn.setup().max_keycode);
        if let Some(m) = self.conn.get_keyboard_mapping(lo, hi - lo + 1).ok().and_then(|c| c.reply().ok()) {
            self.keys = (lo, m.keysyms_per_keycode, m.keysyms);
        }
    }

    fn syms(&self, code: Keycode) -> &[Keysym] {
        let (lo, per, all) = &self.keys;
        let i = code.saturating_sub(*lo) as usize * *per as usize;
        all.get(i..i + *per as usize).unwrap_or(&[])
    }

    fn codes_of(&self, vk: u16) -> Vec<Keycode> {
        let (lo, per, all) = &self.keys;
        (0..all.len() / (*per).max(1) as usize).map(|i| lo + i as u8).filter(|c| self.syms(*c).first().is_some_and(|s| vk_of(*s) == vk)).collect()
    }

    fn prop(&self, w: Window, name: Atom, kind: Atom) -> Option<Vec<u8>> {
        let p = self.conn.get_property(false, w, name, kind, 0, 1 << 16).ok()?.reply().ok()?;
        (!p.value.is_empty()).then_some(p.value)
    }

    fn numbers(&self, w: Window, name: Atom) -> Vec<u32> {
        self.prop(w, name, AtomEnum::ANY.into()).map_or(Vec::new(), |v| v.chunks_exact(4).map(|c| u32::from_ne_bytes(c.try_into().unwrap())).collect())
    }

    fn of(&mut self, id: u32) -> Option<&mut Own> {
        self.own.iter_mut().find(|o| o.id == id)
    }

    fn x_of(&self, id: u32) -> Option<Window> {
        self.own.iter().find(|o| o.id == id).map(|o| o.x)
    }

    fn by_x(&self, x: Window) -> Option<u32> {
        self.own.iter().find(|o| o.x == x).map(|o| o.id)
    }

    // ---- windows ----

    pub fn create(&mut self, m: &Make) {
        let Ok(x) = self.conn.generate_id() else { return };
        let (w, h) = ((m.rect.right - m.rect.left).max(1), (m.rect.bottom - m.rect.top).max(1));
        let mask = EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION | EventMask::LEAVE_WINDOW | EventMask::EXPOSURE | EventMask::KEY_PRESS | EventMask::KEY_RELEASE;
        let mut aux = CreateWindowAux::new().override_redirect(1).event_mask(mask).border_pixel(0).background_pixel(0);
        let (depth, visual) = match self.argb {
            Some((v, c)) => {
                aux = aux.colormap(c);
                (32, v)
            }
            None => (self.depth, 0),
        };
        let _ = self.conn.create_window(depth, x, self.root, m.rect.left as i16, m.rect.top as i16, w as u16, h as u16, 0, WindowClass::INPUT_OUTPUT, visual, &aux);
        let a = &self.atoms;
        let _ = self.conn.change_property8(PropMode::REPLACE, x, a._NET_WM_NAME, a.UTF8_STRING, m.title.as_bytes());
        let _ = self.conn.change_property8(PropMode::REPLACE, x, AtomEnum::WM_NAME, AtomEnum::STRING, m.title.as_bytes());
        let _ = self.conn.change_property8(PropMode::REPLACE, x, AtomEnum::WM_CLASS, AtomEnum::STRING, b"pixel-pet\0Pixel Pet\0");
        let _ = self.conn.change_property32(PropMode::REPLACE, x, a._NET_WM_PID, AtomEnum::CARDINAL, &[std::process::id()]);
        let kind = if m.takes_focus { a._NET_WM_WINDOW_TYPE_UTILITY } else { a._NET_WM_WINDOW_TYPE_DOCK };
        let _ = self.conn.change_property32(PropMode::REPLACE, x, a._NET_WM_WINDOW_TYPE, AtomEnum::ATOM, &[kind]);
        self.own.push(Own { id: m.id, x, rect: r(m.rect.left, m.rect.top, w, h), shown: false, through: m.through, argb: self.argb.is_some(), last: Vec::new(), size: (0, 0), shape: 0 });
    }

    pub fn destroy(&mut self, id: u32) {
        if let Some(x) = self.x_of(id) {
            let _ = self.conn.destroy_window(x);
            let _ = self.conn.flush();
        }
        self.own.retain(|o| o.id != id);
    }

    pub fn show(&mut self, id: u32, on: bool) {
        let Some(o) = self.of(id) else { return };
        o.shown = on;
        let x = o.x;
        let _ = if on { self.conn.map_window(x) } else { self.conn.unmap_window(x) };
        let _ = self.conn.flush();
    }

    pub fn place(&mut self, id: u32, to: RECT) {
        let Some(o) = self.of(id) else { return };
        let (w, h) = ((to.right - to.left).max(1), (to.bottom - to.top).max(1));
        let resized = (w, h) != (o.rect.right - o.rect.left, o.rect.bottom - o.rect.top);
        o.rect = r(to.left, to.top, w, h);
        let x = o.x;
        let mut aux = ConfigureWindowAux::new().x(to.left).y(to.top);
        if resized {
            o.last.clear();
            aux = aux.width(w as u32).height(h as u32);
        }
        let _ = self.conn.configure_window(x, &aux);
        let _ = self.conn.flush();
    }

    pub fn raise(&mut self, id: u32) {
        if let Some(x) = self.x_of(id) {
            let _ = self.conn.configure_window(x, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        }
    }

    pub fn lower(&mut self, _: u32) {}

    /// Put one of our windows just below another.
    pub fn below(&mut self, id: u32, other: u32) {
        if let (Some(x), Some(y)) = (self.x_of(id), self.x_of(other)) {
            let _ = self.conn.configure_window(x, &ConfigureWindowAux::new().sibling(y).stack_mode(StackMode::BELOW));
        }
    }

    pub fn pass_clicks(&mut self, id: u32, on: bool) {
        if let Some(o) = self.of(id) {
            o.through = on;
            o.shape = 0;
        }
    }

    pub fn present(&mut self, id: u32, b: &Bitmap, key: Option<u32>, alpha: u8) {
        let Some(i) = self.own.iter().position(|o| o.id == id) else { return };
        let (x, argb, through) = (self.own[i].x, self.own[i].argb, self.own[i].through);
        // what can be seen is also what can be clicked
        let parts = match key {
            Some(k) => spans(&b.px, b.w, k),
            None => vec![(0, 0, b.w, b.h)],
        };
        let h = hash(&parts, through as u64 + 1);
        if h != self.own[i].shape {
            self.own[i].shape = h;
            let rects: Vec<Rectangle> = parts.iter().map(|p| Rectangle { x: p.0 as i16, y: p.1 as i16, width: p.2 as u16, height: p.3 as u16 }).collect();
            if !argb && key.is_some() {
                let _ = self.conn.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::YX_BANDED, x, 0, 0, &rects);
            }
            let input: &[Rectangle] = if through { &[] } else { &rects };
            let _ = self.conn.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, x, 0, 0, input);
        }
        if !argb {
            let o = alpha as u32 * 0x0101_0101;
            let _ = self.conn.change_property32(PropMode::REPLACE, x, self.atoms._NET_WM_WINDOW_OPACITY, AtomEnum::CARDINAL, &[o]);
        }
        let shown = |p: u32| -> u32 {
            if !argb {
                return p & 0xff_ffff;
            }
            if key == Some(p & 0xff_ffff) {
                return 0;
            }
            let m = |s: u32| ((p >> s & 0xff) * alpha as u32 / 255) << s;
            (alpha as u32) << 24 | m(16) | m(8) | m(0)
        };
        let px: Vec<u32> = b.px.iter().map(|p| shown(*p)).collect();
        // only what is different from what is there
        let (mut l, mut t, mut rr, mut bt) = (b.w, b.h, 0, 0);
        let o = &mut self.own[i];
        if o.last.len() == px.len() && o.size == (b.w, b.h) {
            for y in 0..b.h {
                let (s, e) = ((y * b.w) as usize, ((y + 1) * b.w) as usize);
                if o.last[s..e] != px[s..e] {
                    let f = (s..e).position(|k| o.last[k] != px[k]).unwrap_or(0) as i32;
                    let z = (s..e).rposition(|k| o.last[k] != px[k]).unwrap_or(0) as i32 + 1;
                    (l, rr, t, bt) = (l.min(f), rr.max(z), t.min(y), y + 1);
                }
            }
        } else {
            (l, t, rr, bt) = (0, 0, b.w, b.h);
        }
        if rr > l {
            let gc = match self.gcs[argb as usize] {
                Some(g) => g,
                None => {
                    let Ok(g) = self.conn.generate_id() else { return };
                    let _ = self.conn.create_gc(g, x, &CreateGCAux::new());
                    self.gcs[argb as usize] = Some(g);
                    g
                }
            };
            let depth = if argb { 32 } else { self.depth };
            let w = rr - l;
            let most = ((self.conn.maximum_request_bytes() - 64) / (w as usize * 4)).clamp(1, 4096) as i32;
            let mut y = t;
            while y < bt {
                let n = most.min(bt - y);
                let mut data = Vec::with_capacity((w * n * 4) as usize);
                for row in y..y + n {
                    for p in &px[(row * b.w + l) as usize..(row * b.w + rr) as usize] {
                        data.extend(p.to_le_bytes());
                    }
                }
                let _ = self.conn.put_image(ImageFormat::Z_PIXMAP, x, gc, w as u16, n as u16, l as i16, y as i16, 0, depth, &data);
                y += n;
            }
        }
        let o = &mut self.own[i];
        o.last = px;
        o.size = (b.w, b.h);
        let _ = self.conn.flush();
    }

    // ---- who is in front ----

    pub fn focus(&mut self, id: u32) {
        if let Some(x) = self.own.iter().find(|o| o.id == id && o.shown).map(|o| o.x) {
            let _ = self.conn.set_input_focus(InputFocus::PARENT, x, 0u32);
            let _ = self.conn.flush();
        }
    }

    /// Which of our windows has the keyboard.
    pub fn focused(&self) -> Option<u32> {
        let f = self.conn.get_input_focus().ok()?.reply().ok()?.focus;
        self.by_x(f)
    }

    /// The window of another program that is in front.
    pub fn active(&self) -> Option<u64> {
        self.numbers(self.root, self.atoms._NET_ACTIVE_WINDOW).first().filter(|w| **w != 0).map(|w| *w as u64)
    }

    /// All windows of other programs that can be seen, topmost first.
    pub fn others(&self) -> Vec<Foreign> {
        let a = &self.atoms;
        let mut list = self.numbers(self.root, a._NET_CLIENT_LIST_STACKING);
        if list.is_empty() {
            list = self.numbers(self.root, a._NET_CLIENT_LIST);
        }
        let desk = self.numbers(self.root, a._NET_CURRENT_DESKTOP).first().copied();
        let text = |w: Window, p: Atom| self.prop(w, p, AtomEnum::ANY.into()).map(|v| String::from_utf8_lossy(&v).into_owned());
        let mut out = Vec::new();
        for w in list.into_iter().rev().take(60) {
            let state = self.numbers(w, a._NET_WM_STATE);
            let on = self.numbers(w, a._NET_WM_DESKTOP).first().copied();
            if state.contains(&a._NET_WM_STATE_HIDDEN) || (desk.is_some() && on.is_some_and(|d| d != u32::MAX) && on != desk) {
                continue;
            }
            let Some(g) = self.conn.get_geometry(w).ok().and_then(|c| c.reply().ok()) else { continue };
            let Some(p) = self.conn.translate_coordinates(w, self.root, 0, 0).ok().and_then(|c| c.reply().ok()) else { continue };
            let f = self.numbers(w, a._NET_FRAME_EXTENTS);
            let f = |i: usize| f.get(i).copied().unwrap_or(0) as i32;
            let (x, y, wd, h) = (p.dst_x as i32, p.dst_y as i32, g.width as i32, g.height as i32);
            let class = text(w, AtomEnum::WM_CLASS.into()).map_or(String::new(), |c| c.split('\0').filter(|s| !s.is_empty()).last().unwrap_or("").to_string());
            out.push(Foreign {
                id: w as u64,
                pid: self.numbers(w, a._NET_WM_PID).first().copied().unwrap_or(0),
                rect: RECT { left: x - f(0), top: y - f(2), right: x + wd + f(1), bottom: y + h + f(3) },
                title: text(w, a._NET_WM_NAME).or_else(|| text(w, AtomEnum::WM_NAME.into())).unwrap_or_default(),
                class,
                zoomed: state.contains(&a._NET_WM_STATE_MAXIMIZED_VERT),
                full: state.contains(&a._NET_WM_STATE_FULLSCREEN),
            });
        }
        out
    }

    pub fn monitors(&self) -> Vec<Monitor> {
        // asked for all the time, so remembered for a moment
        let known = self.mons.borrow().0.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
        if !known {
            *self.mons.borrow_mut() = (Some(Instant::now()), self.ask_monitors());
        }
        self.mons.borrow().1.clone()
    }

    fn ask_monitors(&self) -> Vec<Monitor> {
        let mut full: Vec<(bool, RECT)> = self
            .conn
            .randr_get_monitors(self.root, true)
            .ok()
            .and_then(|c| c.reply().ok())
            .map_or(Vec::new(), |m| m.monitors.iter().map(|m| (m.primary, r(m.x as i32, m.y as i32, m.width as i32, m.height as i32))).collect());
        full.retain(|(_, m)| m.right > m.left && m.bottom > m.top);
        if full.is_empty() {
            if let Some(g) = self.conn.get_geometry(self.root).ok().and_then(|c| c.reply().ok()) {
                full.push((true, r(0, 0, g.width as i32, g.height as i32)));
            }
        }
        full.sort_by_key(|(p, _)| !*p);
        // what panels leave free, of everything together
        let w = self.numbers(self.root, self.atoms._NET_WORKAREA);
        let work = (w.len() >= 4 && w[2] > 0 && w[3] > 0).then(|| r(w[0] as i32, w[1] as i32, w[2] as i32, w[3] as i32));
        full.into_iter()
            .map(|(_, m)| {
                let cut = work.map(|w| RECT { left: m.left.max(w.left), top: m.top.max(w.top), right: m.right.min(w.right), bottom: m.bottom.min(w.bottom) });
                Monitor { full: m, work: cut.filter(|c| c.right - c.left > 200 && c.bottom - c.top > 200).unwrap_or(m), dpi: self.dpi }
            })
            .collect()
    }

    // ---- mouse and keyboard ----

    pub fn cursor(&self) -> (i32, i32) {
        self.conn.query_pointer(self.root).ok().and_then(|c| c.reply().ok()).map_or((0, 0), |p| (p.root_x as i32, p.root_y as i32))
    }

    pub fn key_down(&self, vk: u16) -> bool {
        let button = match vk {
            VK_LBUTTON => Some(KeyButMask::BUTTON1),
            VK_MBUTTON => Some(KeyButMask::BUTTON2),
            VK_RBUTTON => Some(KeyButMask::BUTTON3),
            _ => None,
        };
        if let Some(b) = button {
            return self.conn.query_pointer(self.root).ok().and_then(|c| c.reply().ok()).is_some_and(|p| u16::from(p.mask) & u16::from(b) != 0);
        }
        let Some(k) = self.conn.query_keymap().ok().and_then(|c| c.reply().ok()) else { return false };
        self.codes_of(vk).iter().any(|c| k.keys[*c as usize / 8] >> (c % 8) & 1 != 0)
    }

    pub fn idle_ms(&self) -> u32 {
        if !self.idle {
            return 0;
        }
        self.conn.screensaver_query_info(self.root).ok().and_then(|c| c.reply().ok()).map_or(0, |i| i.ms_since_user_input)
    }

    /// Ctrl+Alt and this key, wherever it is pressed. False if another program has it.
    pub fn hotkey(&mut self, id: i32, vk: Option<u16>) -> bool {
        const LOCKS: [u16; 4] = [0, 2, 16, 18]; // with and without caps lock and num lock
        let held = |l: u16| ModMask::from(4 | 8 | l);
        for (_, c) in self.hot.iter().filter(|h| h.0 == id) {
            for l in LOCKS {
                let _ = self.conn.ungrab_key(*c, self.root, held(l));
            }
        }
        self.hot.retain(|h| h.0 != id);
        let Some(vk) = vk else { return true };
        let Some(code) = self.codes_of(vk).first().copied() else { return false };
        let ok = LOCKS.iter().all(|l| self.conn.grab_key(true, self.root, held(*l), code, GrabMode::ASYNC, GrabMode::ASYNC).is_ok_and(|c| c.check().is_ok()));
        if ok {
            self.hot.push((id, code));
        }
        ok
    }

    /// The symbol and text of a key with these modifiers held.
    fn read_key(&self, code: Keycode, state: u16) -> (u16, String) {
        let s = self.syms(code);
        let at = |i: usize| s.get(i).copied().unwrap_or(0);
        let (shift, caps, ctrl, alt, num, gr) = (state & 1 != 0, state & 2 != 0, state & 4 != 0, state & 8 != 0, state & 16 != 0, state & 128 != 0);
        let vk = vk_of(at(0));
        let base = if gr && s.len() >= 6 && at(4) != 0 { 4 } else { 0 };
        let pad = (0xff80..=0xffbf).contains(&at(0));
        let mut sym = if pad { if num != shift { at(1) } else { at(0) } } else { at(base + shift as usize) };
        if sym == 0 {
            sym = at(base);
        }
        let mut c = char_of(sym);
        if caps && !pad {
            c = c.map(|c| if shift { c.to_lowercase().next().unwrap_or(c) } else { c.to_uppercase().next().unwrap_or(c) });
        }
        let text = match vk {
            VK_RETURN => "\r".into(),
            VK_BACK => if ctrl { "\u{7f}".into() } else { "\u{8}".into() },
            VK_ESCAPE => "\u{1b}".into(),
            VK_TAB => "\t".into(),
            0x41..=0x5a if ctrl && !alt => char::from_u32(vk as u32 - 0x40).map_or(String::new(), String::from),
            _ if ctrl || alt => String::new(),
            _ => c.map_or(String::new(), String::from),
        };
        (if vk == 0 { vk_of(sym) } else { vk }, text)
    }

    fn take(&mut self, e: Event) {
        let a = self.atoms;
        match e {
            Event::ButtonPress(b) | Event::ButtonRelease(b) if b.response_type & 0x7f == 4 || b.response_type & 0x7f == 5 => {
                let down = b.response_type & 0x7f == 4;
                let Some(win) = self.by_x(b.event) else { return };
                let what = match (b.detail, down) {
                    (n @ 1..=3, true) => Mouse::Down(n),
                    (n @ 1..=3, false) => Mouse::Up(n),
                    (4, true) => Mouse::Wheel(1),
                    (5, true) => Mouse::Wheel(-1),
                    _ => return,
                };
                self.queue.push_back(Ev::Mouse { win, what, x: b.event_x as i32, y: b.event_y as i32, at: (b.root_x as i32, b.root_y as i32) });
            }
            Event::MotionNotify(m) => {
                let Some(win) = self.by_x(m.event) else { return };
                let e = Ev::Mouse { win, what: Mouse::Move, x: m.event_x as i32, y: m.event_y as i32, at: (m.root_x as i32, m.root_y as i32) };
                // only where it got to matters
                match self.queue.back_mut() {
                    Some(last @ Ev::Mouse { what: Mouse::Move, .. }) => *last = e,
                    _ => self.queue.push_back(e),
                }
            }
            Event::LeaveNotify(l) => {
                if let Some(w) = self.by_x(l.event) {
                    self.queue.push_back(Ev::Leave(w));
                }
            }
            Event::KeyPress(k) if k.event == self.root => {
                if let Some((id, _)) = self.hot.iter().find(|h| h.1 == k.detail) {
                    self.queue.push_back(Ev::Hot(*id));
                }
            }
            Event::KeyPress(k) | Event::KeyRelease(k) if k.event != self.root => {
                let down = k.response_type & 0x7f == 2;
                let (vk, text) = self.read_key(k.detail, u16::from(k.state));
                self.queue.push_back(Ev::Key { vk, down, text: if down { text } else { String::new() } });
            }
            Event::Expose(x) if x.count == 0 => {
                if let Some(w) = self.by_x(x.window) {
                    if let Some(o) = self.of(w) {
                        o.last.clear();
                    }
                    self.queue.push_back(Ev::Exposed(w));
                }
            }
            Event::MappingNotify(_) => self.load_keys(),
            Event::RandrScreenChangeNotify(_) => {
                self.mons.borrow_mut().0 = None;
                self.queue.push_back(Ev::Screens);
            }
            Event::SelectionRequest(q) => self.answer(&q),
            Event::SelectionNotify(n) if n.selection == a.XdndSelection && n.property != 0 => {
                let list = self.prop(n.requestor, n.property, AtomEnum::ANY.into()).map_or(Vec::new(), |v| files(&String::from_utf8_lossy(&v)));
                let (from, win) = self.drag;
                if let Some(x) = self.x_of(win) {
                    self.say(from, a.XdndFinished, [x, 1, a.XdndActionCopy, 0, 0]);
                }
                if !list.is_empty() {
                    self.queue.push_back(Ev::Dropped(win, list));
                }
            }
            Event::ClientMessage(c) if c.type_ == a.XdndEnter => self.drag = (c.data.as_data32()[0], self.by_x(c.window).unwrap_or(0)),
            Event::ClientMessage(c) if c.type_ == a.XdndPosition => {
                let d = c.data.as_data32();
                self.say(d[0], a.XdndStatus, [c.window, 1, 0, 0, a.XdndActionCopy]);
            }
            Event::ClientMessage(c) if c.type_ == a.XdndDrop => {
                let d = c.data.as_data32();
                let _ = self.conn.convert_selection(c.window, a.XdndSelection, a.URI_LIST, a.PIXEL_PET_CLIP, d[2]);
                let _ = self.conn.flush();
            }
            _ => {}
        }
    }

    fn say(&self, to: Window, kind: Atom, data: [u32; 5]) {
        let e = ClientMessageEvent::new(32, to, kind, data);
        let _ = self.conn.send_event(false, to, EventMask::NO_EVENT, e);
        let _ = self.conn.flush();
    }

    /// Something that happened, if anything did.
    pub fn next(&mut self) -> Option<Ev> {
        while let Ok(Some(e)) = self.conn.poll_for_event() {
            self.take(e);
        }
        self.queue.pop_front()
    }

    /// Until something happens, or for this long.
    pub fn wait(&mut self, most: Duration) {
        let _ = self.conn.flush();
        if let Ok(Some(e)) = self.conn.poll_for_event() {
            self.take(e);
            return;
        }
        let mut p = PollFd { fd: self.conn.stream().as_raw_fd(), events: 1, revents: 0 };
        let ms = most.as_millis().min(1000) as i32;
        unsafe { poll(&mut p, 1, ms.max(1)) };
        if p.revents & 0x18 != 0 {
            std::process::exit(0); // the screen has gone
        }
    }

    // ---- the clipboard ----

    fn answer(&self, q: &SelectionRequestEvent) {
        let a = &self.atoms;
        let mut property = q.property;
        if q.target == a.TARGETS {
            let _ = self.conn.change_property32(PropMode::REPLACE, q.requestor, q.property, AtomEnum::ATOM, &[a.TARGETS, a.UTF8_STRING, AtomEnum::STRING.into()]);
        } else if q.target == a.UTF8_STRING || q.target == AtomEnum::STRING.into() {
            let _ = self.conn.change_property8(PropMode::REPLACE, q.requestor, q.property, q.target, self.clip.as_bytes());
        } else {
            property = 0;
        }
        let e = SelectionNotifyEvent { response_type: SELECTION_NOTIFY_EVENT, sequence: 0, time: q.time, requestor: q.requestor, selection: q.selection, target: q.target, property };
        let _ = self.conn.send_event(false, q.requestor, EventMask::NO_EVENT, e);
        let _ = self.conn.flush();
    }

    pub fn set_clipboard(&mut self, t: &str) {
        self.clip = t.to_string();
        if let Some(x) = self.own.first().map(|o| o.x) {
            let _ = self.conn.set_selection_owner(x, self.atoms.CLIPBOARD, 0u32);
            let _ = self.conn.flush();
        }
    }

    pub fn clipboard(&mut self) -> String {
        let a = self.atoms;
        let Some(me) = self.own.first().map(|o| o.x) else { return String::new() };
        let owner = self.conn.get_selection_owner(a.CLIPBOARD).ok().and_then(|c| c.reply().ok()).map_or(0, |r| r.owner);
        if owner == 0 {
            return String::new();
        }
        if self.by_x(owner).is_some() {
            return self.clip.clone();
        }
        let _ = self.conn.convert_selection(me, a.CLIPBOARD, a.UTF8_STRING, a.PIXEL_PET_CLIP, 0u32);
        let _ = self.conn.flush();
        let end = Instant::now() + Duration::from_millis(250);
        while Instant::now() < end {
            match self.conn.poll_for_event() {
                Ok(Some(Event::SelectionNotify(n))) if n.selection == a.CLIPBOARD => {
                    return if n.property == 0 { String::new() } else { self.prop(me, n.property, AtomEnum::ANY.into()).map_or(String::new(), |v| String::from_utf8_lossy(&v).into_owned()) };
                }
                Ok(Some(e)) => self.take(e),
                _ => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        String::new()
    }

    pub fn accept_files(&mut self, id: u32, on: bool) {
        if let (Some(x), true) = (self.x_of(id), on) {
            let _ = self.conn.change_property32(PropMode::REPLACE, x, self.atoms.XdndAware, AtomEnum::ATOM, &[5]);
        }
    }

    /// There is no one way to have an icon by the clock here; the hub does everything it would.
    pub fn tray(&mut self, _: bool, _: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_taken_apart_into_rectangles_that_cover_exactly_it() {
        let (w, h, key) = (7, 6, 9u32);
        let art = ["..XX...", "..XX...", ".XXXX.X", ".XXXX.X", "X......", "......."];
        let px: Vec<u32> = art.iter().flat_map(|r| r.chars().map(|c| if c == 'X' { 1 } else { key })).collect();
        let s = spans(&px, w, key);
        let mut got = vec![key; (w * h) as usize];
        for (x, y, rw, rh) in &s {
            for yy in *y..y + rh {
                for xx in *x..x + rw {
                    assert_eq!(got[(yy * w + xx) as usize], key, "covered twice");
                    got[(yy * w + xx) as usize] = 1;
                }
            }
        }
        assert_eq!(got, px);
        assert_eq!(s.len(), 4, "rows that look the same share theirs");
        assert!(spans(&[key; 12], 4, key).is_empty());
    }

    #[test]
    fn dropped_files_get_their_names_back() {
        assert_eq!(files("# comment\r\nfile:///home/me/a%20b%C3%A9.txt\r\nfile://host/x\r\nhttps://a.b/c%20d\r\n"), ["/home/me/a b\u{e9}.txt", "/x", "https://a.b/c%20d"]);
    }

    #[test]
    fn keys_have_the_names_and_letters_they_should() {
        assert_eq!(vk_of('k' as u32), b'K' as u16);
        assert_eq!(vk_of(0xff0d), VK_RETURN);
        assert_eq!(char_of(0xe9), Some('\u{e9}'));
        assert_eq!(char_of(0x0100_0414), Some('\u{414}'));
        assert_eq!(char_of(0xffe1), None);
    }
}
