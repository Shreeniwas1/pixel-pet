//! The pixel hub menu drawn inside the pet window: a home page with six categories, a page for
//! each, and the pages those lead to.

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
    pub friend: usize,
    pub picking_friend: bool,
    pub pet_page: usize,
    pub packs: Vec<(usize, String, usize)>,
    pub pack_sure: Option<usize>,
    pub pack_note: String,
    pub care_on: bool,
    pub clips_on: bool,
    pub lines: Vec<Line>,
    pub hint: String,
    pub keys: keys::Keys,
    pub key_wait: Option<usize>,
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

// home is page 0
pub(crate) const PAGE_PICK: u8 = 1;
pub(crate) const PAGE_OPTIONS: u8 = 2;
pub(crate) const PAGE_POSITION: u8 = 3;
pub(crate) const PAGE_FEED: u8 = 4;
pub(crate) const PAGE_WARDROBE: u8 = 5;
pub(crate) const PAGE_LAUNCH: u8 = 6;
pub(crate) const PAGE_KEYS: u8 = 7;
pub(crate) const PAGE_PACKS: u8 = 12;
pub(crate) const PAGE_BREAKS: u8 = 13;
// categories
const PAGE_PET: u8 = 8;
const PAGE_LOOKS: u8 = 9;
const PAGE_WORK: u8 = 10;
const PAGE_TOOLS: u8 = 11;
/// The first settings group; group `g` is page `PAGE_GROUP + g`.
pub(crate) const PAGE_GROUP: u8 = 14;

/// How many setting numbers there are, and the ones not in use.
#[cfg(test)]
const SETTINGS: usize = 38;
#[cfg(test)]
const RETIRED: &[usize] = &[35];

// the groups that belong to a category of their own; the others are listed under SETTINGS
const GROUP_SHOW: u8 = 0;
const GROUP_LOOK: u8 = 1;
const GROUP_FOCUS: u8 = 2;
const GROUP_LISTED: u8 = 3;
const GROUP_KEYS: u8 = 7;

pub(crate) const OPT_ROW: i32 = 28;
const TILE_H: i32 = 50;
const NOW_H: i32 = 26;
const CARD_H: i32 = 68;
const FOOT_H: i32 = 26;
const TAB_H: i32 = 22;
const PET_COLS: i32 = 4;
const TABS: i32 = 4; // in a row
const PET_TILE_H: i32 = 70;
const FOOD_TILE_H: i32 = 74;
const BAR_ROW: i32 = 22;
pub(crate) const GROUP_ROW: i32 = 42;

/// The settings, grouped so related ones sit together: name, what is in it, and the stable
/// setting ids that `menu_action` handles.
#[cfg(windows)]
pub(crate) const OPTION_GROUPS: &[(&str, &str, &[usize])] = &[
    ("SHOW", "", &[18, 1, 2, 19, 36, 13, 14, 0, 27]), // download, upload, bits, CPU and memory, now playing, AI tokens, cache reads, facts, chatter
    ("THEME AND SIZE", "", &[16, 6, 17, 9]),          // theme, size, opacity, position
    ("TIMER AND BREAKS", "", &[8, 7, 3, 4]),          // pomodoro, daily goal, break coach, system alerts
    ("BEHAVIOUR", "WALKING, TRICKS, SOUNDS, SLEEP", &[12, 22, 23, 24, 25, 33]), // wander, tricks, gravity, left click, sleep, sounds
    ("WHERE IT GOES", "MONITORS, WINDOWS, ON TOP", &[28, 34, 20, 21]), // visit monitors, walk on windows, always on top, click through
    ("REACTIONS", "MUSIC, CODING, YOUR PC", &[26, 32, 31]), // dance, coding buddy, react to the PC
    ("HIDING", "GAMES, FILMS, CHOSEN APPS", &[5, 29]), // hide for games and films, hide for this app
    ("KEYS AND STARTUP", "SHORTCUTS, START WITH WINDOWS", &[37, 30, 15, 10, 11]), // shortcut keys, open and close key, start with Windows, tell me a fact, replay intro
];

/// What plays, dancing to it and a key that starts the pet are only there on Windows so far.
#[cfg(not(windows))]
pub(crate) const OPTION_GROUPS: &[(&str, &str, &[usize])] = &[
    ("SHOW", "", &[18, 1, 2, 19, 13, 14, 0, 27]), // download, upload, bits, CPU and memory, AI tokens, cache reads, facts, chatter
    ("THEME AND SIZE", "", &[16, 6, 17, 9]),          // theme, size, opacity, position
    ("TIMER AND BREAKS", "", &[8, 7, 3, 4]),          // pomodoro, daily goal, break coach, system alerts
    ("BEHAVIOUR", "WALKING, TRICKS, SOUNDS, SLEEP", &[12, 22, 23, 24, 25, 33]), // wander, tricks, gravity, left click, sleep, sounds
    ("WHERE IT GOES", "MONITORS, WINDOWS, ON TOP", &[28, 34, 20, 21]), // visit monitors, walk on windows, always on top, click through
    ("REACTIONS", "CODING, YOUR PC", &[32, 31]), // coding buddy, react to the PC
    ("HIDING", "GAMES, FILMS, CHOSEN APPS", &[5, 29]), // hide for games and films, hide for this app
    ("KEYS AND STARTUP", "SHORTCUTS, START AT LOGIN", &[37, 15, 10, 11]), // shortcut keys, start at login, tell me a fact, replay intro
];

/// Where the back button leads.
pub(crate) fn parent_page(page: u8) -> u8 {
    match page {
        PAGE_PICK | PAGE_WARDROBE | PAGE_PACKS => PAGE_LOOKS,
        PAGE_FEED | PAGE_BREAKS => PAGE_PET,
        PAGE_LAUNCH => PAGE_TOOLS,
        PAGE_POSITION => PAGE_GROUP + GROUP_LOOK,
        PAGE_KEYS => PAGE_GROUP + GROUP_KEYS,
        p if p >= PAGE_GROUP => match p - PAGE_GROUP {
            GROUP_SHOW => 0,
            GROUP_LOOK => PAGE_LOOKS,
            GROUP_FOCUS => PAGE_WORK,
            _ => PAGE_OPTIONS,
        },
        _ => 0,
    }
}

/// The settings shown on this page, if it is an options group.
fn group_rows(page: u8) -> Option<&'static (&'static str, &'static str, &'static [usize])> {
    page.checked_sub(PAGE_GROUP).and_then(|g| OPTION_GROUPS.get(g as usize))
}

/// What can change the height of a page.
/// The tab that is shown, the tabs of the same pack, and whether there are other packs.
fn picker(page: usize) -> (Vec<pack::Tab>, usize, std::ops::Range<usize>) {
    let tabs = pack::tabs();
    let tab = page.min(tabs.len().saturating_sub(1));
    let of = tabs[tab].pack;
    let first = tabs.iter().position(|t| t.pack == of).unwrap_or(0);
    let n = tabs.iter().filter(|t| t.pack == of).count();
    (tabs, tab, first..first + n)
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Extra {
    pub guide: bool,  // the tiles are being explained
    pub now: bool,    // something is playing
    pub pets: usize,  // on that tab
    pub tabs: usize,  // of the pack it belongs to
    pub packs: bool,  // there are imported ones
    pub lines: usize, // on a page that is a list
    pub friend: bool, // a friend is being picked
}

/// Height of a page below its title bar.
pub(crate) fn page_height(page: u8, x: Extra, u: &dyn Fn(i32) -> i32) -> i32 {
    let gap = u(6);
    let tiles = |rows: i32| rows * u(TILE_H) + (rows - 1) * gap;
    match page {
        0 => u(CARD_H) + gap + tiles(3) + gap + u(FOOT_H) + if x.guide { u(62) + gap } else { 0 } + if x.now { u(NOW_H) + gap } else { 0 },
        PAGE_LOOKS | PAGE_PET | PAGE_TOOLS => tiles(3),
        PAGE_WORK => tiles(2),
        PAGE_BREAKS => x.lines as i32 * u(OPT_ROW) + u(36),
        PAGE_PACKS => (pack::MOST as i32 + 2) * u(OPT_ROW) + u(34),
        PAGE_PICK => {
            let tabs = (x.tabs as i32 + TABS - 1) / TABS + x.packs as i32;
            let rows = (x.pets as i32 + PET_COLS - 1) / PET_COLS;
            tabs * (u(TAB_H) + u(4)) + gap + rows * u(PET_TILE_H) + (rows - 1) * gap + if x.friend { gap + u(FOOT_H) } else { 0 }
        }
        PAGE_KEYS => keys::ACTIONS.len() as i32 * u(OPT_ROW) + u(34),
        PAGE_LAUNCH => (launch::MOST as i32 + 2) * u(OPT_ROW) + u(34),
        PAGE_WARDROBE => u(40) + 3 * u(PET_TILE_H) + 2 * gap,
        PAGE_FEED => u(FOOD_TILE_H) + gap + 3 * u(BAR_ROW) + u(20),
        PAGE_OPTIONS => (OPTION_GROUPS.len() as i32 - GROUP_LISTED as i32) * u(GROUP_ROW),
        PAGE_POSITION => 2 * u(44) + 2 * gap + u(36),
        p => group_rows(p).map_or(0, |(_, _, rows)| rows.len() as i32 * u(OPT_ROW)),
    }
}

/// A list has no more lines than this, or it would not fit.
#[cfg(test)]
const MOST_LINES: usize = 14;

/// A line of a page that is a list.
#[derive(Clone, Default)]
pub(crate) struct Line {
    pub text: String,
    pub right: String,       // at the end of the line
    pub tick: Option<bool>,  // a box to tick there
    pub act: Option<MAct>,
    pub del: Option<MAct>,   // an x there
    pub tone: Tone,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Tone {
    #[default]
    Plain,
    Dim,
    Add,
    Warn,
}

impl Line {
    pub fn new(text: impl Into<String>, right: impl Into<String>, act: MAct) -> Line {
        Line { text: text.into(), right: right.into(), act: Some(act), ..Line::default() }
    }

    pub fn note(text: impl Into<String>, tone: Tone) -> Line {
        Line { text: text.into(), tone, ..Line::default() }
    }

    pub fn toned(self, tone: Tone) -> Line {
        Line { tone, ..self }
    }
}

/// What a page that is a list shows, and the words under it.
fn lines_of(page: u8, s: &State) -> (Vec<Line>, String) {
    match page {
        PAGE_BREAKS => {
            let mut l = vec![Line { text: "REMIND ME".into(), tick: Some(s.care_on), act: Some(MAct::BreakOn), ..Line::default() }];
            for (i, name) in care::NAMES.iter().enumerate() {
                let m = care::EVERY[i][s.care_every[i]];
                let when = if m == 0 { "OFF".to_string() } else { format!("EVERY {m} MIN") };
                l.push(Line::new(*name, when, MAct::BreakEvery(i)).toned(if s.care_on { Tone::Plain } else { Tone::Dim }));
            }
            l.push(Line::new("+ I DRANK A GLASS", format!("{} TODAY", s.water.1), MAct::Drank).toned(Tone::Add));
            l.push(Line::new("REST MY EYES NOW", "20 SEC", MAct::EyesNow).toned(Tone::Add));
            (l, "Only time at the keyboard counts, and I never interrupt a focus round.".into())
        }
        _ => (Vec::new(), String::new()),
    }
}

unsafe fn draw_lines(dc: HDC, mrects: &mut Vec<(RECT, MAct)>, lines: &[Line], hint: &str, ix: i32, top: i32, iw: i32, u: &dyn Fn(i32) -> i32) {
    let rh = u(OPT_ROW);
    for (i, l) in lines.iter().enumerate() {
        let r = rc(ix, top + i as i32 * rh, iw, rh);
        if i > 0 {
            fill(dc, ix, r.top, iw, 1, theme().panel_light);
        }
        let mut end = r.right - u(6);
        if let Some(d) = l.del {
            let x = rc(r.right - u(26), r.top + u(4), u(22), rh - u(8));
            pixel_btn(dc, x, "x", theme().panel_light);
            mrects.push((x, d)); // before the row, so it wins
            end = x.left - u(6);
        }
        if let Some(on) = l.tick {
            let bs = u(14);
            let (bx, by) = (end - bs, r.top + (rh - bs) / 2);
            fill(dc, bx, by, bs, bs, CREAM);
            fill(dc, bx + u(2), by + u(2), bs - u(4), bs - u(4), if on { GREEN } else { theme().panel });
            end = bx - u(6);
        }
        if !l.right.is_empty() {
            let w = l.right.chars().count() as i32 * u(8) + u(4);
            SetTextColor(dc, if l.tone == Tone::Dim { theme().dim } else { UP_BLUE });
            let mut vr = RECT { left: end - w, right: end, ..r };
            text_at(dc, &l.right, &mut vr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
            end -= w;
        }
        SetTextColor(dc, match l.tone {
            Tone::Plain => CREAM,
            Tone::Dim => theme().dim,
            Tone::Add => GREEN,
            Tone::Warn => RED,
        });
        let mut lr = RECT { left: ix + u(8), right: end, ..r };
        text_at(dc, &l.text, &mut lr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
        if let Some(a) = l.act {
            mrects.push((r, a));
        }
    }
    SetTextColor(dc, theme().hint);
    let mut hr = rc(ix + u(4), top + lines.len() as i32 * rh + u(3), iw - u(8), u(34));
    text_at(dc, hint, &mut hr, DT_WORDBREAK);
}

type Tile = (&'static str, &'static [&'static [u8]], u32, MAct, String);

/// The tiles of the home page and of the categories.
fn tiles_of(page: u8, ms: &HubView) -> Vec<Tile> {
    let gold = rgb(0xff, 0xe0, 0x66);
    let key = |id: usize| keys::label_of(&ms.keys, id);
    let machine = PETS[ms.pet].machine();
    let name = |i: usize| PETS[i].name.to_uppercase();
    match page {
        0 => vec![
            ("PET", &ICON_PETS, PINK, MAct::Page(PAGE_PET), "FEED, PLAY, NAP".to_string()),
            ("LOOKS", &ICON_HAT, gold, MAct::Page(PAGE_LOOKS), "PET, HAT, THEME".to_string()),
            ("WORK", &ICON_TASKS, UP_BLUE, MAct::Page(PAGE_WORK), if ms.focus { "FOCUS IS ON".to_string() } else { format!("{} OPEN TASKS", ms.open_tasks) }),
            ("TOOLS", &ICON_CALC, GREEN, MAct::Page(PAGE_TOOLS), "CALC, NOTES, ...".to_string()),
            ("SHOW", &ICON_SHOW, TOMATO, MAct::Group(GROUP_SHOW), "WHAT I DISPLAY".to_string()),
            ("SETTINGS", &ICON_OPTS, ALERT_ORANGE, MAct::Options, "HOW I BEHAVE".to_string()),
        ],
        PAGE_PET => vec![
            (if machine { "FILL UP" } else { "FEED" }, &ICON_FEED, GREEN, MAct::FeedPage, format!("{}% FULL", ms.food.round() as i32)),
            ("NAP", &ICON_NAP, UP_BLUE, MAct::Nap, format!("{}% ENERGY", ms.energy.round() as i32)),
            ("TRICK", &ICON_TRICK, gold, MAct::Trick, "SHOW ME!".to_string()),
            ("BALL", &ICON_BALL, TOMATO, MAct::Ball, "FETCH!".to_string()),
            ("BREAKS", &HEART, PINK, MAct::Breaks, if ms.care_on { "WATER, EYES, ..." } else { "OFF" }.to_string()),
        ],
        PAGE_LOOKS => vec![
            ("CHARACTER", &ICON_PETS, PINK, MAct::Pets, name(ms.pet)),
            ("FRIEND", &HEART, TOMATO, MAct::PickFriend, if ms.friend > 0 { name(ms.friend - 1) } else { "NONE".to_string() }),
            ("WARDROBE", &ICON_HAT, gold, MAct::Wardrobe, format!("LEVEL {}", wardrobe::level(ms.xp))),
            ("THEME, SIZE", &ICON_OPTS, UP_BLUE, MAct::Group(GROUP_LOOK), THEMES[ms.theme.min(THEMES.len() - 1)].name.to_uppercase()),
            ("PACKS", &ICON_PACK, GREEN, MAct::Packs, if ms.packs.is_empty() { "NEW CHARACTERS".to_string() } else { format!("{} IMPORTED", ms.packs.len()) }),
        ],
        PAGE_WORK => vec![
            ("TASKS", &ICON_TASKS, UP_BLUE, MAct::Tasks, format!("{} OPEN", ms.open_tasks)),
            ("FOCUS", &ICON_FOCUS, TOMATO, MAct::Focus, if ms.focus { "ON" } else { "OFF" }.to_string()),
            ("STATS", &ICON_RECAP, GREEN, MAct::Stats, format!("{} DAY STREAK", ms.streak)),
            ("TIMER", &ICON_OPTS, ALERT_ORANGE, MAct::Group(GROUP_FOCUS), "BREAKS, GOAL".to_string()),
        ],
        PAGE_TOOLS => vec![
            ("CALCULATOR", &ICON_CALC, UP_BLUE, MAct::Calc, key(9)),
            ("NOTES", &ICON_NOTE, gold, MAct::Notes, key(10)),
            ("VS CODE", &ICON_CODE, UP_BLUE, MAct::Code, key(11)),
            ("CLIPBOARD", &ICON_CLIP, GREEN, MAct::Clips, if ms.clips_on { key(12) } else { "OFF".to_string() }),
            ("LAUNCH", &ICON_LAUNCH, ALERT_ORANGE, MAct::LaunchPage, format!("{} THINGS", ms.launch.len())),
        ],
        _ => Vec::new(),
    }
}

pub(crate) unsafe fn hub_view() -> HubView {
    let s = ST.lock().unwrap();
    let (lines, hint) = if s.menu { lines_of(s.page, &s) } else { (Vec::new(), String::new()) };
    HubView {
            care_on: s.care_on,
            clips_on: s.clips_on,
            lines,
            hint,
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
            friend: s.friend,
            picking_friend: s.picking_friend,
            pet_page: s.pet_page,
            packs: pack::list(),
            pack_sure: s.pack_sure,
            pack_note: s.pack_note.clone(),
            keys: s.keys,
            key_wait: s.key_wait,
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

/// Two tiles to a row. Returns where the next thing can go.
unsafe fn draw_tiles(dc: HDC, mrects: &mut Vec<(RECT, MAct)>, items: Vec<Tile>, ix: i32, top: i32, iw: i32, u: &dyn Fn(i32) -> i32) -> i32 {
    let (gap, th) = (u(6), u(TILE_H));
    let tw = (iw - gap) / 2;
    let rows = (items.len() as i32 + 1) / 2;
    for (i, (label, icon, accent, act, sub)) in items.into_iter().enumerate() {
        let (x, y) = (ix + (i as i32 % 2) * (tw + gap), top + (i as i32 / 2) * (th + gap));
        fill(dc, x, y, tw, th, accent);
        fill(dc, x + u(2), y + u(2), tw - u(4), th - u(4), theme().panel);
        let ip = u(3);
        let (w, h) = (icon[0].len() as i32 * ip, icon.len() as i32 * ip);
        sprite(dc, icon, x + u(21) - w / 2, y + (th - h) / 2, ip, &|c| (c == b'X').then_some(accent));
        SetTextColor(dc, CREAM);
        let mut lr = RECT { left: x + u(40), top: y + u(7), right: x + tw - u(3), bottom: y + u(25) };
        text_at(dc, label, &mut lr, DT_SINGLELINE | DT_VCENTER);
        SetTextColor(dc, theme().dim);
        let mut sr = RECT { left: x + u(40), top: y + u(25), right: x + tw - u(3), bottom: y + u(43) };
        text_at(dc, &sub, &mut sr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
        mrects.push((rc(x, y, tw, th), act));
    }
    top + rows * th + (rows - 1) * gap
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
        let (tabs, tab, mine) = picker(ms.pet_page);
        let extra = Extra { guide: ms.guide, now: ms.now.is_some(), pets: tabs[tab].pets.len(), tabs: mine.len(), packs: !ms.packs.is_empty(), friend: ms.picking_friend, lines: ms.lines.len() };
        let rows_h = page_height(ms.page, extra, &u);
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
        let title = match ms.page {
            0 => "PET HUB",
            PAGE_PICK => if ms.picking_friend { "CHOOSE A FRIEND" } else { "CHOOSE A CHARACTER" },
            PAGE_OPTIONS => "SETTINGS",
            PAGE_POSITION => "POSITION",
            PAGE_FEED => if machine { "FILL UP" } else { "FEED" },
            PAGE_WARDROBE => "WARDROBE",
            PAGE_LAUNCH => "QUICK LAUNCH",
            PAGE_KEYS => "SHORTCUT KEYS",
            PAGE_PACKS => "CHARACTER PACKS",
            PAGE_BREAKS => "BREAKS",
            PAGE_PET => "PET",
            PAGE_LOOKS => "LOOKS",
            PAGE_WORK => "WORK",
            PAGE_TOOLS => "TOOLS",
            p => group_rows(p).map_or("", |g| g.0),
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
                // how it is doing; a click leads to its food
                let card = rc(ix, top, iw, u(CARD_H));
                fill(dc, card.left, card.top, iw, u(CARD_H), theme().panel);
                mrects.push((card, MAct::FeedPage));
                SetTextColor(dc, CREAM);
                let mut nr = rc(ix + u(8), top + u(3), iw - u(16), u(18));
                text_at(dc, &pet.name.to_uppercase(), &mut nr, DT_SINGLELINE | DT_VCENTER);
                SetTextColor(dc, theme().accent);
                let mut lr = rc(ix + u(8), top + u(3), iw - u(16), u(18));
                text_at(dc, &format!("LEVEL {}", wardrobe::level(ms.xp)), &mut lr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
                for (i, (name, v, c)) in [(if machine { "TANK" } else { "FOOD" }, ms.food, GREEN), ("ENERGY", ms.energy, UP_BLUE), ("HAPPY", ms.happy, PINK)].into_iter().enumerate() {
                    let y = top + u(22) + i as i32 * u(14);
                    SetTextColor(dc, theme().hint);
                    let mut lr = rc(ix + u(8), y, u(60), u(14));
                    text_at(dc, name, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    let (bx, bw) = (ix + u(66), iw - u(74));
                    fill(dc, bx, y + u(4), bw, u(7), theme().panel_light);
                    fill(dc, bx, y + u(4), (bw as f32 * v.clamp(0.0, 100.0) / 100.0) as i32, u(7), if v < 25.0 { RED } else { c });
                }
                let top = draw_tiles(dc, &mut mrects, tiles_of(0, ms), ix, top + u(CARD_H) + gap, iw, &u) + gap;
                let third = (iw - 2 * gap) / 3;
                for (k, (label, bg, act)) in [(if ms.minimal { "MINIMAL ON" } else { "MINIMAL" }, if ms.minimal { BTN_GREEN } else { theme().panel_light }, MAct::Minimal), ("HIDE", theme().panel_light, MAct::Hide), ("QUIT", BTN_RED, MAct::Quit)].into_iter().enumerate() {
                    let r = rc(ix + k as i32 * (third + gap), top, third, u(FOOT_H));
                    pixel_btn(dc, r, label, bg);
                    mrects.push((r, act));
                }
                if ms.guide {
                    let gy = top + u(FOOT_H) + gap;
                    fill(dc, ix, gy, iw, u(62), theme().panel);
                    SetTextColor(dc, theme().hint);
                    let mut gr = RECT { left: ix + u(6), top: gy + u(3), right: ix + iw - u(6), bottom: gy + u(59) };
                    text_at(dc, "Pet: feed and play. Looks: characters, hats, theme. Work: tasks and focus. Show: what I display. Settings: how I behave.", &mut gr, DT_WORDBREAK);
                }
            }
            PAGE_PET | PAGE_LOOKS | PAGE_WORK | PAGE_TOOLS => {
                draw_tiles(dc, &mut mrects, tiles_of(ms.page, ms), ix, top, iw, &u);
            }
            PAGE_BREAKS => draw_lines(dc, &mut mrects, &ms.lines, &ms.hint, ix, top, iw, &u),
            PAGE_PICK => {
                let chosen = if ms.picking_friend { ms.friend.checked_sub(1) } else { Some(ms.pet) };
                let small = CreateFontW(-u(11), 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 3, 49, wide("Consolas").as_ptr());
                let old = SelectObject(dc, small);
                let mut top = top;
                if !ms.packs.is_empty() {
                    // from pack to pack
                    let bw = u(30);
                    let (l, r) = (rc(ix, top, bw, u(TAB_H)), rc(ix + iw - bw, top, bw, u(TAB_H)));
                    pixel_btn(dc, l, "<", theme().panel_light);
                    pixel_btn(dc, r, ">", theme().panel_light);
                    let before = (mine.start + tabs.len() - 1) % tabs.len();
                    mrects.push((l, MAct::PetPage(tabs.iter().position(|t| t.pack == tabs[before].pack).unwrap_or(0))));
                    mrects.push((r, MAct::PetPage(mine.end % tabs.len())));
                    SetTextColor(dc, theme().accent);
                    let mut nr = rc(ix + bw, top, iw - 2 * bw, u(TAB_H));
                    text_at(dc, &pack::name_of(tabs[tab].pack).to_uppercase(), &mut nr, DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_END_ELLIPSIS);
                    top += u(TAB_H) + u(4);
                }
                // the tabs; the one with the chosen pet is marked
                let bw = (iw - (TABS - 1) * u(4)) / TABS;
                for (n, t) in mine.clone().enumerate() {
                    let r = rc(ix + (n as i32 % TABS) * (bw + u(4)), top + (n as i32 / TABS) * (u(TAB_H) + u(4)), bw, u(TAB_H));
                    pixel_btn(dc, r, tabs[t].name, if t == tab { BTN_GREEN } else { theme().panel_light });
                    if chosen.is_some_and(|c| tabs[t].pets.contains(&c)) {
                        fill(dc, r.right - u(7), r.top + u(3), u(4), u(4), theme().accent);
                    }
                    mrects.push((r, MAct::PetPage(t)));
                }
                let tab_rows = (mine.len() as i32 + TABS - 1) / TABS;
                let top = top + tab_rows * (u(TAB_H) + u(4)) + gap;
                let tw = (iw - (PET_COLS - 1) * gap) / PET_COLS;
                let th = u(PET_TILE_H);
                for (n, &i) in tabs[tab].pets.iter().enumerate() {
                    let p = &PETS[i];
                    let (cx0, cy0) = (ix + (n as i32 % PET_COLS) * (tw + gap), top + (n as i32 / PET_COLS) * (th + gap));
                    let r = rc(cx0, cy0, tw, th);
                    tile(dc, r, theme().panel_edge, chosen == Some(i));
                    GdiFlush();
                    let cell = 2.4 * scale;
                    let feet = ((cx0 + tw / 2) as f32, (cy0 + th - u(18)) as f32);
                    blit(art, &compose(p, &Rig::REST), p, &Rig::REST, feet, cell, 1);
                    SetTextColor(dc, if chosen == Some(i) { GREEN } else { CREAM });
                    let mut nr = rc(cx0, cy0 + th - u(18), tw, u(16));
                    text_at(dc, &p.name.to_uppercase(), &mut nr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
                    mrects.push((r, MAct::SetPet(i)));
                }
                SelectObject(dc, old);
                DeleteObject(small);
                if ms.picking_friend {
                    let rows = (tabs[tab].pets.len() as i32 + PET_COLS - 1) / PET_COLS;
                    let r = rc(ix, top + rows * (th + gap), iw, u(FOOT_H));
                    pixel_btn(dc, r, "NO FRIEND", if ms.friend == 0 { BTN_GREEN } else { theme().panel_light });
                    mrects.push((r, MAct::NoFriend));
                }
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
                text_at(dc, &format!("{} SERVES THE FAVOURITE", keys::label_of(&ms.keys, 7)), &mut hr, DT_SINGLELINE | DT_VCENTER | DT_CENTER);
            }
            PAGE_KEYS => {
                let rh = u(OPT_ROW);
                for (i, a) in keys::ACTIONS.iter().enumerate() {
                    let r = rc(ix, top + i as i32 * rh, iw, rh);
                    if i > 0 {
                        fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    }
                    SetTextColor(dc, CREAM);
                    let mut lr = RECT { left: ix + u(8), ..r };
                    text_at(dc, a.name, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    let waiting = ms.key_wait == Some(i);
                    let fixed = a.id == keys::CLOSE && ms.more[4];
                    SetTextColor(dc, if waiting { ALERT_ORANGE } else { UP_BLUE });
                    let mut vr = RECT { right: r.right - u(32), ..r };
                    let shown = if waiting { keys::named("CTL+ALT+...") } else if fixed { keys::named("CTL+ALT+Q") } else { keys::label(ms.keys[i]) };
                    text_at(dc, &shown, &mut vr, DT_SINGLELINE | DT_VCENTER | DT_RIGHT);
                    if !fixed {
                        let x = rc(r.right - u(26), r.top + u(4), u(22), rh - u(8));
                        pixel_btn(dc, x, "x", theme().panel_light);
                        mrects.push((x, MAct::NoKey(i)));
                        mrects.push((r, MAct::PickKey(i)));
                    }
                }
                SetTextColor(dc, theme().hint);
                let mut hr = rc(ix + u(4), top + keys::ACTIONS.len() as i32 * rh + u(2), iw - u(8), u(32));
                text_at(dc, &keys::named("Click a row, then hold Ctrl+Alt and press a letter or digit."), &mut hr, DT_WORDBREAK);
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
            PAGE_PACKS => {
                let rh = u(OPT_ROW);
                for i in 0..pack::MOST {
                    let r = rc(ix, top + i as i32 * rh, iw, rh);
                    if i > 0 {
                        fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    }
                    let mut lr = RECT { left: ix + u(8), right: r.right - u(34), ..r };
                    let Some((n, name, count)) = ms.packs.get(i) else {
                        SetTextColor(dc, theme().dim);
                        text_at(dc, "-", &mut lr, DT_SINGLELINE | DT_VCENTER);
                        continue;
                    };
                    let sure = ms.pack_sure == Some(*n);
                    SetTextColor(dc, if sure { RED } else { CREAM });
                    let text = if sure { "CLICK x AGAIN TO REMOVE IT".to_string() } else { format!("{name} ({count})") };
                    text_at(dc, &text, &mut lr, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
                    let x = rc(r.right - u(26), r.top + u(4), u(22), rh - u(8));
                    pixel_btn(dc, x, "x", if sure { BTN_RED } else { theme().panel_light });
                    mrects.push((x, MAct::PackForget(*n))); // before the row, so it wins
                    if let Some(t) = tabs.iter().position(|t| t.pack == *n) {
                        mrects.push((r, MAct::PetTab(t)));
                    }
                }
                let y = top + pack::MOST as i32 * rh;
                for (k, (label, act)) in [("+ IMPORT A PACK", MAct::PackAdd), ("OPEN THE PACKS FOLDER", MAct::PackFolder)].into_iter().enumerate() {
                    let r = rc(ix, y + k as i32 * rh, iw, rh);
                    fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    SetTextColor(dc, if k == 0 { GREEN } else { CREAM });
                    let mut lr = RECT { left: ix + u(8), ..r };
                    text_at(dc, label, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    mrects.push((r, act));
                }
                SetTextColor(dc, if ms.pack_note.is_empty() { theme().hint } else { RED });
                let mut hr = rc(ix + u(4), y + 2 * rh + u(2), iw - u(8), u(32));
                let hint = if cfg!(target_os = "macos") { "A pack is a .petpack file with new characters." } else { "Or drop a .petpack file on the pet." };
                text_at(dc, if ms.pack_note.is_empty() { hint } else { &ms.pack_note }, &mut hr, DT_WORDBREAK);
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
                for (n, g) in (GROUP_LISTED as usize..OPTION_GROUPS.len()).enumerate() {
                    let (name, about, _) = OPTION_GROUPS[g];
                    let r = rc(ix, top + n as i32 * rh, iw, rh);
                    if n > 0 {
                        fill(dc, ix, r.top, iw, 1, theme().panel_light);
                    }
                    SetTextColor(dc, CREAM);
                    let mut lr = rc(ix + u(8), r.top + u(3), iw - u(30), u(18));
                    text_at(dc, name, &mut lr, DT_SINGLELINE | DT_VCENTER);
                    SetTextColor(dc, theme().dim);
                    let mut ar = rc(ix + u(8), r.top + u(20), iw - u(30), u(18));
                    text_at(dc, about, &mut ar, DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
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
                    (if cfg!(windows) { "START WITH WINDOWS" } else { "START AT LOGIN" }, Some(ms.flags[9]), String::new()),
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
                    ("SHORTCUT KEYS", None, ">".to_string()),
                ];
                let rh = u(OPT_ROW);
                for (n, i) in group_rows(ms.page).map_or(&[][..], |(_, _, rows)| rows).iter().enumerate() {
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
        for (name, _, rows) in OPTION_GROUPS {
            assert!(!name.is_empty() && !rows.is_empty());
            for i in *rows {
                seen[*i] += 1;
            }
        }
        // dancing, the key that starts the pet, and what plays
        let elsewhere: &[usize] = if cfg!(windows) { &[] } else { &[26, 30, 36] };
        for (i, n) in seen.iter().enumerate() {
            assert_eq!(*n, !(RETIRED.contains(&i) || elsewhere.contains(&i)) as u32, "setting {i}");
        }
    }

    #[test]
    fn the_hub_window_is_tall_enough_for_every_page_and_the_biggest_pet() {
        let rows = PETS.iter().map(|p| p.h()).max().unwrap();
        for scale in [0.75f32, 1.0, 1.25, 1.5, 2.0] {
            let u = move |v: i32| (v as f32 * scale).round() as i32;
            for page in 0..PAGE_GROUP + OPTION_GROUPS.len() as u8 {
                // the fullest tab of the fullest pack
                for (pets, tabs) in [(1, 1), (pack::PER_TAB, pack::MOST_TABS)] {
                    let panel = 2 * (u(3) + u(8)) + u(24) + u(6) + page_height(page, Extra { guide: true, now: true, pets, tabs, packs: true, friend: true, lines: MOST_LINES }, &u);
                    let pet = rows * u(PX) + u(12) + u(crate::body::PAD_TOP.max(crate::body::PAD_BOTTOM));
                    assert!(panel + pet <= u(MENU_H), "page {page} too tall at {scale}x: {} vs {}", panel + pet, u(MENU_H));
                }
            }
        }
    }

    fn view() -> HubView {
        unsafe { hub_view() }
    }

    #[test]
    fn groups_are_where_the_categories_say() {
        for (g, name) in [(GROUP_SHOW, "SHOW"), (GROUP_LOOK, "THEME AND SIZE"), (GROUP_FOCUS, "TIMER AND BREAKS"), (GROUP_KEYS, "KEYS AND STARTUP")] {
            assert_eq!(OPTION_GROUPS[g as usize].0, name);
        }
        for (i, (name, about, _)) in OPTION_GROUPS.iter().enumerate() {
            assert_eq!(about.is_empty(), i < GROUP_LISTED as usize, "{name}: the listed ones say what is in them");
        }
    }

    #[test]
    fn no_page_is_a_wall_of_tiles_and_every_page_can_be_reached() {
        let v = view();
        let mut reached = vec![0u8];
        for page in [0, PAGE_PET, PAGE_LOOKS, PAGE_WORK, PAGE_TOOLS] {
            let tiles = tiles_of(page, &v);
            assert!((4..=6).contains(&tiles.len()), "page {page} has {} tiles", tiles.len());
            for (label, _, _, act, sub) in &tiles {
                assert!(label.len() <= 11 && sub.len() <= 16, "{label} / {sub} does not fit");
                match act {
                    MAct::Page(p) => reached.push(*p),
                    MAct::Group(g) => reached.push(PAGE_GROUP + g),
                    MAct::Options => reached.push(PAGE_OPTIONS),
                    MAct::Pets | MAct::PickFriend => reached.push(PAGE_PICK),
                    MAct::FeedPage => reached.push(PAGE_FEED),
                    MAct::Wardrobe => reached.push(PAGE_WARDROBE),
                    MAct::LaunchPage => reached.push(PAGE_LAUNCH),
                    MAct::Packs => reached.push(PAGE_PACKS),
                    MAct::Breaks => reached.push(PAGE_BREAKS),
                    _ => {}
                }
            }
        }
        // the settings list, and the two settings that open a page
        reached.extend((GROUP_LISTED..OPTION_GROUPS.len() as u8).map(|g| PAGE_GROUP + g));
        reached.extend([PAGE_POSITION, PAGE_KEYS]);
        for p in 0..PAGE_GROUP + OPTION_GROUPS.len() as u8 {
            assert!(reached.contains(&p), "nothing leads to page {p}");
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
