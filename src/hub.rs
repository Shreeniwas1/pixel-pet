//! The pixel hub menu drawn inside the pet window: tiles, pet picker, options and position pages.

use crate::prelude::*;

/// Everything the hub needs to know, copied out of the global state once per paint.
pub(crate) struct HubView {
    pub open: bool,
    pub page: u8,
    pub pet: usize,
    pub theme: usize,
    pub opacity: usize,
    pub flags: [bool; 16], // facts, upload, bits, coach, system, fullscreen, wander, tokens, cache reads, autostart, download, pc, on top, click through, tricks, gravity
    pub click_hits: bool,
    pub more: Vec<bool>, // settings 26 and up
    pub app: String,
    pub now: Option<media::Now>,
    pub launch: Vec<launch::Entry>,
    pub nap_after: usize,
    pub minimal: bool,
    pub size: usize,
    pub goal: u32,
    pub preset: usize,
    pub open_tasks: usize,
    pub streak: u32,
    pub focus: bool,
    pub energy: f32,
    pub happy: f32,
    pub food: f32,
    pub xp: u32,
    pub hat: usize,
    pub guide: bool, // first-run tour still running: explain the tiles
}

pub(crate) const PAGE_OPTIONS: u8 = 2;
pub(crate) const PAGE_POSITION: u8 = 3;
pub(crate) const PAGE_FEED: u8 = 4;
pub(crate) const PAGE_WARDROBE: u8 = 5;
pub(crate) const PAGE_LAUNCH: u8 = 6;
pub(crate) const PAGE_KEYS: u8 = 7;
/// The first options group; group `g` is page `PAGE_GROUP + g`.
pub(crate) const PAGE_GROUP: u8 = 8;

/// How many setting numbers there are, and the ones not in use.
#[cfg(test)]
const SETTINGS: usize = 37;
#[cfg(test)]
const RETIRED: &[usize] = &[35];

/// The group with what the pet shows and says; it has its own tile.
pub(crate) const GROUP_SHOW: u8 = 1;

pub(crate) const OPT_ROW: i32 = 28;
const TILE_H: i32 = 46;
const TILE_ROWS: i32 = 6;
const NOW_H: i32 = 26;
const PET_COLS: i32 = 4;
const PET_TILE_H: i32 = 70;
const FOOD_TILE_H: i32 = 74;
const BAR_ROW: i32 = 22;
pub(crate) const GROUP_ROW: i32 = 34;

/// The settings, grouped so related ones sit together. The numbers are the stable setting ids
/// that `menu_action` handles; the order here is only about presentation.
pub(crate) const OPTION_GROUPS: &[(&str, &[usize])] = &[
    ("LOOK", &[16, 17, 6, 9]),                // theme, opacity, size, position
    ("SHOW", &[18, 1, 2, 19, 36, 13, 14, 0, 27]), // download, upload, bits, CPU and memory, now playing, AI tokens, cache reads, facts, chatter
    ("PET", &[12, 22, 23, 24, 25, 33, 10, 11]), // wander, tricks, gravity, left click, sleep, sounds, tell me a fact, replay intro
    ("LIVELY", &[28, 34, 26, 32, 31]), // visit monitors, walk on windows, dance, coding buddy, react to the PC
    ("FOCUS AND CARE", &[7, 8, 3, 4]),        // daily goal, pomodoro, break coach, system alerts
    ("WINDOWS", &[20, 21, 5, 29, 30, 15]), // always on top, click through, hide for films and games, hide for this app, open and close key, start with Windows
];

/// Where the back button leads.
pub(crate) fn parent_page(page: u8) -> u8 {
    match page {
        PAGE_POSITION => PAGE_GROUP, // it lives under LOOK
        p if p >= PAGE_GROUP => PAGE_OPTIONS,
        _ => 0,
    }
}

/// The settings shown on this page, if it is an options group.
fn group_rows(page: u8) -> Option<&'static (&'static str, &'static [usize])> {
    page.checked_sub(PAGE_GROUP).and_then(|g| OPTION_GROUPS.get(g as usize))
}

/// Height of a page below its title bar.
pub(crate) fn page_height(page: u8, guide: bool, now: bool, u: &dyn Fn(i32) -> i32) -> i32 {
    let gap = u(6);
    match page {
        0 => TILE_ROWS * u(TILE_H) + (TILE_ROWS - 1) * gap + if guide { u(62) + gap } else { 0 } + if now { u(NOW_H) + gap } else { 0 },
        1 => {
            let rows = (PETS.len() as i32 + PET_COLS - 1) / PET_COLS;
            rows * u(PET_TILE_H) + (rows - 1) * gap
        }
        PAGE_LAUNCH => (launch::MOST as i32 + 2) * u(OPT_ROW) + u(34),
        PAGE_WARDROBE => u(40) + 3 * u(PET_TILE_H) + 2 * gap,
        PAGE_FEED => u(FOOD_TILE_H) + gap + 3 * u(BAR_ROW) + u(20),
        PAGE_OPTIONS => OPTION_GROUPS.len() as i32 * u(GROUP_ROW),
        PAGE_POSITION => 2 * u(44) + 2 * gap + u(36),
        p => group_rows(p).map_or(0, |(_, rows)| rows.len() as i32 * u(OPT_ROW)),
    }
}

pub(crate) unsafe fn hub_view() -> HubView {
    let s = ST.lock().unwrap();
    HubView {
            open: s.menu,
            page: s.page,
            pet: s.pet,
            theme: s.theme,
            opacity: s.opacity,
            flags: [s.facts_on, s.show_up, s.bits, s.care_on, s.sys_on, s.dnd_on, s.walk_on, s.tokens_on, s.cache_on, s.autostart, s.show_speed, s.show_pc, s.on_top, s.through, s.tricks_on, s.gravity],
            click_hits: s.click_hits,
            more: vec![s.dance_on, s.talk_on, s.visit_on, rules::is_listed(&s.hide_for, &s.last_app), s.shortcut, s.react_on, s.buddy_on, s.sound_on, s.perch_on, true, s.show_now],
            now: media::NOW.lock().unwrap().clone(),
            launch: s.launch.clone(),
            app: s.last_app.trim_end_matches(".exe").to_uppercase(),
            nap_after: s.nap_after,
            minimal: s.minimal,
            size: s.size,
            goal: s.stats.goal,
            preset: s.pomo_preset,
            open_tasks: s.todos.iter().filter(|t| !t.done).count(),
            streak: s.stats.streak((local_now().0 / 1440) as u32),
            focus: s.focus,
            energy: s.mood.energy,
            happy: s.mood.happy,
            food: s.mood.food,
            xp: s.xp,
            hat: body::hat(&s),
            guide: s.intro != crate::intro::DONE,
    }
}

/// Draw the hub above (or below) the pet when it is open. Returns the click targets and the top
/// edge of the panel (so reminders can cover it). `base_y`/`ch` locate the pet; `flipy` is true
/// when the pet sits at the top of its window; the panel starts at `ox`. `art` is the same picture
/// as `dc`, for drawing pets.
pub(crate) unsafe fn draw_hub(dc: HDC, art: &mut Surface, scale: f32, ox: i32, base_y: i32, ch: i32, flipy: bool, ms: &HubView) -> (Vec<(RECT, MAct)>, Option<i32>) {
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let mut mrects: Vec<(RECT, MAct)> = Vec::new();
    let mut menu_top: Option<i32> = None;
    if ms.open {
        let (fpad, fb) = (u(8), u(3));
        let (x0, pw) = (ox + u(2), u(PANEL_W) - u(2) - u(6));
        let gap = u(6);
        let iw = pw - 2 * (fb + fpad);
        let ix = x0 + fb + fpad;
        let title_h = u(24);
        let rows_h = page_height(ms.page, ms.guide, ms.now.is_some(), &u);
        let ph = 2 * (fb + fpad) + title_h + gap + rows_h;
        let py = if flipy { base_y + ch + u(12) } else { (base_y - u(12) - ph).max(u(2)) };
        menu_top = Some(py);
        fill(dc, x0, py, pw, ph, theme().accent);
        fill(dc, x0 + fb, py + fb, pw - 2 * fb, ph - 2 * fb, theme().ink);

        // title bar: back, title, close
        let ty = py + fb + fpad;
        let (bw, bh) = (u(26), u(22));
        let mut tx = ix;
        if ms.page > 0 {
            pixel_btn(dc, RECT { left: ix, top: ty, right: ix + bw, bottom: ty + bh }, "<", theme().panel_light);
            mrects.push((RECT { left: ix, top: ty, right: ix + bw, bottom: ty + bh }, MAct::Back));
            tx = ix + bw + u(8);
        }
        let pet = &PETS[ms.pet];
        let machine = pet.machine();
        let title = match group_rows(ms.page) {
            Some((name, _)) => name,
            None => ["PET HUB", "CHOOSE A PET", "OPTIONS", "POSITION", if machine { "FILL UP" } else { "FEED" }, "WARDROBE", "QUICK LAUNCH", "SHORTCUT KEYS"][ms.page.min(7) as usize],
        };
        SetTextColor(dc, theme().accent);
        let mut tr = RECT { left: tx, top: ty, right: ix + iw - bw - u(6), bottom: ty + bh };
        text_at(dc, title, &mut tr, DT_SINGLELINE | DT_VCENTER);
        let xr = RECT { left: ix + iw - bw, top: ty, right: ix + iw, bottom: ty + bh };
        pixel_btn(dc, xr, "X", BTN_RED);
        mrects.push((xr, MAct::Close));
        let top = ty + title_h + gap;

        let tile = |dc: HDC, r: RECT, accent: u32, on: bool| {
            fill(dc, r.left, r.top, r.right - r.left, r.bottom - r.top, if on { GREEN } else { accent });
            fill(dc, r.left + u(2), r.top + u(2), r.right - r.left - u(4), r.bottom - r.top - u(4), theme().panel);
        };
        match ms.page {
            0 => {
                let mut top = top;
                if let Some(n) = &ms.now {
                    // what is playing, with its buttons
                    let h = u(NOW_H);
                    fill(dc, ix, top, iw, h, theme().panel);
                    sprite(dc, &NOTE, ix + u(6), top + (h - 12 * u(1)) / 2, u(2), &|c| (c == b'X').then_some(if n.playing { GREEN } else { theme().dim }));
                    let bw = u(26);
                    for (k, (label, c)) in [("<<", media::BACK), (if n.playing { "||" } else { ">" }, media::TOGGLE), (">>", media::NEXT)].into_iter().enumerate() {
                        let r = rc(ix + iw - (3 - k as i32) * (bw + u(2)), top + u(2), bw, h - u(4));
                        pixel_btn(dc, r, label, theme().panel_light);
                        mrects.push((r, MAct::Media(c)));
                    }
                    SetTextColor(dc, CREAM);
                    let mut lr = rc(ix + u(22), top, iw - u(22) - 3 * (bw + u(2)) - u(4), h);
                    text_at(dc, &media::line(n), &mut lr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    top += h + gap;
                }
                let tw = (iw - 2 * gap) / 3;
                let th = u(TILE_H);
                let on = |b: bool| (if b { "ON" } else { "OFF" }).to_string();
                let gold = rgb(0xff, 0xe0, 0x66);
                let items: Vec<(&str, &[&[u8]], u32, MAct, String)> = vec![
                    (if machine { "FILL UP" } else { "FEED" }, &ICON_FEED, GREEN, MAct::FeedPage, format!("{}%", ms.food.round() as i32)),
                    ("TRICK", &ICON_TRICK, gold, MAct::Trick, "SHOW ME!".to_string()),
                    ("PETS", &ICON_PETS, PINK, MAct::Pets, PETS[ms.pet].name.to_uppercase()),
                    ("BALL", &ICON_BALL, TOMATO, MAct::Ball, "FETCH!".to_string()),
                    ("WARDROBE", &ICON_HAT, gold, MAct::Wardrobe, format!("LEVEL {}", wardrobe::level(ms.xp))),
                    ("CALC", &ICON_CALC, UP_BLUE, MAct::Calc, "CTL+ALT+K".to_string()),
                    ("NOTES", &ICON_NOTE, gold, MAct::Notes, "CTL+ALT+N".to_string()),
                    ("VS CODE", &ICON_CODE, UP_BLUE, MAct::Code, "CTL+ALT+D".to_string()),
                    ("LAUNCH", &ICON_LAUNCH, ALERT_ORANGE, MAct::LaunchPage, format!("{} THINGS", ms.launch.len())),
                    ("TASKS", &ICON_TASKS, UP_BLUE, MAct::Tasks, format!("{} OPEN", ms.open_tasks)),
                    ("FOCUS", &ICON_FOCUS, TOMATO, MAct::Focus, on(ms.focus)),
                    ("STATS", &ICON_RECAP, GREEN, MAct::Stats, format!("{}D STREAK", ms.streak)),
                    ("SHOW", &ICON_SHOW, GREEN, MAct::Group(GROUP_SHOW), "CHOOSE".to_string()),
                    ("OPTIONS", &ICON_OPTS, ALERT_ORANGE, MAct::Options, "SETTINGS".to_string()),
                    ("MINIMAL", &ICON_MINI, theme().hint, MAct::Minimal, on(ms.minimal)),
                    ("NAP", &ICON_NAP, UP_BLUE, MAct::Nap, format!("{}%", ms.energy.round() as i32)),
                    ("HIDE", &ICON_HIDE, theme().hint, MAct::Hide, "CTL+ALT+H".to_string()),
                    ("QUIT", &ICON_QUIT, RED, MAct::Quit, "CTL+ALT+Q".to_string()),
                ];
                for (i, (label, icon, accent, act, sub)) in items.into_iter().enumerate() {
                    let (cx0, cy0) = (ix + (i as i32 % 3) * (tw + gap), top + (i as i32 / 3) * (th + gap));
                    let r = rc(cx0, cy0, tw, th);
                    tile(dc, r, accent, false);
                    let ip = u(2);
                    let ih = icon.len() as i32 * ip;
                    sprite(dc, icon, cx0 + u(9), cy0 + (th - ih) / 2, ip, &|c| (c == b'X').then_some(accent));
                    SetTextColor(dc, CREAM);
                    let mut lr = RECT { left: cx0 + u(30), top: cy0 + u(5), right: cx0 + tw - u(3), bottom: cy0 + u(23) };
                    text_at(dc, label, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    SetTextColor(dc, theme().dim);
                    let mut sr = RECT { left: cx0 + u(30), top: cy0 + u(23), right: cx0 + tw - u(3), bottom: cy0 + u(41) };
                    text_at(dc, &sub, &mut sr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    mrects.push((r, act));
                }
                if ms.guide {
                    let gy = top + TILE_ROWS * (th + gap);
                    fill(dc, ix, gy, iw, u(62), theme().panel);
                    SetTextColor(dc, theme().hint);
                    let mut gr = RECT { left: ix + u(6), top: gy + u(3), right: ix + iw - u(6), bottom: gy + u(59) };
                    text_at(dc, "Show: choose what I display. Tasks: to-dos. Focus: timer. Stats: progress. Minimal: small and quiet. Nap: energy.", &mut gr, DT_WORDBREAK);
                }
            }
            1 => {
                let tw = (iw - (PET_COLS - 1) * gap) / PET_COLS;
                let th = u(PET_TILE_H);
                let small = CreateFontW(-u(11), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
                let old = SelectObject(dc, small);
                for (i, p) in PETS.iter().enumerate() {
                    let (cx0, cy0) = (ix + (i as i32 % PET_COLS) * (tw + gap), top + (i as i32 / PET_COLS) * (th + gap));
                    let r = rc(cx0, cy0, tw, th);
                    tile(dc, r, theme().panel_edge, i == ms.pet);
                    GdiFlush();
                    let cell = 2.4 * scale;
                    let feet = ((cx0 + tw / 2) as f32, (cy0 + th - u(18)) as f32);
                    blit(art, &compose(p, &Rig::REST), p, &Rig::REST, feet, cell, 1);
                    SetTextColor(dc, if i == ms.pet { GREEN } else { CREAM });
                    let mut nr = rc(cx0, cy0 + th - u(18), tw, u(16));
                    text_at(dc, &p.name.to_uppercase(), &mut nr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
                    mrects.push((r, MAct::SetPet(i)));
                }
                SelectObject(dc, old);
                DeleteObject(small);
            }
            PAGE_FEED => {
                let tw = (iw - 2 * gap) / 3;
                let th = u(FOOD_TILE_H);
                for (i, &f) in pet.foods.iter().enumerate() {
                    let food = &FOODS[f];
                    let r = rc(ix + i as i32 * (tw + gap), top, tw, th);
                    tile(dc, r, if i == 0 { PINK } else { theme().panel_edge }, false);
                    let p = u(4);
                    let (fw, fh) = (food.grid[0].len() as i32 * p, food.grid.len() as i32 * p);
                    sprite(dc, food.grid, r.left + (tw - fw) / 2, top + u(8) + (u(36) - fh) / 2, p, &pet_color);
                    SetTextColor(dc, CREAM);
                    let mut nr = rc(r.left, top + u(50), tw, u(18));
                    text_at(dc, food.name, &mut nr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
                    mrects.push((r, MAct::Feed(f)));
                }
                let bars = [(if machine { "TANK" } else { "FOOD" }, ms.food, GREEN), ("ENERGY", ms.energy, UP_BLUE), ("HAPPY", ms.happy, PINK)];
                let by0 = top + th + gap;
                for (i, (name, v, c)) in bars.into_iter().enumerate() {
                    let y = by0 + i as i32 * u(BAR_ROW);
                    SetTextColor(dc, theme().hint);
                    let mut lr = rc(ix + u(4), y, u(64), u(BAR_ROW));
                    text_at(dc, name, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    let (n, sg) = (16, u(2));
                    let sw = (iw - u(72) - (n - 1) * sg) / n;
                    let lit = (v / 100.0 * n as f32).round() as i32;
                    for k in 0..n {
                        let col = if k >= lit { theme().panel_light } else if v < 25.0 { RED } else { c };
                        fill(dc, ix + u(70) + k * (sw + sg), y + u(5), sw, u(BAR_ROW) - u(10), col);
                    }
                }
                SetTextColor(dc, theme().dim);
                let mut hr = rc(ix, by0 + 3 * u(BAR_ROW), iw, u(20));
                text_at(dc, "CTRL+ALT+E SERVES THE FAVOURITE", &mut hr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
            }
            PAGE_LAUNCH => {
                let rh = u(OPT_ROW);
                for i in 0..launch::MOST {
                    let r = rc(ix, top + i as i32 * rh, iw, rh);
                    if i > 0 {
                        fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    }
                    let mut lr = RECT { left: ix + u(8), right: r.right - u(40), ..r };
                    match ms.launch.get(i) {
                        Some(e) => {
                            SetTextColor(dc, CREAM);
                            text_at(dc, &e.name, &mut lr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                            let x = rc(r.right - u(26), r.top + u(4), u(22), rh - u(8));
                            pixel_btn(dc, x, "x", theme().panel_light);
                            mrects.push((x, MAct::Forget(i))); // before the row, so it wins
                            mrects.push((r, MAct::Launch(i)));
                        }
                        None => {
                            SetTextColor(dc, theme().dim);
                            text_at(dc, "-", &mut lr, DT_SINGLELINE | DT_VCENTER);
                        }
                    }
                }
                let y = top + launch::MOST as i32 * rh;
                let front = if ms.app.is_empty() { "ADD THE APP IN FRONT".to_string() } else { format!("ADD {}", ms.app.chars().take(24).collect::<String>()) };
                for (k, (label, act)) in [(front, MAct::AddFront), ("ADD THE LINK YOU COPIED".to_string(), MAct::AddLink)].into_iter().enumerate() {
                    let r = rc(ix, y + k as i32 * rh, iw, rh);
                    fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    SetTextColor(dc, GREEN);
                    let mut lr = RECT { left: ix + u(8), ..r };
                    text_at(dc, &format!("+ {label}"), &mut lr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    mrects.push((r, act));
                }
                SetTextColor(dc, theme().hint);
                let mut hr = rc(ix + u(4), y + 2 * rh + u(2), iw - u(8), u(32));
                text_at(dc, "Or drop a file, folder or shortcut on the pet.", &mut hr, DT_WORDBREAK);
            }
            PAGE_WARDROBE => {
                SetTextColor(dc, theme().accent);
                let mut lr = rc(ix + u(4), top, iw, u(18));
                text_at(dc, &format!("LEVEL {}", wardrobe::level(ms.xp)), &mut lr, DT_SINGLELINE | DT_VCENTER);
                SetTextColor(dc, theme().hint);
                let mut rr = rc(ix, top, iw - u(4), u(18));
                text_at(dc, &format!("{} / {} XP", ms.xp, wardrobe::needed(wardrobe::level(ms.xp) + 1)), &mut rr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
                fill(dc, ix, top + u(20), iw, u(10), theme().panel_light);
                fill(dc, ix, top + u(20), (iw as f32 * wardrobe::progress(ms.xp)) as i32, u(10), theme().accent);
                let tw = (iw - (PET_COLS - 1) * gap) / PET_COLS;
                let th = u(PET_TILE_H);
                let small = CreateFontW(-u(11), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
                let old = SelectObject(dc, small);
                for (i, hat) in wardrobe::HATS.iter().enumerate() {
                    let r = rc(ix + (i as i32 % PET_COLS) * (tw + gap), top + u(40) + (i as i32 / PET_COLS) * (th + gap), tw, th);
                    let open = wardrobe::worn(i, ms.xp) == i;
                    tile(dc, r, theme().panel_edge, i == ms.hat);
                    if !hat.grid.is_empty() {
                        let p = u(4);
                        let (w, h) = (hat.grid[0].len() as i32 * p, hat.grid.len() as i32 * p);
                        let shade = theme().panel_light;
                        sprite(dc, hat.grid, r.left + (tw - w) / 2, r.top + u(6) + (u(40) - h) / 2, p, &|c| if open { pet_color(c) } else { pet_color(c).map(|_| shade) });
                    }
                    SetTextColor(dc, if i == ms.hat { GREEN } else if open { CREAM } else { theme().dim });
                    let mut nr = rc(r.left, r.bottom - u(18), tw, u(16));
                    let name = if open { hat.name.to_string() } else { format!("LEVEL {}", hat.level) };
                    text_at(dc, &name, &mut nr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
                    mrects.push((r, MAct::Wear(i)));
                }
                SelectObject(dc, old);
                DeleteObject(small);
            }
            PAGE_OPTIONS => {
                let rh = u(GROUP_ROW);
                for (g, (name, _)) in OPTION_GROUPS.iter().enumerate() {
                    let r = rc(ix, top + g as i32 * rh, iw, rh);
                    if g > 0 {
                        fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    }
                    SetTextColor(dc, CREAM);
                    let mut lr = RECT { left: ix + u(8), ..r };
                    text_at(dc, name, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    SetTextColor(dc, UP_BLUE);
                    let mut vr = RECT { right: r.right - u(6), ..r };
                    text_at(dc, ">", &mut vr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
                    mrects.push((r, MAct::Group(g as u8)));
                }
            }
            PAGE_POSITION => {
                let tw = (iw - gap) / 2;
                let th = u(44);
                let names = ["BOTTOM-RIGHT", "BOTTOM-LEFT", "TOP-RIGHT", "TOP-LEFT"];
                for (i, name) in names.iter().enumerate() {
                    let (cx0, cy0) = (ix + (i as i32 % 2) * (tw + gap), top + (i as i32 / 2) * (th + gap));
                    let r = RECT { left: cx0, top: cy0, right: cx0 + tw, bottom: cy0 + th };
                    tile(dc, r, theme().panel_edge, false);
                    // a tiny screen with the pet's corner marked
                    let (sw, sh, blk) = (u(36), u(24), u(9));
                    let (sx, sy) = (cx0 + u(8), cy0 + (th - sh) / 2);
                    fill(dc, sx, sy, sw, sh, CREAM);
                    fill(dc, sx + u(2), sy + u(2), sw - u(4), sh - u(4), theme().ink);
                    let right = i % 2 == 0;
                    let bottom = i < 2;
                    let bx = if right { sx + sw - u(2) - blk } else { sx + u(2) };
                    let by = if bottom { sy + sh - u(2) - blk } else { sy + u(2) };
                    fill(dc, bx, by, blk, blk, theme().accent);
                    SetTextColor(dc, CREAM);
                    let mut nr = RECT { left: sx + sw + u(8), top: cy0, right: cx0 + tw - u(2), bottom: cy0 + th };
                    text_at(dc, name, &mut nr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    mrects.push((r, MAct::Corner(i as u8)));
                }
                let wy = top + 2 * (th + gap);
                let r = RECT { left: ix, top: wy, right: ix + iw, bottom: wy + u(36) };
                tile(dc, r, UP_BLUE, false);
                SetTextColor(dc, CREAM);
                let mut nr = r;
                text_at(dc, "MOVE TO NEXT MONITOR", &mut nr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
                mrects.push((r, MAct::NextMonitor));
            }
            _ => {
                let size_name = ["SMALL", "MEDIUM", "LARGE"][ms.size.min(2)];
                let hide_label = if ms.app.is_empty() { "HIDE FOR APP IN FRONT".to_string() } else { format!("HIDE FOR {}", ms.app.chars().take(17).collect::<String>()) };
                let more = |id: usize| Some(ms.more[id - 26]);
                let rows: Vec<(&str, Option<bool>, String)> = vec![
                    ("SHOW FACTS", Some(ms.flags[0]), String::new()),
                    ("UPLOAD SPEED", Some(ms.flags[1]), String::new()),
                    ("SPEED IN BITS", Some(ms.flags[2]), String::new()),
                    ("BREAK COACH", Some(ms.flags[3]), String::new()),
                    ("SYSTEM ALERTS", Some(ms.flags[4]), String::new()),
                    ("HIDE FOR GAMES AND FILMS", Some(ms.flags[5]), String::new()),
                    ("SIZE", None, size_name.to_string()),
                    ("DAILY GOAL", None, format!("{} SESSIONS", ms.goal)),
                    ("POMODORO", None, POMO_NAMES[ms.preset].to_uppercase()),
                    ("POSITION", None, ">".to_string()),
                    ("TELL ME A FACT", None, ">".to_string()),
                    ("REPLAY INTRO", None, ">".to_string()),
                    ("WANDER AROUND", Some(ms.flags[6]), String::new()),
                    ("AI TOKENS", Some(ms.flags[7]), String::new()),
                    ("COUNT CACHE READS", Some(ms.flags[8]), String::new()),
                    ("START WITH WINDOWS", Some(ms.flags[9]), String::new()),
                    ("THEME", None, THEMES[ms.theme.min(THEMES.len() - 1)].name.to_uppercase()),
                    ("PET OPACITY", None, format!("{}%", OPACITIES[ms.opacity.min(OPACITIES.len() - 1)])),
                    ("DOWNLOAD SPEED", Some(ms.flags[10]), String::new()),
                    ("CPU AND MEMORY", Some(ms.flags[11]), String::new()),
                    ("ALWAYS ON TOP", Some(ms.flags[12]), String::new()),
                    ("CLICK THROUGH", Some(ms.flags[13]), String::new()),
                    ("RANDOM TRICKS", Some(ms.flags[14]), String::new()),
                    ("GRAVITY", Some(ms.flags[15]), String::new()),
                    ("LEFT CLICK", None, (if ms.click_hits { "HITS" } else { "OPENS HUB" }).to_string()),
                    ("SLEEPS AFTER", None, NAP_NAMES[ms.nap_after.min(3)].to_string()),
                    ("DANCE TO MUSIC", more(26), String::new()),
                    ("PET CHATTER", more(27), String::new()),
                    ("VISIT OTHER MONITORS", more(28), String::new()),
                    (&hide_label, more(29), String::new()),
                    ("CTRL+ALT+Q OPENS ME TOO", more(30), String::new()),
                    ("REACT TO MY PC", more(31), String::new()),
                    ("CODING BUDDY", more(32), String::new()),
                    ("SOUNDS", more(33), String::new()),
                    ("WALK ON WINDOWS", more(34), String::new()),
                    ("", None, String::new()),
                    ("NOW PLAYING", more(36), String::new()),
                ];
                let rh = u(OPT_ROW);
                for (n, i) in group_rows(ms.page).map_or(&[][..], |(_, rows)| rows).iter().enumerate() {
                    let Some((label, flag, val)) = rows.get(*i) else { continue };
                    let r = rc(ix, top + n as i32 * rh, iw, rh);
                    if n > 0 {
                        fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    }
                    SetTextColor(dc, CREAM);
                    let mut lr = RECT { left: ix + u(8), right: r.right - u(120), ..r };
                    text_at(dc, label, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    match flag {
                        Some(on) => {
                            let bs = u(14);
                            let bx = ix + iw - bs - u(6);
                            let by = r.top + (rh - bs) / 2;
                            fill(dc, bx, by, bs, bs, CREAM);
                            fill(dc, bx + u(2), by + u(2), bs - u(4), bs - u(4), if *on { GREEN } else { theme().panel });
                        }
                        None => {
                            SetTextColor(dc, UP_BLUE);
                            let mut vr = RECT { left: r.right - u(120), right: r.right - u(6), ..r };
                            text_at(dc, val, &mut vr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
                        }
                    }
                    mrects.push((r, MAct::Opt(*i as u8)));
                }
            }
        }
    }
    (mrects, menu_top)
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn every_setting_appears_in_exactly_one_group() {
        let mut seen = [0u32; SETTINGS];
        for (name, rows) in OPTION_GROUPS {
            assert!(!name.is_empty() && !rows.is_empty());
            for i in *rows {
                seen[*i] += 1;
            }
        }
        for (i, n) in seen.iter().enumerate() {
            assert_eq!(*n, !RETIRED.contains(&i) as u32, "setting {i}");
        }
    }

    #[test]
    fn the_hub_window_is_tall_enough_for_every_page_and_the_biggest_pet() {
        let rows = PETS.iter().map(|p| p.h()).max().unwrap();
        for scale in [0.75f32, 1.0, 1.25, 1.5, 2.0] {
            let u = move |v: i32| (v as f32 * scale).round() as i32;
            for page in 0..PAGE_GROUP + OPTION_GROUPS.len() as u8 {
                let panel = 2 * (u(3) + u(8)) + u(24) + u(6) + page_height(page, true, true, &u);
                let pet = rows * u(PX) + u(12) + u(crate::body::PAD_TOP.max(crate::body::PAD_BOTTOM));
                assert!(panel + pet <= u(MENU_H), "page {page} too tall at {scale}x: {} vs {}", panel + pet, u(MENU_H));
            }
        }
    }

    #[test]
    fn back_always_leads_to_the_hub() {
        for start in 0..PAGE_GROUP + OPTION_GROUPS.len() as u8 {
            let mut p = start;
            for _ in 0..4 {
                p = parent_page(p);
            }
            assert_eq!(p, 0, "stuck going back from page {start}");
        }
    }
}
