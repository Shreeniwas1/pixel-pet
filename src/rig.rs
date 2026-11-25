//! Draws a pet in a pose: body parts onto a small canvas, then the canvas onto the window,
//! stretched, leaned and turned. Plain memory, no Win32, so poses can be rendered in tests.

use crate::prelude::*;

/// A cell of pixel art is SUB x SUB canvas pixels, so limbs and eyelids can move by thirds.
pub(crate) const SUB: i32 = 3;
const CW: i32 = 40; // canvas size in cells
const CH: i32 = 40;
const FLOOR: i32 = 35; // cells above the floor line
const W: i32 = CW * SUB;
const H: i32 = CH * SUB;
const NONE: u32 = u32::MAX;

pub(crate) struct Canvas {
    px: Vec<u32>, // colours as COLORREF, NONE where see-through
    used: (i32, i32, i32, i32), // the part that was drawn on: left, top, right, bottom
}

fn sub(v: f32) -> i32 {
    (v * SUB as f32).round() as i32
}

impl Canvas {
    fn new() -> Canvas {
        Canvas { px: vec![NONE; (W * H) as usize], used: (W, H, 0, 0) }
    }

    /// Canvas-pixel block; `x`, `y` in canvas pixels from the centre of the floor line (y down).
    fn block(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        let (x0, y0) = ((W / 2 + x).max(0), (FLOOR * SUB + y).max(0));
        let (x1, y1) = ((W / 2 + x + w).min(W), (FLOOR * SUB + y + h).min(H));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            self.px[(row * W + x0) as usize..(row * W + x1) as usize].fill(c);
        }
        let u = self.used;
        self.used = (u.0.min(x0), u.1.min(y0), u.2.max(x1), u.3.max(y1));
    }

    fn cell(&mut self, x: f32, y: f32, w: f32, h: f32, c: u32) {
        self.block(sub(x), sub(y), sub(w), sub(h), c);
    }

    fn grid(&mut self, rows: &[&[u8]], x: f32, y: f32) {
        for (j, row) in rows.iter().enumerate() {
            for (i, &ch) in row.iter().enumerate() {
                if let Some(c) = pet_color(ch) {
                    self.cell(x + i as f32, y + j as f32, 1.0, 1.0, c);
                }
            }
        }
    }

    /// A chain of square blocks with one outline around all of them.
    fn chain(&mut self, pts: &[(f32, f32, f32, u32)]) {
        for &(x, y, s, _) in pts {
            self.cell(x - s / 2.0 - 1.0, y - s / 2.0 - 1.0, s + 2.0, s + 2.0, PET_INK);
        }
        for &(x, y, s, c) in pts {
            self.cell(x - s / 2.0, y - s / 2.0, s, s, c);
        }
    }

    fn at(&self, x: i32, y: i32) -> u32 {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.px[(y * W + x) as usize]
        } else {
            NONE
        }
    }
}

fn col(c: u8) -> u32 {
    pet_color(c).unwrap_or(CREAM)
}

fn mix(a: u32, b: u32, k: f32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = ((a >> s & 0xff) as f32, (b >> s & 0xff) as f32);
        ((x + (y - x) * k) as u32) << s
    };
    ch(0) | ch(8) | ch(16)
}

fn arm(pet: &Pet, a: &Arms, side: f32, deg: f32, shoulder: (f32, f32), front: bool) -> Vec<(f32, f32, f32, u32)> {
    let rad = deg.to_radians();
    let n = a.len + 1;
    (0..n)
        .map(|i| {
            let d = i as f32;
            // a front arm bends in towards the face, the others swing out from the body
            let out = if front { -0.75 } else { 1.0 };
            let x = shoulder.0 * side + side * out * rad.sin() * d;
            let y = shoulder.1 + rad.cos() * d;
            let c = if i + 1 == n { a.hand } else { a.c };
            (x, y, if pet.torso.is_empty() { 2.0 } else { 1.6 }, col(c))
        })
        .collect()
}

fn draw_eyes(cv: &mut Canvas, pet: &Pet, r: &Rig, x0: f32, y0: f32) {
    let at = |j: i32, i: i32| pet.head.get(j as usize).and_then(|w| w.get(i as usize)).copied().unwrap_or(b'.');
    let (gw, gh) = (pet.w(), pet.head.len() as i32);
    let face = col(pet.face);
    for j in 0..gh {
        for i in 0..gw {
            if at(j, i) != b'E' || (i > 0 && at(j, i - 1) == b'E') || (j > 0 && at(j - 1, i) == b'E') {
                continue;
            }
            let ew = (i..gw).take_while(|&k| at(j, k) == b'E').count() as i32;
            let eh = (j..gh).take_while(|&k| at(k, i) == b'E').count() as i32;
            // where the eye may wander without leaving the face
            let dir = if r.look > 0.1 { 1 } else if r.look < -0.1 { -1 } else { 0 };
            let edge = if dir > 0 { i + ew } else { i - 1 };
            let free = dir != 0 && (0..eh).all(|k| at(j + k, edge) == pet.face);
            let shift = if free { sub(r.look) } else { 0 };
            cv.cell(x0 + i as f32, y0 + j as f32, ew as f32, eh as f32, face);
            let (bx, by) = (sub(x0 + i as f32) + shift, sub(y0 + j as f32));
            let (w, h) = (ew * SUB, eh * SUB);
            let lid = (h - SUB).max(1);
            match r.eyes {
                Eyes::Open | Eyes::Half | Eyes::Angry => {
                    cv.block(bx, by, w, h, PET_INK);
                    if ew >= 2 && r.eyes != Eyes::Half {
                        cv.block(bx + 1, by + 1, 2, 2, CREAM); // a shiny highlight
                    }
                    if r.eyes == Eyes::Half {
                        cv.block(bx, by, w, h / 2, face);
                    }
                    if r.eyes == Eyes::Angry {
                        // brows slanting down towards the nose
                        for k in 0..w {
                            let left = i + ew <= gw / 2;
                            let d = if left { k } else { w - 1 - k };
                            cv.block(bx + k, by - 2, 1, (d * h / (2 * w) + 2).max(1), face);
                            cv.block(bx + k, by - 2 + d * h / (2 * w), 1, 2, PET_INK);
                        }
                    }
                }
                Eyes::Closed => cv.block(bx, by + lid, w, 1.max(SUB / 2), PET_INK),
                Eyes::Happy => {
                    // an upturned arc
                    for k in 0..w {
                        let m = (w - 1) as f32 / 2.0;
                        let d = ((k as f32 - m).abs() / m.max(1.0) * (h as f32 * 0.45)) as i32;
                        cv.block(bx + k, by + h / 4 + d, 1, 2, PET_INK);
                    }
                }
                Eyes::Wide => {
                    cv.block(bx - 1, by - 1, w + 2, h + 2, PET_INK);
                    cv.block(bx, by, w, h, col(b'i'));
                    cv.block(bx + w / 2 - 1, by + h / 2 - 1, 2, 3, PET_INK);
                }
                Eyes::Dizzy => {
                    for k in 0..h {
                        let a = k * (w - 1) / (h - 1).max(1);
                        cv.block(bx + a, by + k, 2, 1, PET_INK);
                        cv.block(bx + w - 1 - a, by + k, 2, 1, PET_INK);
                    }
                }
            }
        }
    }
}

const PHONE: &[&[u8]] = &[b"KKKK", b"KccK", b"KccK", b"KccK", b"KKKK"];
const UMBRELLA: &[&[u8]] = &[
    b"......KKKKKK......",
    b"...KKKRRiiRRKKK...",
    b"..KRRRRiiiiRRRRK..",
    b".KiiRRRiiiiRRRiiK.",
    b"KiiiRRRiiiiRRRiiiK",
    b"KKKKKKKKKKKKKKKKKK",
    b"........KK........",
    b"........KK........",
    b"........KK........",
];
const CUP: &[&[u8]] = &[b"KKKKK.", b"KdddKK", b"KiiiKK", b"KiiiK.", b".KKK.."];

/// Draw the pet in this pose, facing right, standing on the floor line.
pub(crate) fn compose(pet: &Pet, r: &Rig) -> Canvas {
    let mut cv = Canvas::new();
    let leg_len = pet.legs.len as f32;
    let tuck = if pet.build == Build::Legs { (leg_len * r.crouch * SUB as f32).round() / SUB as f32 } else { 0.0 };
    let bottom = -(pet.lift() as f32) + tuck; // where the body starts
    let hw = pet.head[0].len() as f32;
    let tw = pet.torso.first().map_or(hw, |t| t.len() as f32);
    let torso_y = bottom - pet.torso.len() as f32;
    let head_y = bottom - pet.body_h() as f32 + sub(r.head.1) as f32 / SUB as f32;
    let head_x = -hw / 2.0 + sub(r.head.0) as f32 / SUB as f32;

    // tail, behind everything, curling up from the back
    if let Some(t) = &pet.tail {
        let mut pts = Vec::new();
        for i in 0..t.len {
            let d = i as f32;
            let curl = 0.5 + 0.16 * d + 0.3 * r.tail;
            let x = -tw / 2.0 - 0.6 - d * curl.cos() * 0.95 + 0.25 * r.tail * d;
            let y = bottom - 1.8 - d * curl.sin() * 0.95;
            pts.push((x, y, 1.6, col(if i + 2 > t.len { t.tip } else { t.c })));
        }
        cv.chain(&pts);
    }

    match pet.build {
        Build::Legs => {
            for (n, &(fwd, lift)) in r.legs.iter().enumerate() {
                let hip = if n == 0 { -pet.legs.hip } else { pet.legs.hip };
                let reach = leg_len - tuck;
                let up = (lift * leg_len.min(2.0) * 0.6).min(reach.max(0.0));
                let ankle = -1.0 - up;
                let top = bottom;
                let mut pts = Vec::new();
                let steps = (reach.ceil() as i32).max(1);
                for i in 0..=steps {
                    let k = i as f32 / steps as f32;
                    pts.push((hip + fwd * k, top + (ankle - top) * k - 0.0, 2.0, col(pet.legs.c)));
                }
                let (fx, fy) = (hip + fwd, ankle);
                for &(x, y, _, _) in &pts {
                    cv.cell(x - 2.0, y - 1.0, 4.0, 1.0 + 0.34, PET_INK);
                }
                cv.cell(fx - 2.0, fy - 1.0, 5.0, 2.0, PET_INK);
                for &(x, y, _, c) in &pts {
                    cv.cell(x - 1.0, y - 1.0, 2.0, 1.0, c);
                }
                cv.cell(fx - 1.0, fy - 1.0, 3.0, 1.0, col(pet.legs.foot));
            }
        }
        Build::Wheels => {}
        Build::Hover => {
            if pet.arms.is_some() {
                // a ghost's wavy hem
                let n = hw as i32;
                for i in 0..n {
                    let edge = i == 0 || i == n - 1;
                    let wave = (i as f32 * 1.25 + r.hem * 5.0).sin();
                    let drop = ((1.3 + 1.1 * wave) * SUB as f32).round() as i32;
                    let x = sub(-hw / 2.0 + i as f32);
                    cv.block(x, sub(bottom) - 1, SUB, drop + SUB, PET_INK);
                    if !edge {
                        cv.block(x, sub(bottom) - 1, SUB, drop.max(0) + 1, col(pet.legs.c));
                    }
                }
            } else {
                // thrusters
                for k in [-5.0f32, 0.0, 5.0] {
                    let f = 1.0 + 0.6 * (r.hem * 17.0 + k).sin();
                    cv.cell(k - 1.0, bottom, 2.0, f, col(b'Y'));
                    cv.cell(k - 0.5, bottom, 1.0, f * 0.6, col(b'i'));
                }
            }
        }
    }

    if let Prop::Beam(k) = r.prop {
        for y in sub(bottom)..0 {
            let half = (4.0 + (y - sub(bottom)) as f32 / SUB as f32 * 0.7) * k * SUB as f32;
            for x in -(half as i32)..(half as i32) {
                if (x + y).rem_euclid(3) != 0 {
                    cv.block(x, y, 1, 1, col(b'Y'));
                }
            }
        }
    }

    cv.grid(pet.torso, -tw / 2.0, torso_y);
    let shoulder = if pet.torso.is_empty() { (hw / 2.0 - 0.6, bottom - 3.4) } else { (tw / 2.0 - 0.3, torso_y + 1.6) };
    if let (Some(a), false) = (&pet.arms, r.arms_front) {
        cv.chain(&arm(pet, a, -1.0, r.arms[0], shoulder, false));
        cv.chain(&arm(pet, a, 1.0, r.arms[1], shoulder, false));
    }

    cv.grid(pet.head, head_x, head_y);
    draw_eyes(&mut cv, pet, r, head_x, head_y);
    if r.mouth > 0.15 && pet.build != Build::Wheels {
        let (mw, mh) = (2.0 + r.mouth.min(1.3), 2.4 * r.mouth.min(1.3));
        let (mx, my) = (pet.mouth.0 - mw / 2.0, head_y + pet.mouth.1 as f32);
        cv.cell(mx, my, mw, mh, PET_INK);
        if mh > 1.2 {
            cv.cell(mx + 0.67, my + mh * 0.55, mw - 1.34, mh * 0.45 - 0.34, PINK);
        }
    }

    if pet.build == Build::Wheels {
        let rad = pet.legs.len as f32 / 2.0;
        for side in [-1.0f32, 1.0] {
            let (wx, wy) = (pet.legs.hip * side, -rad);
            let n = sub(rad) as f32;
            for y in -(n as i32)..=(n as i32) {
                for x in -(n as i32)..=(n as i32) {
                    let d = ((x * x + y * y) as f32).sqrt();
                    if d > n {
                        continue;
                    }
                    let a = ((y as f32).atan2(x as f32).to_degrees() - r.wheel).rem_euclid(90.0);
                    let c = if d > n - 2.0 {
                        PET_INK
                    } else if d > n * 0.55 {
                        col(b'N')
                    } else if a < 30.0 || d < 1.5 {
                        col(pet.legs.foot)
                    } else {
                        col(b'B')
                    };
                    cv.block(sub(wx) + x, sub(wy) + y, 1, 1, c);
                }
            }
        }
    }

    if let (Some(a), true) = (&pet.arms, r.arms_front) {
        cv.chain(&arm(pet, a, -1.0, r.arms[0], shoulder, true));
        cv.chain(&arm(pet, a, 1.0, r.arms[1], shoulder, true));
    }
    match r.prop {
        Prop::Phone => cv.grid(PHONE, 1.0, head_y + pet.mouth.1 as f32 + 0.5),
        Prop::Cup => cv.grid(CUP, 0.0, head_y + pet.mouth.1 as f32 + if r.arms[1] > 115.0 { -0.5 } else { 1.0 }),
        Prop::Food(f, gone) => {
            let g = FOODS[f.min(FOODS.len() - 1)].grid;
            let left = &g[((g.len() as f32 * gone) as usize).min(g.len())..];
            let fw = g[0].len() as f32;
            let y = head_y + pet.mouth.1 as f32 + 0.5 + (g.len() - left.len()) as f32 * 0.6;
            // the machines are filled up at the back, everyone else eats
            let (x, y) = if pet.arms.is_some() { (pet.mouth.0 - fw / 2.0, y) } else { (-hw / 2.0 - 1.0, bottom - left.len() as f32 - 1.0) };
            cv.grid(left, x, y);
        }
        Prop::Umbrella => cv.grid(UMBRELLA, if pet.arms.is_some() { shoulder.0 - 8.5 } else { -9.0 }, head_y - 6.0),
        Prop::Tongue(k) if k > 0.02 => {
            let (x, y, l) = (pet.mouth.0, head_y + pet.mouth.1 as f32 + 0.6, k * 13.0);
            cv.cell(x, y - 0.34, l, 1.67, PET_INK);
            cv.cell(x, y, l, 1.0, PINK);
            cv.cell(x + l - 1.0, y - 0.67, 1.67, 2.34, PINK);
        }
        _ => {}
    }

    if r.glitch > 0 {
        cv.used = (0, cv.used.1, W, cv.used.3);
        // torn scan lines
        let src = cv.px.clone();
        for y in 0..H {
            let band = (y / 4) as u32;
            let n = band.wrapping_mul(2654435761).wrapping_add(r.glitch.wrapping_mul(40503)) >> 13;
            let shift = if n % 3 == 0 { (n % 13) as i32 - 6 } else { 0 };
            for x in 0..W {
                let from = x - shift;
                cv.px[(y * W + x) as usize] = if (0..W).contains(&from) { src[(y * W + from) as usize] } else { NONE };
            }
        }
    }
    if r.red > 0.02 {
        for p in cv.px.iter_mut().filter(|p| **p != NONE && **p != PET_INK) {
            *p = mix(*p, rgb(0xff, 0x40, 0x40), r.red);
        }
    }
    cv
}

/// A 32-bit picture in memory (0x00RRGGBB, top row first).
pub(crate) struct Surface<'a> {
    pub px: &'a mut [u32],
    pub w: i32,
    pub h: i32,
}

fn dib(c: u32) -> u32 {
    (c & 0xff) << 16 | (c & 0xff00) | (c >> 16 & 0xff)
}

/// Put a composed pet onto the surface: `feet` is where the centre of its floor line goes, `px`
/// the size of one cell, `facing` 1 for right and -1 for left.
pub(crate) fn blit(dst: &mut Surface, cv: &Canvas, pet: &Pet, r: &Rig, feet: (f32, f32), px: f32, facing: i32) {
    let (sin, cos) = r.rot.to_radians().sin_cos();
    let piv = -(pet.h() as f32) * r.pivot * r.sy;
    let f = if facing < 0 { -1.0f32 } else { 1.0 };
    // forward: scale, lean, turn about the pivot, move, mirror
    let place = |x: f32, y: f32| {
        let (x, y) = (x * r.sx, y * r.sy);
        let x = x - y * r.shear;
        let (rx, ry) = (x * cos - (y - piv) * sin, x * sin + (y - piv) * cos + piv);
        (feet.0 + f * (rx + r.dx) * px, feet.1 + (ry - r.dy) * px)
    };
    let (hx, hy) = (CW as f32 / 2.0, FLOOR as f32);
    let cells = |v: i32| v as f32 / SUB as f32;
    let (l, t, rt, b) = (cells(cv.used.0) - hx, cells(cv.used.1) - hy, cells(cv.used.2) - hx, cells(cv.used.3) - hy);
    if rt <= l || b <= t || r.alpha <= 0.0 {
        return;
    }
    let corners = [place(l, t), place(rt, t), place(l, b), place(rt, b)];
    let lo = |g: fn(&(f32, f32)) -> f32| corners.iter().map(g).fold(f32::MAX, f32::min);
    let hi = |g: fn(&(f32, f32)) -> f32| corners.iter().map(g).fold(f32::MIN, f32::max);
    let (x0, x1) = ((lo(|c| c.0) as i32).max(0), (hi(|c| c.0) as i32 + 1).min(dst.w));
    let (y0, y1) = ((lo(|c| c.1) as i32).max(0), (hi(|c| c.1) as i32 + 1).min(dst.h));
    // going one pixel right moves this far over the canvas
    let k = SUB as f32;
    let (dux, duy) = (f / px * cos, -f / px * sin);
    let (dcx, dcy) = ((dux + duy * r.shear) / r.sx * k, duy / r.sy * k);
    for y in y0..y1 {
        let rx = f * (x0 as f32 + 0.5 - feet.0) / px - r.dx;
        let ry = (y as f32 + 0.5 - feet.1) / px + r.dy - piv;
        let (ux, uy) = (rx * cos + ry * sin, -rx * sin + ry * cos + piv);
        let (bx, by) = (((ux + uy * r.shear) / r.sx + hx) * k, (uy / r.sy + hy) * k);
        for x in x0..x1 {
            let n = (x - x0) as f32;
            let (cx, cy) = ((bx + dcx * n).floor() as i32, (by + dcy * n).floor() as i32);
            let c = cv.at(cx, cy);
            if c == NONE {
                continue;
            }
            if r.alpha < 1.0 {
                // colour-keyed windows have no half-transparency, so fade by dropping canvas pixels
                let order = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5][((cy & 3) * 4 + (cx & 3)) as usize];
                if order as f32 / 16.0 >= r.alpha {
                    continue;
                }
            }
            dst.px[(y * dst.w + x) as usize] = dib(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(cv: &Canvas) -> (i32, i32, i32, i32) {
        let mut b = (W, H, -1, -1);
        for y in 0..H {
            for x in 0..W {
                if cv.at(x, y) != NONE {
                    b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
                }
            }
        }
        b
    }

    #[test]
    fn every_pet_stands_on_the_floor_inside_the_canvas() {
        for pet in PETS.iter() {
            let (x0, y0, x1, y1) = bounds(&compose(pet, &Rig::REST));
            assert!(x0 > 0 && y0 > 0 && x1 < W - 1 && y1 < H - 1, "{} is cut off", pet.name);
            let floor = FLOOR * SUB;
            if pet.build == Build::Hover {
                assert!(y1 < floor, "{} touches the floor", pet.name);
            } else {
                assert!((y1 + 1 - floor).abs() <= 1, "{} stands at {} instead of {floor}", pet.name, y1 + 1);
            }
            let tall = floor - y0;
            assert!((tall - pet.h() * SUB).abs() <= SUB, "{} is {tall} high, says {}", pet.name, pet.h() * SUB);
        }
    }

    #[test]
    fn no_pose_is_cut_off_by_the_canvas() {
        for pet in PETS.iter() {
            for &c in COMMON.iter().chain(pet.tricks).chain(&[Clip::Walk, Clip::Run, Clip::Held, Clip::Fall, Clip::Cheer, Clip::Eat, Clip::Climb, Clip::Glide]) {
                for i in 0..40 {
                    let t = i as f32 / 40.0 * c.secs().unwrap_or(2.0);
                    let r = sample(pet, c, t, &Push { food: pet.foods[0], ..Push::NONE });
                    let (x0, y0, x1, y1) = bounds(&compose(pet, &r));
                    assert!(x0 > 0 && y0 > 0 && x1 < W - 1 && y1 < H - 1, "{} {c:?} at {t:.2}", pet.name);
                }
            }
        }
    }

    #[test]
    fn facing_left_is_the_mirror_image() {
        let pet = &PETS[4];
        let cv = compose(pet, &Rig::REST);
        let draw = |facing: i32| {
            let mut px = vec![0u32; 200 * 120];
            blit(&mut Surface { px: &mut px, w: 200, h: 120 }, &cv, pet, &Rig::REST, (100.0, 100.0), 3.0, facing);
            px
        };
        let (a, b) = (draw(1), draw(-1));
        for y in 0..120 {
            for x in 0..200 {
                assert_eq!(a[y * 200 + x], b[y * 200 + 199 - x]);
            }
        }
    }

    #[test]
    fn a_vanished_pet_draws_nothing() {
        let pet = &PETS[9];
        let r = Rig { alpha: 0.0, ..Rig::REST };
        let mut px = vec![7u32; 100 * 100];
        blit(&mut Surface { px: &mut px, w: 100, h: 100 }, &compose(pet, &r), pet, &r, (50.0, 90.0), 3.0, 1);
        assert!(px.iter().all(|p| *p == 7));
    }

    #[test]
    fn no_pose_uses_the_transparent_colour() {
        for pet in PETS.iter() {
            for k in [0.0, 0.3, 0.6] {
                let r = Rig { red: k, ..Rig::REST };
                assert!(compose(pet, &r).px.iter().all(|p| *p != KEY), "{}", pet.name);
            }
        }
    }

    /// Not a check: renders contact sheets so new art and acts can be looked at.
    /// `PET_SHEET=dir cargo test contact_sheet -- --ignored`
    #[test]
    #[ignore]
    fn contact_sheet() {
        let dir = std::env::var("PET_SHEET").expect("set PET_SHEET to a folder");
        let save = |name: &str, px: &[u32], w: i32, h: i32| {
            let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
            for p in px {
                out.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
            }
            std::fs::write(format!("{dir}/{name}.ppm"), out).unwrap();
        };
        let (cell_w, cell_h, scale) = (150, 150, 4.0f32);
        let sheet = |name: &str, jobs: Vec<(usize, Clip, f32)>, cols: i32| {
            let rows = (jobs.len() as i32 + cols - 1) / cols;
            let (w, h) = (cols * cell_w, rows * cell_h);
            let mut px = vec![0x5c606eu32; (w * h) as usize];
            for (n, (p, c, t)) in jobs.into_iter().enumerate() {
                let pet = &PETS[p];
                let push = Push { food: pet.foods[0], pull: (0.35, 0.3), swing: 18.0, spin: 40.0, ..Push::NONE };
                let r = sample(pet, c, t, &push);
                let (gx, gy) = (n as i32 % cols, n as i32 / cols);
                let feet = ((gx * cell_w + cell_w / 2) as f32, (gy * cell_h + cell_h - 14) as f32);
                blit(&mut Surface { px: &mut px, w, h }, &compose(pet, &r), pet, &r, feet, scale, 1);
            }
            save(name, &px, w, h);
        };
        sheet("cast", (0..PETS.len()).map(|p| (p, Clip::Stand, 0.5)).collect(), 8);
        let walk = (0..PETS.len()).flat_map(|p| (0..4).map(move |i| (p, Clip::Walk, 0.5 + i as f32 * 0.13))).collect();
        sheet("walk", walk, 8);
        let acts = [Clip::Sleep, Clip::Held, Clip::Pull, Clip::Flinch, Clip::Dizzy, Clip::Angry, Clip::Eat, Clip::Cheer, Clip::Wave, Clip::Dance, Clip::Sit, Clip::Yawn];
        let per = (0..PETS.len()).flat_map(|p| acts.map(|c| (p, c, c.secs().map_or(0.6, |s| s * 0.42)))).collect();
        sheet("acts", per, acts.len() as i32);
        let tricks = (0..PETS.len()).flat_map(|p| PETS[p].tricks.iter().flat_map(move |&c| (0..4).map(move |i| (p, c, c.secs().unwrap() * (0.2 + 0.2 * i as f32))))).collect();
        sheet("tricks", tricks, 12);
        let fresh = [Clip::Climb, Clip::Glide];
        sheet("fresh", [0usize, 3, 4, 13].into_iter().flat_map(|p| fresh.into_iter().flat_map(move |c| [(p, c, 0.3), (p, c, 0.75)])).collect(), 10);
    }
}
