//! Windows on macOS, through Cocoa. Coordinates are points, counted from the top left of the first
//! screen the way the rest of the program does; Cocoa counts from the bottom.

use super::gdi::Bitmap;
use super::*;
use objc2::encode::{Encode, Encoding};
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use std::collections::VecDeque;
use std::ffi::{c_void, CStr, CString};
use std::time::{Duration, Instant};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

type Id = *mut AnyObject;
type Void = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    w: f64,
    h: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    at: Point,
    size: Size,
}

unsafe impl Encode for Point {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for Size {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for Rect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}
#[link(name = "QuartzCore", kind = "framework")]
extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> Void;
    fn CGDataProviderCreateWithCFData(d: Void) -> Void;
    fn CGDataProviderRelease(p: Void);
    fn CGImageCreate(w: usize, h: usize, bpc: usize, bpp: usize, row: usize, space: Void, info: u32, p: Void, decode: *const f64, smooth: bool, intent: i32) -> Void;
    fn CGImageRelease(i: Void);
    fn CGEventSourceKeyState(source: i32, key: u16) -> bool;
    fn CGEventSourceButtonState(source: i32, button: u32) -> bool;
    fn CGEventSourceSecondsSinceLastEventType(source: i32, kind: u32) -> f64;
    fn CGWindowListCopyWindowInfo(what: u32, relative: u32) -> Void;
    fn CGRectMakeWithDictionaryRepresentation(d: Void, r: *mut Rect) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFDataCreate(a: Void, p: *const u8, n: isize) -> Void;
    fn CFRelease(o: Void);
    fn CFArrayGetCount(a: Void) -> isize;
    fn CFArrayGetValueAtIndex(a: Void, i: isize) -> Void;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct HotId {
    sign: u32,
    id: u32,
}

#[repr(C)]
struct EventType {
    class: u32,
    kind: u32,
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn GetApplicationEventTarget() -> Void;
    fn InstallEventHandler(target: Void, f: extern "C" fn(Void, Void, Void) -> i32, n: usize, list: *const EventType, data: Void, out: *mut Void) -> i32;
    fn RegisterEventHotKey(code: u32, mods: u32, id: HotId, target: Void, options: u32, out: *mut Void) -> i32;
    fn UnregisterEventHotKey(h: Void) -> i32;
    fn GetEventParameter(e: Void, name: u32, kind: u32, actual: *mut u32, size: usize, out_size: *mut usize, out: Void) -> i32;
}

/// What came from outside the event queue: hot keys and the menu bar icon.
static OUTSIDE: Mutex<VecDeque<Ev>> = Mutex::new(VecDeque::new());

extern "C" fn hot(_: Void, e: Void, _: Void) -> i32 {
    let mut id = HotId { sign: 0, id: 0 };
    // kEventParamDirectObject, typeEventHotKeyID
    let ok = unsafe { GetEventParameter(e, u32::from_be_bytes(*b"----"), u32::from_be_bytes(*b"hkid"), std::ptr::null_mut(), 8, std::ptr::null_mut(), &mut id as *mut _ as Void) };
    if ok == 0 {
        OUTSIDE.lock().unwrap().push_back(Ev::Hot(id.id as i32));
    }
    0
}

extern "C" fn yes(_: &AnyObject, _: Sel) -> objc2::runtime::Bool {
    objc2::runtime::Bool::YES
}

extern "C" fn icon_clicked(_: &AnyObject, _: Sel, _: Id) {
    let kind: usize = unsafe {
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let e: Id = msg_send![app, currentEvent];
        if e.is_null() { 2 } else { msg_send![e, type] }
    };
    // WM_RBUTTONUP, WM_LBUTTONUP
    OUTSIDE.lock().unwrap().push_back(Ev::Tray(if kind == 3 || kind == 4 { 0x0205 } else { 0x0202 }));
}

/// An NSString that lives as long as the surrounding pool.
fn ns(s: &str) -> Id {
    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()] }
}

fn text(o: Id) -> String {
    if o.is_null() {
        return String::new();
    }
    let p: *const std::ffi::c_char = unsafe { msg_send![o, UTF8String] };
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

/// What Windows calls the key with this code (they are places on the keyboard, named after the
/// American one).
pub(crate) const KEYS: &[(u16, u16)] = &[
    (0, b'A' as u16), (11, b'B' as u16), (8, b'C' as u16), (2, b'D' as u16), (14, b'E' as u16), (3, b'F' as u16), (5, b'G' as u16), (4, b'H' as u16), (34, b'I' as u16), (38, b'J' as u16),
    (40, b'K' as u16), (37, b'L' as u16), (46, b'M' as u16), (45, b'N' as u16), (31, b'O' as u16), (35, b'P' as u16), (12, b'Q' as u16), (15, b'R' as u16), (1, b'S' as u16), (17, b'T' as u16),
    (32, b'U' as u16), (9, b'V' as u16), (13, b'W' as u16), (7, b'X' as u16), (16, b'Y' as u16), (6, b'Z' as u16), (29, b'0' as u16), (18, b'1' as u16), (19, b'2' as u16), (20, b'3' as u16),
    (21, b'4' as u16), (23, b'5' as u16), (22, b'6' as u16), (26, b'7' as u16), (28, b'8' as u16), (25, b'9' as u16), (36, VK_RETURN), (76, VK_RETURN), (48, VK_TAB), (49, VK_SPACE), (51, VK_BACK), (53, VK_ESCAPE),
    (117, VK_DELETE), (115, VK_HOME), (119, VK_END), (116, VK_PRIOR), (121, VK_NEXT), (123, VK_LEFT), (124, VK_RIGHT), (125, VK_DOWN), (126, VK_UP), (56, VK_SHIFT), (60, VK_SHIFT), (59, VK_CONTROL), (62, VK_CONTROL), (58, VK_MENU), (61, VK_MENU),
];

fn vk_of(code: u16) -> u16 {
    KEYS.iter().find(|k| k.0 == code).map_or(0, |k| k.1)
}

/// What a key press writes. Command does here what Ctrl does elsewhere.
pub(crate) fn written(vk: u16, chars: &str, command: bool, control: bool) -> String {
    match vk {
        VK_RETURN => "\r".into(),
        VK_BACK => if control { "\u{7f}".into() } else { "\u{8}".into() },
        VK_ESCAPE => "\u{1b}".into(),
        VK_TAB => "\t".into(),
        0x41..=0x5a if command || control => char::from_u32(vk as u32 - 0x40).map_or(String::new(), String::from),
        _ if command || control => String::new(),
        // arrows and the like have characters of their own, which are none
        _ => chars.chars().filter(|c| !c.is_control() && !(0xf700..=0xf8ff).contains(&(*c as u32))).collect(),
    }
}

struct Own {
    id: u32,
    win: Id,
    layer: Id,
    rect: RECT,
    shown: bool,
    focusable: bool,
    through: bool,
    ignoring: bool,
    mask: Option<(i32, i32, Vec<bool>)>, // which of its pixels can be seen
    alpha: u8,
}

pub(crate) struct Screen {
    app: Id,
    space: Void,
    plain: &'static AnyClass,
    own: Vec<Own>,
    queue: VecDeque<Ev>,
    hots: Vec<(i32, Void)>,
    mons: std::cell::RefCell<(Option<Instant>, Vec<Monitor>)>,
    seen: Vec<(i32, i32, i32, i32)>, // the monitors at the last look
    looked: Instant,
    mouse: (i32, i32),
    over: u32,
    held: u32, // the window a button went down in
    flags: usize,
    icon: Id,
    clicker: Id,
}

impl Screen {
    pub fn open() -> Result<Screen, String> {
        unsafe {
            let app: Id = msg_send![class!(NSApplication), sharedApplication];
            if app.is_null() {
                return Err("there is no window server".into());
            }
            let _: bool = msg_send![app, setActivationPolicy: 1isize]; // no icon in the Dock
            let _: () = msg_send![app, finishLaunching];
            // a window without a frame takes no keys unless it says so
            let plain = {
                let mut b = ClassBuilder::new(c"PixelPetPanel", class!(NSPanel)).ok_or("a class is there already")?;
                b.add_method(sel!(canBecomeKeyWindow), yes as extern "C" fn(_, _) -> _);
                b.register()
            };
            let clicker = {
                let mut b = ClassBuilder::new(c"PixelPetClicker", class!(NSObject)).ok_or("a class is there already")?;
                b.add_method(sel!(clicked:), icon_clicked as extern "C" fn(_, _, _));
                let c = b.register();
                let o: Id = msg_send![c, new];
                o
            };
            let kind = EventType { class: u32::from_be_bytes(*b"keyb"), kind: 5 }; // kEventHotKeyPressed
            InstallEventHandler(GetApplicationEventTarget(), hot, 1, &kind, std::ptr::null_mut(), std::ptr::null_mut());
            let mut s = Screen {
                app,
                space: CGColorSpaceCreateDeviceRGB(),
                plain,
                own: Vec::new(),
                queue: VecDeque::new(),
                hots: Vec::new(),
                mons: Default::default(),
                seen: Vec::new(),
                looked: Instant::now(),
                mouse: (0, 0),
                over: 0,
                held: 0,
                flags: 0,
                icon: std::ptr::null_mut(),
                clicker,
            };
            if s.monitors().is_empty() {
                return Err("there is no screen".into());
            }
            s.seen = s.outline();
            Ok(s)
        }
    }

    /// Height of the first screen, which everything is measured against.
    fn height(&self) -> f64 {
        unsafe {
            let list: Id = msg_send![class!(NSScreen), screens];
            let n: usize = msg_send![list, count];
            if n == 0 {
                return 0.0;
            }
            let first: Id = msg_send![list, objectAtIndex: 0usize];
            let f: Rect = msg_send![first, frame];
            f.size.h
        }
    }

    fn flip(&self, r: Rect) -> RECT {
        let h = self.height();
        fine::dots(r.at.x, h - r.at.y - r.size.h, r.size.w, r.size.h)
    }

    fn unflip(&self, r: RECT) -> Rect {
        let (x, top, w, h) = fine::points(r);
        Rect { at: Point { x, y: self.height() - top - h }, size: Size { w, h } }
    }

    fn of(&mut self, id: u32) -> Option<&mut Own> {
        self.own.iter_mut().find(|o| o.id == id)
    }

    // ---- windows ----

    pub fn create(&mut self, m: &Make) {
        autoreleasepool(|_| unsafe {
            let frame = self.unflip(m.rect);
            // the pet itself never takes the keyboard away from anybody; the tools do, when they are used
            let w: Id = if m.takes_focus { msg_send![self.plain, alloc] } else { msg_send![class!(NSPanel), alloc] };
            // borderless, and a panel, so that using it does not bring the whole program to the front
            let style: usize = 1 << 7;
            let w: Id = msg_send![w, initWithContentRect: frame, styleMask: style, backing: 2usize, defer: false];
            let clear: Id = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![w, setOpaque: false];
            let _: () = msg_send![w, setBackgroundColor: clear];
            let _: () = msg_send![w, setHasShadow: false];
            let _: () = msg_send![w, setReleasedWhenClosed: false];
            let _: () = msg_send![w, setHidesOnDeactivate: false];
            let _: () = msg_send![w, setTitle: ns(&m.title)];
            let _: () = msg_send![w, setLevel: if m.top { 25isize } else { 3isize }];
            // on every desktop, and over programs that fill the screen
            let _: () = msg_send![w, setCollectionBehavior: (1usize | 1 << 4 | 1 << 6 | 1 << 8)];
            let view: Id = msg_send![w, contentView];
            let layer: Id = msg_send![class!(CALayer), new];
            let _: () = msg_send![view, setLayer: layer];
            let _: () = msg_send![view, setWantsLayer: true];
            let _: () = msg_send![layer, setMagnificationFilter: ns("nearest")];
            // on a screen that is not Retina every dot is made of four
            let _: () = msg_send![layer, setMinificationFilter: ns("linear")];
            // dot for dot from the top left corner, never stretched to fit
            let _: () = msg_send![layer, setContentsGravity: ns("topLeft")];
            let _: () = msg_send![layer, setContentsScale: fine::DOTS];
            if m.through {
                let _: () = msg_send![w, setIgnoresMouseEvents: true];
            }
            self.own.push(Own { id: m.id, win: w, layer, rect: m.rect, shown: false, focusable: m.takes_focus, through: m.through, ignoring: m.through, mask: None, alpha: 255 });
        });
    }

    pub fn destroy(&mut self, id: u32) {
        if let Some(o) = self.of(id) {
            unsafe {
                let _: () = msg_send![o.win, orderOut: std::ptr::null_mut::<AnyObject>()];
                let _: () = msg_send![o.win, close];
                let _: () = msg_send![o.win, release];
            }
        }
        self.own.retain(|o| o.id != id);
    }

    pub fn show(&mut self, id: u32, on: bool) {
        let Some(o) = self.of(id) else { return };
        o.shown = on;
        unsafe {
            if on {
                let _: () = msg_send![o.win, orderFrontRegardless];
            } else {
                let _: () = msg_send![o.win, orderOut: std::ptr::null_mut::<AnyObject>()];
            }
        }
    }

    pub fn place(&mut self, id: u32, to: RECT) {
        let frame = self.unflip(to);
        let Some(o) = self.of(id) else { return };
        o.rect = to;
        unsafe {
            let _: () = msg_send![o.win, setFrame: frame, display: false];
        }
    }

    pub fn raise(&mut self, id: u32) {
        if let Some(o) = self.of(id).filter(|o| o.shown) {
            unsafe {
                let _: () = msg_send![o.win, setLevel: 25isize];
                let _: () = msg_send![o.win, orderFrontRegardless];
            }
        }
    }

    /// Among the ordinary windows.
    pub fn lower(&mut self, id: u32) {
        if let Some(o) = self.of(id) {
            unsafe {
                let _: () = msg_send![o.win, setLevel: 0isize];
            }
        }
    }

    pub fn below(&mut self, id: u32, other: u32) {
        let Some(n) = self.of(other).map(|o| unsafe { msg_send![o.win, windowNumber] }) else { return };
        let n: isize = n;
        if let Some(o) = self.of(id) {
            unsafe {
                let _: () = msg_send![o.win, setLevel: 25isize];
                if o.shown {
                    let _: () = msg_send![o.win, orderWindow: -1isize, relativeTo: n];
                }
            }
        }
    }

    pub fn pass_clicks(&mut self, id: u32, on: bool) {
        if let Some(o) = self.of(id) {
            o.through = on;
            o.ignoring = on;
            unsafe {
                let _: () = msg_send![o.win, setIgnoresMouseEvents: on];
            }
        }
    }

    pub fn present(&mut self, id: u32, b: &Bitmap, key: Option<u32>, alpha: u8) {
        let space = self.space;
        let Some(o) = self.of(id) else { return };
        let px: Vec<u32> = b.px.iter().map(|p| if key == Some(p & 0xff_ffff) { 0 } else { 0xff00_0000 | p }).collect();
        o.mask = key.map(|_| (b.w, b.h, px.iter().map(|p| *p != 0).collect()));
        let faded = std::mem::replace(&mut o.alpha, alpha) != alpha;
        autoreleasepool(|_| unsafe {
            let data = CFDataCreate(std::ptr::null_mut(), px.as_ptr() as *const u8, (px.len() * 4) as isize);
            let provider = CGDataProviderCreateWithCFData(data);
            // 32 bits, alpha first, in the byte order of this machine
            let image = CGImageCreate(b.w as usize, b.h as usize, 8, 32, b.w as usize * 4, space, 2 | 2 << 12, provider, std::ptr::null(), false, 0);
            let _: () = msg_send![class!(CATransaction), begin];
            let _: () = msg_send![class!(CATransaction), setDisableActions: true];
            let _: () = msg_send![o.layer, setContentsScale: fine::DOTS];
            let _: () = msg_send![o.layer, setContents: image as Id];
            let _: () = msg_send![class!(CATransaction), commit];
            CGImageRelease(image);
            CGDataProviderRelease(provider);
            CFRelease(data);
            if faded {
                let _: () = msg_send![o.win, setAlphaValue: alpha as f64 / 255.0];
            }
        });
    }

    // ---- who is in front ----

    pub fn focus(&mut self, id: u32) {
        if let Some(o) = self.of(id).filter(|o| o.shown && o.focusable) {
            unsafe {
                // such a panel gets the keys although another program stays the active one
                let _: () = msg_send![o.win, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
            }
        }
    }

    pub fn focused(&self) -> Option<u32> {
        self.own.iter().find(|o| o.shown && o.focusable && unsafe { msg_send![o.win, isKeyWindow] }).map(|o| o.id)
    }

    pub fn active(&self) -> Option<u64> {
        self.others().first().map(|o| o.id)
    }

    pub fn others(&self) -> Vec<Foreign> {
        let mut out = Vec::new();
        let mons = self.monitors();
        autoreleasepool(|_| unsafe {
            // on the screen, without the desktop itself
            let list = CGWindowListCopyWindowInfo(1 | 16, 0);
            if list.is_null() {
                return;
            }
            for i in 0..CFArrayGetCount(list).min(80) {
                let d = CFArrayGetValueAtIndex(list, i) as Id;
                let get = |k: &str| -> Id { msg_send![d, objectForKey: ns(k)] };
                let number = |k: &str| -> i64 {
                    let n = get(k);
                    if n.is_null() { 0 } else { msg_send![n, longLongValue] }
                };
                let pid = number("kCGWindowOwnerPID") as u32;
                if number("kCGWindowLayer") != 0 || pid == std::process::id() {
                    continue;
                }
                let mut r = Rect { at: Point { x: 0.0, y: 0.0 }, size: Size { w: 0.0, h: 0.0 } };
                let bounds = get("kCGWindowBounds");
                if bounds.is_null() || !CGRectMakeWithDictionaryRepresentation(bounds as Void, &mut r) {
                    continue;
                }
                let rect = fine::dots(r.at.x, r.at.y, r.size.w, r.size.h);
                let owner = text(get("kCGWindowOwnerName"));
                let name = text(get("kCGWindowName")); // only with permission to record the screen
                let same = |a: &RECT| (a.left, a.top, a.right, a.bottom) == (rect.left, rect.top, rect.right, rect.bottom);
                out.push(Foreign {
                    id: number("kCGWindowNumber") as u64,
                    pid,
                    rect,
                    title: if name.is_empty() { owner.clone() } else { name },
                    class: owner,
                    zoomed: mons.iter().any(|m| same(&m.work)),
                    full: mons.iter().any(|m| same(&m.full)),
                });
            }
            CFRelease(list);
        });
        out
    }

    fn ask_monitors(&self) -> Vec<Monitor> {
        autoreleasepool(|_| unsafe {
            let list: Id = msg_send![class!(NSScreen), screens];
            let n: usize = msg_send![list, count];
            (0..n)
                .map(|i| {
                    let s: Id = msg_send![list, objectAtIndex: i];
                    let (f, v): (Rect, Rect) = (msg_send![s, frame], msg_send![s, visibleFrame]);
                    Monitor { full: self.flip(f), work: self.flip(v), dpi: (96.0 * fine::DOTS) as u32 }
                })
                .collect()
        })
    }

    pub fn monitors(&self) -> Vec<Monitor> {
        let known = self.mons.borrow().0.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
        if !known {
            *self.mons.borrow_mut() = (Some(Instant::now()), self.ask_monitors());
        }
        self.mons.borrow().1.clone()
    }

    fn outline(&self) -> Vec<(i32, i32, i32, i32)> {
        self.monitors().iter().flat_map(|m| [m.full, m.work]).map(|r| (r.left, r.top, r.right, r.bottom)).collect()
    }

    // ---- mouse and keyboard ----

    pub fn cursor(&self) -> (i32, i32) {
        let p: Point = unsafe { msg_send![class!(NSEvent), mouseLocation] };
        fine::dot(p.x, self.height() - p.y)
    }

    pub fn key_down(&self, vk: u16) -> bool {
        unsafe {
            match vk {
                VK_LBUTTON => CGEventSourceButtonState(0, 0),
                VK_RBUTTON => CGEventSourceButtonState(0, 1),
                VK_MBUTTON => CGEventSourceButtonState(0, 2),
                _ => KEYS.iter().filter(|k| k.1 == vk).any(|k| CGEventSourceKeyState(0, k.0)),
            }
        }
    }

    pub fn idle_ms(&self) -> u32 {
        let s = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
        (s * 1000.0).clamp(0.0, 4.0e9) as u32
    }

    pub fn hotkey(&mut self, id: i32, vk: Option<u16>) -> bool {
        for (_, h) in self.hots.iter().filter(|h| h.0 == id) {
            unsafe { UnregisterEventHotKey(*h) };
        }
        self.hots.retain(|h| h.0 != id);
        let Some(code) = vk.and_then(|vk| KEYS.iter().find(|k| k.1 == vk)).map(|k| k.0) else { return vk.is_none() };
        let mut h = std::ptr::null_mut();
        // control and option
        let ok = unsafe { RegisterEventHotKey(code as u32, 0x1000 | 0x0800, HotId { sign: u32::from_be_bytes(*b"PPET"), id: id as u32 }, GetApplicationEventTarget(), 0, &mut h) } == 0;
        if ok {
            self.hots.push((id, h));
        }
        ok
    }

    /// The topmost of our windows that can be seen at this point.
    fn under(&self, at: (i32, i32)) -> u32 {
        // the tools are in front of the pet, and later ones in front of earlier
        let hit = |o: &&Own| {
            let (x, y) = (at.0 - o.rect.left, at.1 - o.rect.top);
            o.shown
                && !o.through
                && x >= 0
                && y >= 0
                && at.0 < o.rect.right
                && at.1 < o.rect.bottom
                && o.mask.as_ref().is_none_or(|(w, h, m)| x < *w && y < *h && m[(y * w + x) as usize])
        };
        self.own.iter().rev().find(hit).map_or(0, |o| o.id)
    }

    fn mouse_event(&mut self, win: u32, what: Mouse) {
        let at = self.cursor();
        if let Some(r) = self.own.iter().find(|o| o.id == win).map(|o| o.rect) {
            self.queue.push_back(Ev::Mouse { win, what, x: at.0 - r.left, y: at.1 - r.top, at });
        }
    }

    /// Cocoa only tells the window in front about the mouse, so it is watched. Clicks beside what
    /// can be seen of a window go to whatever is behind it.
    fn watch(&mut self) {
        let at = self.cursor();
        let over = self.under(at);
        if self.held == 0 {
            for o in self.own.iter_mut().filter(|o| o.shown && o.mask.is_some()) {
                let ignore = o.through || o.id != over;
                if ignore != o.ignoring {
                    o.ignoring = ignore;
                    unsafe {
                        let _: () = msg_send![o.win, setIgnoresMouseEvents: ignore];
                    }
                }
            }
        }
        if at != self.mouse {
            self.mouse = at;
            let to = if self.held != 0 { self.held } else { over };
            if to != 0 {
                self.mouse_event(to, Mouse::Move);
            }
        }
        if self.over != over {
            let was = std::mem::replace(&mut self.over, over);
            if was != 0 {
                self.queue.push_back(Ev::Leave(was));
            }
        }
        if self.looked.elapsed() > Duration::from_secs(2) {
            self.looked = Instant::now();
            let now = self.outline();
            if now != self.seen {
                self.seen = now;
                self.queue.push_back(Ev::Screens);
            }
        }
    }

    unsafe fn take(&mut self, e: Id) {
        let kind: usize = msg_send![e, type];
        let flags: usize = msg_send![e, modifierFlags];
        let (shift, control, option, command) = (1usize << 17, 1usize << 18, 1usize << 19, 1usize << 20);
        match kind {
            1..=4 | 25 | 26 => {
                let win: Id = msg_send![e, window];
                let id = self.own.iter().find(|o| o.win == win && !win.is_null()).map_or(0, |o| o.id);
                let right = matches!(kind, 3 | 4) || (kind <= 2 && flags & control != 0);
                let button = if kind >= 25 { 2 } else if right { 3 } else { 1 };
                let down = matches!(kind, 1 | 3 | 25);
                let to = if down { id } else if self.held != 0 { self.held } else { id };
                if to != 0 {
                    self.held = if down { to } else { 0 };
                    self.mouse_event(to, if down { Mouse::Down(button) } else { Mouse::Up(button) });
                }
            }
            22 => {
                let dy: f64 = msg_send![e, scrollingDeltaY];
                let over = self.under(self.cursor());
                if over != 0 && dy.abs() > 0.1 {
                    self.mouse_event(over, Mouse::Wheel(if dy > 0.0 { 1 } else { -1 }));
                }
            }
            10 | 11 => {
                let code: u16 = msg_send![e, keyCode];
                let vk = vk_of(code);
                let chars: Id = msg_send![e, characters];
                let t = if kind == 10 { written(vk, &text(chars), flags & command != 0, flags & control != 0) } else { String::new() };
                self.queue.push_back(Ev::Key { vk, down: kind == 10, text: t });
                return; // the system would beep at keys nobody takes
            }
            12 => {
                for (mask, vk) in [(shift, VK_SHIFT), (control | command, VK_CONTROL), (option, VK_MENU)] {
                    if (flags & mask != 0) != (self.flags & mask != 0) {
                        self.queue.push_back(Ev::Key { vk, down: flags & mask != 0, text: String::new() });
                    }
                }
                self.flags = flags;
            }
            _ => {}
        }
        let _: () = msg_send![self.app, sendEvent: e];
    }

    /// The next event of the system, waiting this long for one.
    fn pull(&mut self, most: Duration) -> bool {
        autoreleasepool(|_| unsafe {
            let until: Id = if most.is_zero() { msg_send![class!(NSDate), distantPast] } else { msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: most.as_secs_f64()] };
            let e: Id = msg_send![self.app, nextEventMatchingMask: usize::MAX, untilDate: until, inMode: ns("kCFRunLoopDefaultMode"), dequeue: true];
            if e.is_null() {
                return false;
            }
            self.take(e);
            true
        })
    }

    pub fn next(&mut self) -> Option<Ev> {
        while self.queue.is_empty() && self.pull(Duration::ZERO) {}
        if self.queue.is_empty() {
            self.watch();
        }
        self.queue.pop_front().or_else(|| OUTSIDE.lock().unwrap().pop_front())
    }

    pub fn wait(&mut self, most: Duration) {
        // not for long, as the mouse is watched
        self.pull(most.min(Duration::from_millis(33)));
    }

    // ---- the clipboard, and the icon in the menu bar ----

    pub fn set_clipboard(&mut self, t: &str) {
        autoreleasepool(|_| unsafe {
            let p: Id = msg_send![class!(NSPasteboard), generalPasteboard];
            let _: isize = msg_send![p, clearContents];
            let _: bool = msg_send![p, setString: ns(t), forType: ns("public.utf8-plain-text")];
        });
    }

    pub fn clipboard(&mut self) -> String {
        autoreleasepool(|_| unsafe {
            let p: Id = msg_send![class!(NSPasteboard), generalPasteboard];
            let s: Id = msg_send![p, stringForType: ns("public.utf8-plain-text")];
            text(s)
        })
    }

    pub fn accept_files(&mut self, _: u32, _: bool) {}

    pub fn tray(&mut self, on: bool, tip: &str) -> bool {
        autoreleasepool(|_| unsafe {
            let bar: Id = msg_send![class!(NSStatusBar), systemStatusBar];
            if !on {
                if !self.icon.is_null() {
                    let _: () = msg_send![bar, removeStatusItem: self.icon];
                    let _: () = msg_send![self.icon, release];
                    self.icon = std::ptr::null_mut();
                }
                return;
            }
            if self.icon.is_null() {
                let i: Id = msg_send![bar, statusItemWithLength: -1.0f64];
                self.icon = msg_send![i, retain];
                let b: Id = msg_send![self.icon, button];
                let _: () = msg_send![b, setTitle: ns("\u{1f43e}")];
                let _: () = msg_send![b, setTarget: self.clicker];
                let _: () = msg_send![b, setAction: sel!(clicked:)];
                // left and right, when let go
                let _: isize = msg_send![b, sendActionOn: 1usize << 2 | 1 << 4];
            }
            let b: Id = msg_send![self.icon, button];
            let _: () = msg_send![b, setToolTip: ns(tip)];
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_write_what_they_should() {
        assert_eq!(written(b'A' as u16, "a", false, false), "a");
        assert_eq!(written(b'V' as u16, "v", true, false), "\u{16}");
        assert_eq!(written(VK_BACK, "\u{7f}", false, false), "\u{8}");
        assert_eq!(written(VK_LEFT, "\u{f702}", false, false), "");
        assert_eq!(written(VK_RETURN, "\r", false, false), "\r");
        assert_eq!(written(b'9' as u16, "9", true, false), "");
    }

    #[test]
    fn every_letter_and_digit_has_a_key() {
        for c in (b'A'..=b'Z').chain(b'0'..=b'9') {
            assert_eq!(KEYS.iter().filter(|k| k.1 == c as u16).count(), 1, "{}", c as char);
        }
    }
}
