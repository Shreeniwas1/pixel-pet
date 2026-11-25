//! The pixel hub menu drawn inside the pet window: tiles, pet picker, options and position pages.

use crate::prelude::*;

/// Everything the hub needs to know, copied out of the global state once per paint.
pub(crate) struct HubView {
    pub open: bool,
    pub page: u8,
    pub pet: usize,
    pub theme: usize,
    pub opacity: usize,
    pub flags: [bool; 12], // facts, upload, bits, coach, system, fullscreen, wander, tokens, cache reads, autostart, download, pc
    pub size: usize,
    pub goal: u32,
    pub preset: usize,
    pub open_tasks: usize,
    pub streak: u32,
    pub focus: bool,
    pub energy: f32,
    pub guide: bool, // first-run tour still running: explain the tiles
}

pub(crate) const PAGE_OPTIONS: u8 = 2;
pub(crate) const PAGE_POSITION: u8 = 3;
/// The first options group; group `g` is page `PAGE_GROUP + g`.
pub(crate) const PAGE_GROUP: u8 = 4;

pub(crate) const OPT_ROW: i32 = 28;
pub(crate) const GROUP_ROW: i32 = 34;

/// The settings, grouped so related ones sit together. The numbers are the stable setting ids
/// that `menu_action` handles; the order here is only about presentation.
pub(crate) const OPTION_GROUPS: &[(&str, &[usize])] = &[
    ("LOOK", &[16, 17, 6, 9]),                // theme, opacity, size, position
    ("INFO BUBBLE", &[18, 1, 2, 19, 13, 14]), // download, upload, bits, CPU and memory, AI tokens, cache reads
    ("PET", &[12, 0, 10, 11]),                // wander, facts, tell me a fact, replay intro
    ("FOCUS AND CARE", &[7, 8, 3, 4]),        // daily goal, pomodoro, break coach, system alerts
    ("WINDOWS", &[5, 15]),                    // hide in fullscreen, start with Windows
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
pub(crate) fn page_height(page: u8, guide: bool, u: &dyn Fn(i32) -> i32) -> i32 {
    let gap = u(6);
    match page {
        0 => 4 * u(52) + 3 * gap + if guide { u(62) + gap } else { 0 },
        1 => {
            let rows = (PETS.len() as i32 + 1) / 2;
            rows * u(64) + (rows - 1) * gap
        }
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
            flags: [s.facts_on, s.show_up, s.bits, s.care_on, s.sys_on, s.dnd_on, s.walk_on, s.tokens_on, s.cache_on, s.autostart, s.show_speed, s.show_pc],
            size: s.size,
            goal: s.stats.goal,
            preset: s.pomo_preset,
            open_tasks: s.todos.iter().filter(|t| !t.done).count(),
            streak: s.stats.streak((local_now().0 / 1440) as u32),
            focus: s.focus,
            energy: s.mood.energy,
            guide: s.intro != crate::intro::DONE,
    }
}

/// Draw the hub above (or below) the pet when it is open. Returns the click targets and the top
/// edge of the panel (so reminders can cover it). `base_y`/`ch` locate the pet; `flipy` is true
/// when the pet sits at the top of its window.
pub(crate) unsafe fn draw_hub(dc: HDC, scale: f32, w: i32, base_y: i32, ch: i32, flipy: bool, ms: &HubView) -> (Vec<(RECT, MAct)>, Option<i32>) {
    let u = |v: i32| (v as f32 * scale).round() as i32;
    let mut mrects: Vec<(RECT, MAct)> = Vec::new();
    let mut menu_top: Option<i32> = None;
    if ms.open {
        let (fpad, fb) = (u(8), u(3));
        let (x0, pw) = (u(2), w - u(2) - u(6));
        let gap = u(6);
        let iw = pw - 2 * (fb + fpad);
        let ix = x0 + fb + fpad;
        let title_h = u(24);
        let rows_h = page_height(ms.page, ms.guide, &u);
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
        let title = match group_rows(ms.page) {
            Some((name, _)) => name,
            None => ["PET HUB", "CHOOSE A PET", "OPTIONS", "POSITION"][ms.page.min(3) as usize],
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
                let tw = (iw - gap) / 2;
                let th = u(52);
                let items: [(&str, &[&[u8]], u32, MAct, String); 7] = [
                    ("TASKS", &ICON_TASKS, UP_BLUE, MAct::Tasks, format!("{} OPEN", ms.open_tasks)),
                    ("FOCUS", &ICON_FOCUS, TOMATO, MAct::Focus, (if ms.focus { "ON" } else { "OFF" }).to_string()),
                    ("STATS", &ICON_RECAP, GREEN, MAct::Stats, format!("{}D STREAK", ms.streak)),
                    ("PETS", &ICON_PETS, PINK, MAct::Pets, PETS[ms.pet].name.to_uppercase()),
                    ("OPTIONS", &ICON_OPTS, ALERT_ORANGE, MAct::Options, "SETTINGS".to_string()),
                    ("QUIT", &ICON_QUIT, RED, MAct::Quit, "BYE!".to_string()),
                    ("NAP", &ICON_NAP, UP_BLUE, MAct::Nap, format!("ENERGY {}%", ms.energy.round() as i32)),
                ];
                for (i, (label, icon, accent, act, sub)) in items.into_iter().enumerate() {
                    let (cx0, cy0) = (ix + (i as i32 % 2) * (tw + gap), top + (i as i32 / 2) * (th + gap));
                    // the last tile (NAP) spans the full width
                    let wide = i == 6;
                    let r = RECT { left: cx0, top: cy0, right: if wide { ix + iw } else { cx0 + tw }, bottom: cy0 + th };
                    let tw = if wide { iw } else { tw };
                    tile(dc, r, accent, false);
                    let ip = u(3);
                    let ih = icon.len() as i32 * ip;
                    sprite(dc, icon, cx0 + u(10), cy0 + (th - ih) / 2, ip, &|c| (c == b'X').then_some(accent));
                    SetTextColor(dc, CREAM);
                    let mut lr = RECT { left: cx0 + u(40), top: cy0 + u(8), right: cx0 + tw - u(4), bottom: cy0 + u(26) };
                    text_at(dc, label, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    SetTextColor(dc, theme().dim);
                    let mut sr = RECT { left: cx0 + u(40), top: cy0 + u(27), right: cx0 + tw - u(4), bottom: cy0 + u(45) };
                    text_at(dc, &sub, &mut sr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    mrects.push((r, act));
                }
                if ms.guide {
                    let gy = top + 4 * (th + gap);
                    fill(dc, ix, gy, iw, u(62), theme().panel);
                    SetTextColor(dc, theme().hint);
                    let mut gr = RECT { left: ix + u(6), top: gy + u(3), right: ix + iw - u(6), bottom: gy + u(59) };
                    text_at(dc, "Tasks: to-dos. Focus: timer. Stats: progress. Pets: pick a buddy. Options: settings. Nap: quick energy. X closes.", &mut gr, DT_WORDBREAK);
                }
            }
            1 => {
                let tw = (iw - gap) / 2;
                let th = u(64);
                for (i, p) in PETS.iter().enumerate() {
                    let (cx0, cy0) = (ix + (i as i32 % 2) * (tw + gap), top + (i as i32 / 2) * (th + gap));
                    let r = RECT { left: cx0, top: cy0, right: cx0 + tw, bottom: cy0 + th };
                    tile(dc, r, theme().panel_edge, i == ms.pet);
                    let pp = u(2);
                    let ph2 = p.grid.len() as i32 * pp;
                    sprite(dc, p.grid, cx0 + u(8), cy0 + (th - ph2) / 2, pp, &pet_color);
                    SetTextColor(dc, if i == ms.pet { GREEN } else { CREAM });
                    let mut nr = RECT { left: cx0 + u(8) + 16 * pp + u(6), top: cy0, right: cx0 + tw - u(4), bottom: cy0 + th };
                    text_at(dc, &p.name.to_uppercase(), &mut nr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    mrects.push((r, MAct::SetPet(i)));
                }
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
                let rows: [(&str, Option<bool>, String); 20] = [
                    ("SHOW FACTS", Some(ms.flags[0]), String::new()),
                    ("UPLOAD SPEED", Some(ms.flags[1]), String::new()),
                    ("SPEED IN BITS", Some(ms.flags[2]), String::new()),
                    ("BREAK COACH", Some(ms.flags[3]), String::new()),
                    ("SYSTEM ALERTS", Some(ms.flags[4]), String::new()),
                    ("HIDE IN FULLSCREEN", Some(ms.flags[5]), String::new()),
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
                ];
                let rh = u(OPT_ROW);
                for (n, i) in group_rows(ms.page).map_or(&[][..], |(_, rows)| rows).iter().enumerate() {
                    let (label, flag, val) = &rows[*i];
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
        let mut seen = [0u32; 20];
        for (name, rows) in OPTION_GROUPS {
            assert!(!name.is_empty() && !rows.is_empty());
            for i in *rows {
                seen[*i] += 1;
            }
        }
        assert!(seen.iter().all(|n| *n == 1), "each of the 20 settings once: {seen:?}");
    }

    #[test]
    fn the_hub_window_is_tall_enough_for_every_page_and_the_biggest_pet() {
        let rows = PETS.iter().map(|p| p.grid.len() as i32).max().unwrap();
        for scale in [0.75f32, 1.0, 1.25, 1.5, 2.0] {
            let u = move |v: i32| (v as f32 * scale).round() as i32;
            for page in 0..PAGE_GROUP + OPTION_GROUPS.len() as u8 {
                let panel = 2 * (u(3) + u(8)) + u(24) + u(6) + page_height(page, true, &u);
                let pet = rows * u(PX) + u(12) + u(8);
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
