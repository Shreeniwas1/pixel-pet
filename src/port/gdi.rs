//! Drawing in memory: the few calls of GDI the pet uses, on plain arrays of pixels (0x00RRGGBB).

use super::*;
use std::collections::HashMap;
use windows_sys::Win32::Graphics::Gdi::*;

pub(crate) struct Bitmap {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
}

pub(crate) struct Dc {
    pub bmp: *mut Obj,
    font: *mut Obj,
    pen: *mut Obj,
    at: (i32, i32),
    text: u32, // colours are kept the way pixels are
    back: u32,
    opaque: bool,
    brush: u32,
    pub org: (i32, i32),
    pub clip: RECT,
}

#[derive(Clone, Copy)]
pub(crate) struct Font {
    adv: i32,
    line: i32,
    base: i32,
    size: f32,
}

pub(crate) enum Obj {
    Dc(Dc),
    Bitmap(Bitmap),
    Font(Font),
    Pen(i32, u32),
}

const ALL: RECT = RECT { left: 0, top: 0, right: i32::MAX, bottom: i32::MAX };
/// The one stock brush in use; never looked into.
const STOCK_DC_BRUSH: usize = 16;

fn boxed(o: Obj) -> *mut Obj {
    Box::into_raw(Box::new(o))
}

pub(crate) unsafe fn free(o: *mut Obj) {
    if o as usize > 4096 {
        drop(Box::from_raw(o));
    }
}

fn swap(c: u32) -> u32 {
    (c & 0xff) << 16 | c & 0xff00 | c >> 16 & 0xff
}

pub(crate) fn new_bitmap(w: i32, h: i32) -> *mut Obj {
    let (w, h) = (w.max(1), h.max(1));
    boxed(Obj::Bitmap(Bitmap { w, h, px: vec![0; (w * h) as usize] }))
}

pub(crate) unsafe fn bitmap<'a>(o: *mut Obj) -> Option<&'a mut Bitmap> {
    match o.as_mut()? {
        Obj::Bitmap(b) => Some(b),
        _ => None,
    }
}

/// A drawing context on this bitmap, its origin at `org`, kept inside `clip`.
pub(crate) fn dc_on(bmp: *mut Obj, org: (i32, i32), clip: RECT) -> HDC {
    boxed(Obj::Dc(Dc { bmp, font: std::ptr::null_mut(), pen: std::ptr::null_mut(), at: (0, 0), text: 0, back: 0xff_ffff, opaque: true, brush: 0xff_ffff, org, clip })) as HDC
}

unsafe fn dc<'a>(h: HDC) -> Option<&'a mut Dc> {
    match (h as *mut Obj).as_mut()? {
        Obj::Dc(d) => Some(d),
        _ => None,
    }
}

/// Colours a context was given: (text, behind it, brush).
pub(crate) unsafe fn colours(h: HDC) -> (u32, u32, u32) {
    dc(h).map_or((0, 0, 0), |d| (d.text, d.back, d.brush))
}

impl Dc {
    unsafe fn fill(&mut self, l: i32, t: i32, r: i32, b: i32, c: u32) {
        let (ox, oy, clip) = (self.org.0, self.org.1, self.clip);
        let Some(m) = bitmap(self.bmp) else { return };
        let l = (l + ox).max(clip.left).max(0);
        let t = (t + oy).max(clip.top).max(0);
        let r = r.saturating_add(ox).min(clip.right).min(m.w);
        let b = b.saturating_add(oy).min(clip.bottom).min(m.h);
        for y in t..b {
            if r > l {
                m.px[(y * m.w + l) as usize..(y * m.w + r) as usize].fill(c);
            }
        }
    }
}

// ---- letters ----

static FACE: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
type Glyph = (fontdue::Metrics, Vec<u8>);
static GLYPHS: Mutex<Option<HashMap<(char, u32), std::sync::Arc<Glyph>>>> = Mutex::new(None);

fn face() -> &'static fontdue::Font {
    FACE.get_or_init(|| fontdue::Font::from_bytes(&include_bytes!("../../assets/fonts/FiraMono-Bold.ttf")[..], fontdue::FontSettings::default()).expect("the font that comes with it"))
}

fn glyph(c: char, size: f32) -> std::sync::Arc<Glyph> {
    let mut g = GLYPHS.lock().unwrap();
    let map = g.get_or_insert_with(HashMap::new);
    if map.len() > 4000 {
        map.clear();
    }
    map.entry((c, (size * 8.0) as u32)).or_insert_with(|| std::sync::Arc::new(face().rasterize(c, size))).clone()
}

/// Consolas, which everything is laid out for, is this wide and high compared with its size.
fn font_of(px: i32) -> Font {
    let px = px.max(1) as f32;
    let adv = (px * 0.55).round().max(1.0);
    Font { adv: adv as i32, line: (px * 1.17).round() as i32, base: (px * 0.90).round() as i32, size: (adv / 0.6).max(4.0) }
}

impl Font {
    pub fn adv(&self) -> i32 {
        self.adv
    }

    pub fn line(&self) -> i32 {
        self.line
    }
}

pub(crate) unsafe fn font(h: HFONT) -> Font {
    match (h as usize > 4096).then(|| &*(h as *mut Obj)) {
        Some(Obj::Font(f)) => *f,
        _ => font_of(14),
    }
}

impl Dc {
    /// One line of text with its top left corner here.
    pub unsafe fn letters(&mut self, text: &[char], x: i32, y: i32, f: Font, clip: RECT) {
        let (ox, oy) = self.org;
        let c = RECT { left: (clip.left + ox).max(self.clip.left).max(0), top: (clip.top + oy).max(self.clip.top).max(0), right: clip.right.saturating_add(ox).min(self.clip.right), bottom: clip.bottom.saturating_add(oy).min(self.clip.bottom) };
        let ink = self.text;
        let Some(m) = bitmap(self.bmp) else { return };
        let (right, bottom) = (c.right.min(m.w), c.bottom.min(m.h));
        for (i, &ch) in text.iter().enumerate() {
            if ch == ' ' {
                continue;
            }
            let g = glyph(ch, f.size);
            let gx = x + ox + i as i32 * f.adv + g.0.xmin;
            let gy = y + oy + f.base - g.0.height as i32 - g.0.ymin;
            for (k, &a) in g.1.iter().enumerate() {
                let (px, py) = (gx + (k % g.0.width) as i32, gy + (k / g.0.width) as i32);
                if a < 24 || px < c.left || py < c.top || px >= right || py >= bottom {
                    continue;
                }
                let p = &mut m.px[(py * m.w + px) as usize];
                // a little heavier than it comes, as small light letters on dark are hard to read
                let a = (a as u32 * 5 / 4).min(255);
                let mixed = |s: u32| ((ink >> s & 0xff) * a + (*p >> s & 0xff) * (255 - a)) / 255;
                *p = mixed(16) << 16 | mixed(8) << 8 | mixed(0);
            }
        }
    }
}

pub(crate) unsafe fn letters(h: HDC, text: &[char], x: i32, y: i32, f: Font, clip: RECT) {
    if let Some(d) = dc(h) {
        d.letters(text, x, y, f, clip);
    }
}

/// Break text into lines no longer than `most` letters.
pub(crate) fn wrap(text: &[char], most: usize) -> Vec<(usize, usize)> {
    let most = most.max(1);
    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let end = text[start..].iter().position(|c| *c == '\n').map_or(text.len(), |n| start + n);
        let mut a = start;
        while end - a > most {
            // the last space that still fits, or in the middle of a word
            let cut = text[a..=a + most].iter().rposition(|c| *c == ' ').filter(|n| *n > 0).map_or(a + most, |n| a + n + 1);
            out.push((a, cut));
            a = cut;
        }
        out.push((a, end));
        if end == text.len() {
            return out;
        }
        start = end + 1;
    }
}

unsafe fn draw_text(d: &mut Dc, text: &[char], r: &mut RECT, flags: u32) -> i32 {
    let f = font(d.font as HFONT);
    let wide = r.right - r.left;
    if flags & DT_SINGLELINE != 0 {
        let mut t: Vec<char> = text.iter().copied().filter(|c| *c != '\n' && *c != '\r').collect();
        if flags & DT_CALCRECT != 0 {
            r.right = r.left + t.len() as i32 * f.adv;
            r.bottom = r.top + f.line;
            return f.line;
        }
        let fits = (wide / f.adv).max(0) as usize;
        if flags & DT_END_ELLIPSIS != 0 && t.len() > fits {
            t.truncate(fits.saturating_sub(3));
            t.extend(['.'; 3].iter().take(fits));
        }
        let w = t.len() as i32 * f.adv;
        let x = if flags & DT_CENTER != 0 { r.left + (wide - w) / 2 } else if flags & DT_RIGHT != 0 { r.right - w } else { r.left };
        let y = if flags & DT_VCENTER != 0 { r.top + (r.bottom - r.top - f.line) / 2 } else { r.top };
        d.letters(&t, x, y, f, *r);
        return f.line;
    }
    let text: Vec<char> = text.iter().copied().filter(|c| *c != '\r').collect();
    let lines = wrap(&text, if flags & DT_WORDBREAK != 0 { (wide / f.adv).max(1) as usize } else { usize::MAX / 2 });
    let h = lines.len() as i32 * f.line;
    if flags & DT_CALCRECT != 0 {
        let most = lines.iter().map(|(a, b)| text[*a..*b].iter().rposition(|c| *c != ' ').map_or(0, |n| n + 1)).max().unwrap_or(0);
        r.right = r.left + most as i32 * f.adv;
        r.bottom = r.top + h;
        return h;
    }
    for (i, (a, b)) in lines.iter().enumerate() {
        let line = &text[*a..*b];
        let len = line.iter().rposition(|c| *c != ' ').map_or(0, |n| n + 1);
        let w = len as i32 * f.adv;
        let x = if flags & DT_CENTER != 0 { r.left + (wide - w) / 2 } else if flags & DT_RIGHT != 0 { r.right - w } else { r.left };
        d.letters(&line[..len], x, r.top + i as i32 * f.line, f, *r);
    }
    h
}

// ---- what the pet calls ----

#[no_mangle]
pub unsafe extern "system" fn CreateCompatibleDC(_: HDC) -> HDC {
    dc_on(std::ptr::null_mut(), (0, 0), ALL)
}

#[no_mangle]
pub unsafe extern "system" fn DeleteDC(h: HDC) -> BOOL {
    free(h as *mut Obj);
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetDC(_: HWND) -> HDC {
    dc_on(std::ptr::null_mut(), (0, 0), ALL)
}

#[no_mangle]
pub unsafe extern "system" fn ReleaseDC(_: HWND, h: HDC) -> i32 {
    free(h as *mut Obj);
    1
}

#[no_mangle]
pub unsafe extern "system" fn CreateCompatibleBitmap(_: HDC, w: i32, h: i32) -> HBITMAP {
    new_bitmap(w, h) as HBITMAP
}

#[no_mangle]
pub unsafe extern "system" fn CreateDIBSection(_: HDC, bi: *const BITMAPINFO, _: u32, bits: *mut *mut std::ffi::c_void, _: HANDLE, _: u32) -> HBITMAP {
    let o = new_bitmap((*bi).bmiHeader.biWidth, (*bi).bmiHeader.biHeight.abs());
    if let Some(b) = bitmap(o) {
        *bits = b.px.as_mut_ptr() as *mut std::ffi::c_void;
    }
    o as HBITMAP
}

#[no_mangle]
pub unsafe extern "system" fn GdiFlush() -> BOOL {
    1
}

#[no_mangle]
pub unsafe extern "system" fn SelectObject(h: HDC, o: HGDIOBJ) -> HGDIOBJ {
    let Some(d) = dc(h) else { return std::ptr::null_mut() };
    let o = o as *mut Obj;
    let slot = match (o as usize > 4096).then(|| &*o) {
        Some(Obj::Bitmap(_)) => &mut d.bmp,
        Some(Obj::Font(_)) => &mut d.font,
        Some(Obj::Pen(..)) => &mut d.pen,
        _ => return std::ptr::null_mut(),
    };
    std::mem::replace(slot, o) as HGDIOBJ
}

#[no_mangle]
pub unsafe extern "system" fn DeleteObject(o: HGDIOBJ) -> BOOL {
    free(o as *mut Obj);
    1
}

#[no_mangle]
pub unsafe extern "system" fn GetStockObject(_: i32) -> HGDIOBJ {
    STOCK_DC_BRUSH as HGDIOBJ
}

#[no_mangle]
pub unsafe extern "system" fn SetDCBrushColor(h: HDC, c: COLORREF) -> COLORREF {
    dc(h).map_or(0, |d| std::mem::replace(&mut d.brush, swap(c)))
}

#[no_mangle]
pub unsafe extern "system" fn SetTextColor(h: HDC, c: COLORREF) -> COLORREF {
    dc(h).map_or(0, |d| swap(std::mem::replace(&mut d.text, swap(c))))
}

#[no_mangle]
pub unsafe extern "system" fn SetBkColor(h: HDC, c: COLORREF) -> COLORREF {
    dc(h).map_or(0, |d| swap(std::mem::replace(&mut d.back, swap(c))))
}

#[no_mangle]
pub unsafe extern "system" fn SetBkMode(h: HDC, mode: i32) -> i32 {
    dc(h).map_or(0, |d| std::mem::replace(&mut d.opaque, mode == 2) as i32 + 1)
}

#[no_mangle]
pub unsafe extern "system" fn FillRect(h: HDC, r: *const RECT, _: HBRUSH) -> i32 {
    if let Some(d) = dc(h) {
        let (r, c) = (*r, d.brush);
        d.fill(r.left, r.top, r.right, r.bottom, c);
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn CreatePen(_: PEN_STYLE, width: i32, c: COLORREF) -> HPEN {
    boxed(Obj::Pen(width.max(1), swap(c))) as HPEN
}

#[no_mangle]
pub unsafe extern "system" fn MoveToEx(h: HDC, x: i32, y: i32, old: *mut POINT) -> BOOL {
    if let Some(d) = dc(h) {
        if !old.is_null() {
            *old = POINT { x: d.at.0, y: d.at.1 };
        }
        d.at = (x, y);
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn LineTo(h: HDC, x: i32, y: i32) -> BOOL {
    let Some(d) = dc(h) else { return 0 };
    let (a, b) = (std::mem::replace(&mut d.at, (x, y)), (x, y));
    let (w, c) = match (d.pen as usize > 4096).then(|| &*d.pen) {
        Some(Obj::Pen(w, c)) => (*w as f32, *c),
        _ => (1.0, 0),
    };
    let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    let len = dx.hypot(dy).max(1.0);
    if dx.abs() >= dy.abs() {
        // upright slices
        let half = w * len / dx.abs().max(1.0) / 2.0;
        let (l, r) = (a.0.min(b.0), a.0.max(b.0));
        for x in l..=r {
            let y = a.1 as f32 + dy * (x - a.0) as f32 / if dx == 0.0 { 1.0 } else { dx };
            let t = (y - half + 0.5).floor() as i32;
            d.fill(x, t, x + 1, t + (2.0 * half).round().max(1.0) as i32, c);
        }
    } else {
        let half = w * len / dy.abs().max(1.0) / 2.0;
        let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
        for y in t..=bt {
            let x = a.0 as f32 + dx * (y - a.1) as f32 / dy;
            let l = (x - half + 0.5).floor() as i32;
            d.fill(l, y, l + (2.0 * half).round().max(1.0) as i32, y + 1, c);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn BitBlt(to: HDC, x: i32, y: i32, w: i32, h: i32, from: HDC, sx: i32, sy: i32, _: ROP_CODE) -> BOOL {
    let (Some(d), Some(s)) = (dc(to), dc(from)) else { return 0 };
    let (x, y, sx, sy) = (x + d.org.0, y + d.org.1, sx + s.org.0, sy + s.org.1);
    let clip = d.clip;
    if d.bmp == s.bmp {
        return 0;
    }
    let (Some(dm), Some(sm)) = (bitmap(d.bmp), bitmap(s.bmp)) else { return 0 };
    for j in 0..h {
        let (ty, fy) = (y + j, sy + j);
        if ty < clip.top.max(0) || ty >= clip.bottom.min(dm.h) || fy < 0 || fy >= sm.h {
            continue;
        }
        let skip = (clip.left.max(0) - x).max(-sx).max(0);
        let n = (w.min(clip.right.min(dm.w).saturating_sub(x)).min(sm.w - sx) - skip).max(0) as usize;
        let (ti, fi) = ((ty * dm.w + x + skip) as usize, (fy * sm.w + sx + skip) as usize);
        if n > 0 {
            dm.px[ti..ti + n].copy_from_slice(&sm.px[fi..fi + n]);
        }
    }
    1
}

#[no_mangle]
pub unsafe extern "system" fn CreateFontW(h: i32, _: i32, _: i32, _: i32, _: i32, _: u32, _: u32, _: u32, _: u32, _: u32, _: u32, _: u32, _: u32, _: *const u16) -> HFONT {
    boxed(Obj::Font(font_of(h.abs()))) as HFONT
}

#[no_mangle]
pub unsafe extern "system" fn DrawTextW(h: HDC, text: *const u16, n: i32, r: *mut RECT, flags: u32) -> i32 {
    let Some(d) = dc(h) else { return 0 };
    let t: Vec<char> = if n < 0 { string(text) } else { String::from_utf16_lossy(std::slice::from_raw_parts(text, n as usize)) }.chars().collect();
    draw_text(d, &t, &mut *r, flags)
}

#[no_mangle]
pub unsafe extern "system" fn GetDeviceCaps(_: HDC, _: i32) -> i32 {
    (super::scale() * 96.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    #[test]
    fn filling_stays_inside_the_picture_and_the_clip() {
        unsafe {
            let m = new_bitmap(10, 10);
            let h = dc_on(m, (2, 2), RECT { left: 0, top: 0, right: 8, bottom: 8 });
            SetDCBrushColor(h, 0x0000ff); // red
            FillRect(h, &RECT { left: -50, top: -50, right: 50, bottom: 50 }, std::ptr::null_mut());
            let b = bitmap(m).unwrap();
            assert_eq!(b.px.iter().filter(|p| **p == 0xff0000).count(), 64);
            assert_eq!(b.px[99], 0);
            DeleteDC(h);
            free(m);
        }
    }

    #[test]
    fn pictures_are_copied_with_every_kind_of_overhang() {
        unsafe {
            let (a, b) = (new_bitmap(8, 8), new_bitmap(6, 6));
            bitmap(b).unwrap().px.fill(7);
            let (da, db) = (dc_on(a, (0, 0), ALL), dc_on(b, (0, 0), ALL));
            for (x, y, w, h, sx, sy) in [(-3, -3, 6, 6, 0, 0), (5, 5, 6, 6, 0, 0), (0, 0, 100, 100, 3, 3), (0, 0, 6, 6, -2, -2), (100, 100, 5, 5, 0, 0)] {
                BitBlt(da, x, y, w, h, db, sx, sy, SRCCOPY);
            }
            assert!(bitmap(a).unwrap().px.iter().any(|p| *p == 7));
            for h in [da, db] {
                DeleteDC(h);
            }
            free(a);
            free(b);
        }
    }

    #[test]
    fn text_is_measured_as_it_is_laid_out_and_leaves_its_mark() {
        unsafe {
            let m = new_bitmap(300, 100);
            let h = dc_on(m, (0, 0), ALL);
            let f = CreateFontW(-14, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, std::ptr::null());
            SelectObject(h, f);
            SetTextColor(h, 0xffffff);
            let mut r = RECT { left: 0, top: 0, right: 300, bottom: 100 };
            let t = wide("HELLO PET");
            DrawTextW(h, t.as_ptr(), -1, &mut r, DT_SINGLELINE | DT_CALCRECT);
            assert_eq!(r.right, 9 * 8, "as wide as Consolas at this size");
            let mut r = RECT { left: 0, top: 0, right: 300, bottom: 30 };
            DrawTextW(h, t.as_ptr(), -1, &mut r, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
            let lit = bitmap(m).unwrap().px.iter().filter(|p| **p > 0x808080).count();
            assert!((100..1500).contains(&lit), "{lit} pixels");
            assert!(bitmap(m).unwrap().px[30 * 300..].iter().all(|p| *p == 0), "below its box");
            DeleteDC(h);
            DeleteObject(f);
            free(m);
        }
    }

    #[test]
    fn lines_break_at_spaces_or_inside_words_too_long() {
        let t: Vec<char> = "one two three".chars().collect();
        let got: Vec<String> = wrap(&t, 8).iter().map(|(a, b)| t[*a..*b].iter().collect()).collect();
        assert_eq!(got, ["one two ", "three"]);
        let t: Vec<char> = "abcdefghij\n\nx".chars().collect();
        let got: Vec<String> = wrap(&t, 4).iter().map(|(a, b)| t[*a..*b].iter().collect()).collect();
        assert_eq!(got, ["abcd", "efgh", "ij", "", "x"]);
        assert_eq!(wrap(&[], 5), [(0, 0)]);
    }

    #[test]
    fn slanted_lines_are_as_thick_as_asked() {
        unsafe {
            let m = new_bitmap(100, 100);
            let h = dc_on(m, (0, 0), ALL);
            let p = CreatePen(0, 5, 0xffffff);
            SelectObject(h, p);
            MoveToEx(h, 10, 10, std::ptr::null_mut());
            LineTo(h, 90, 70);
            let lit = bitmap(m).unwrap().px.iter().filter(|p| **p != 0).count();
            assert!((400..650).contains(&lit), "100 long and 5 thick: {lit}");
            DeleteDC(h);
            DeleteObject(p);
            free(m);
        }
    }
}
