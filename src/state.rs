//! Shared state: the `State` struct behind the global lock, window handles, sizes, pomodoro maths.

use crate::prelude::*;

// ---- layout (logical px, scaled by DPI) ----
pub(crate) const PANEL_W: i32 = 340; // bubbles, cards and the hub
pub(crate) const WIN_W: i32 = PANEL_W + body::PAD_X;
pub(crate) const WIN_H: i32 = 290;
pub(crate) const FOCUS_H: i32 = 500; // taller window in focus mode (timer + tasks)
pub(crate) const MENU_H: i32 = 558; // taller window while the pixel hub menu is open
pub(crate) const PX: i32 = 6; // one sprite pixel
pub(crate) const TICK_MS: u32 = 500;
pub(crate) const FACT_EVERY: u32 = 120; // ticks (60 s)
pub(crate) const FACT_FOR: u32 = 18; // ticks (9 s)

// ======================================================================
// state
// ======================================================================

pub(crate) const SIZES: [f32; 3] = [0.75, 1.0, 1.4]; // small, medium, large

pub(crate) struct State {
    pub scale: f32, // DPI scale of the monitor the pet is on
    pub stats_scale: f32, // DPI scale of the stats window
    pub max_scale: f32, // largest scale at which the tallest window fits the screen
    pub size: usize,
    pub tick: u32,
    pub last_in: u32,
    pub last_out: u32,
    pub speed: f64, // download, bytes per second
    pub up: f64,    // upload, bytes per second
    pub fact: usize,
    pub fact_left: u32,
    // pet inner life
    pub mood: Mood,
    pub stats: Stats,
    pub intro: u8,        // first-run tour step (intro::DONE when finished)
    pub intro_wait: u32,  // ticks before the next step
    pub stats_range: usize, // stats window chart: 0 = 7 days, 1 = 14, 2 = 30
    pub coach: Coach,
    pub watch: Watch,
    pub care_on: bool,
    pub dnd_on: bool,     // hide + stay quiet while a fullscreen app/game is in front
    pub quiet: bool,      // currently in that state
    pub sys_on: bool,
    pub cpu_prev: (u64, u64), // (idle, total) cpu time
    pub cpu: u32,
    pub ram: u32,
    pub msg: String, // pet's own speech bubble
    pub msg_left: u32,
    pub say_n: u32,
    pub theme: usize,        // index into THEMES
    pub opacity: usize,      // index into OPACITIES
    pub autostart: bool,     // start with Windows (mirrors the registry entry)
    pub cache_on: bool,      // count cache reads in the AI token totals
    pub tok_day: u32,        // day the token milestone below belongs to
    pub tok_mile: u32,       // highest token milestone already cheered today
    pub tok_seen: bool,      // first reading taken (so old usage is not cheered at startup)
    pub tokens_on: bool,     // show AI coding token usage (read from local Claude Code logs)
    pub walk_on: bool,       // walk around the screen by itself
    pub tricks_on: bool,     // do random little acts by itself
    pub gravity: bool,       // falls when dropped, can be thrown
    pub click_hits: bool,    // a left click hits the pet (otherwise it opens the hub)
    pub visit_on: bool,      // goes to the other monitors
    pub dance_on: bool,      // dances to music
    pub talk_on: bool,       // says things by itself
    pub hide_for: Vec<String>, // programs to hide from
    pub last_app: String,    // program in front (not us)
    pub shortcut: bool,      // Ctrl+Alt+Q opens it too
    pub nap_after: usize,    // index into NAP_AFTER
    pub body: Body,
    pub flip: (bool, bool),  // the pet sits on the left / at the top of its window
    pub placed: bool,        // flip has been worked out
    pub stroke: i32, // accumulated mouse travel over the pet (petting)
    pub last_mouse: Option<(i32, i32)>,
    pub pet: usize,
    pub facts_on: bool,
    pub show_speed: bool,    // show the download speed in the info bubble
    pub show_up: bool,
    pub bits: bool,
    pub show_pc: bool,       // show CPU and memory load in the info bubble
    // presence: how much of the screen the pet takes up
    pub minimal: bool,       // small and quiet; details only on hover
    pub hidden: bool,        // tucked away in the tray (not saved)
    pub on_top: bool,        // stay above other windows
    pub through: bool,       // mouse clicks fall through the pet
    pub hover: bool,         // minimal mode: the mouse is over the pet
    pub hover_rect: RECT,    // what counts as "over the pet" (client coordinates, recorded by paint)
    pub pos: Option<(i32, i32)>,
    // focus mode + pomodoro
    pub focus: bool,
    pub pomo_preset: usize,
    pub pomo_phase: u8, // 0 focus, 1 short break, 2 long break
    pub pomo_running: bool,
    pub pomo_end: Option<Instant>,
    pub pomo_left: u64, // seconds left while paused (u64::MAX = full length)
    pub pomo_done: u32,
    pub frects: Vec<(RECT, FAct)>,
    // todo list
    pub todos: Vec<Todo>,
    pub next_id: u32,
    pub alert: Option<u32>,
    pub view: Vec<u32>, // todo ids in list order
    pub todo_sel: Option<u32>,
    pub todo_scroll: usize,
    pub todo_prev: String,
    // click targets recorded by paint (client coordinates)
    pub btns: Vec<(RECT, u8)>,
    pub pet_rect: RECT,
    pub fact_rect: Option<RECT>,
    // pixel hub menu (left click)
    pub menu: bool,
    pub page: u8, // 0 hub, 1 pets, 2 options, 3 position, 4.. one page per options group
    pub menu_ticks: u32,
    pub mrects: Vec<(RECT, MAct)>,
    // mouse
    pub down: bool,
    pub dragging: bool,
    pub cur0: (i32, i32),
    pub win0: (i32, i32),
    // data usage (bytes)
    pub day_key: u32,
    pub day_rx: u64,
    pub day_tx: u64,
    pub mon_key: u32,
    pub mon_rx: u64,
    pub mon_tx: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum MAct {
    Close,
    Back,
    Tasks,
    Focus,
    Stats,
    Pets,
    Options,
    Group(u8),
    Minimal,
    Nap,
    FeedPage,
    Feed(usize),
    Trick,
    Calc,
    Notes,
    Code,
    Hide,
    Quit,
    SetPet(usize),
    Opt(u8),
    Corner(u8),
    NextMonitor,
}

#[derive(Clone, Copy)]
pub(crate) enum FAct {
    Toggle,
    Reset,
    Skip,
    Exit,
    Task(u32),
}

/// (focus, short break, long break) minutes
pub(crate) const POMO: [(u64, u64, u64); 3] = [(25, 5, 15), (50, 10, 20), (15, 3, 10)];
pub(crate) const POMO_NAMES: [&str; 3] = ["25 / 5 min", "50 / 10 min", "15 / 3 min"];

pub(crate) fn pomo_len(s: &State) -> u64 {
    let p = POMO[s.pomo_preset];
    60 * match s.pomo_phase {
        0 => p.0,
        1 => p.1,
        _ => p.2,
    }
}

pub(crate) fn pomo_remaining(s: &State) -> u64 {
    if s.pomo_running {
        s.pomo_end.map_or(0, |e| e.saturating_duration_since(Instant::now()).as_secs_f64().ceil() as u64)
    } else if s.pomo_left == u64::MAX {
        pomo_len(s)
    } else {
        s.pomo_left
    }
}

pub(crate) fn pomo_reset(s: &mut State) {
    s.pomo_running = false;
    s.pomo_end = None;
    s.pomo_left = u64::MAX;
}

pub(crate) fn pomo_toggle(s: &mut State) {
    if s.pomo_running {
        s.pomo_left = pomo_remaining(s);
        s.pomo_running = false;
        s.pomo_end = None;
    } else {
        let r = pomo_remaining(s);
        s.pomo_end = Some(Instant::now() + Duration::from_secs(r));
        s.pomo_running = true;
    }
}

/// Move to the next phase (paused, waiting for START). Only a finished focus round counts.
pub(crate) fn pomo_advance(s: &mut State, finished: bool) {
    if s.pomo_phase == 0 {
        if finished {
            s.pomo_done += 1;
        }
        s.pomo_phase = if finished && s.pomo_done % 4 == 0 { 2 } else { 1 };
    } else {
        s.pomo_phase = 0;
    }
    pomo_reset(s);
}

pub(crate) const NO_RECT: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };

pub(crate) static ST: Mutex<State> = Mutex::new(State {
    scale: 1.0,
    stats_scale: 1.0,
    max_scale: 100.0,
    size: 1,
    tick: 0,
    last_in: 0,
    last_out: 0,
    speed: 0.0,
    up: 0.0,
    fact: 0,
    fact_left: 0,
    mood: Mood::new(),
    stats: Stats::new(),
    intro: 0,
    intro_wait: 6,
    stats_range: 0,
    coach: Coach::new(),
    watch: Watch::new(),
    care_on: true,
    dnd_on: true,
    quiet: false,
    sys_on: true,
    cpu_prev: (0, 0),
    cpu: 0,
    ram: 0,
    msg: String::new(),
    msg_left: 0,
    say_n: 0,
    theme: 0,
    opacity: 0,
    autostart: false,
    cache_on: false,
    tok_day: 0,
    tok_mile: 0,
    tok_seen: false,
    tokens_on: true,
    walk_on: true,
    tricks_on: true,
    gravity: true,
    click_hits: true,
    visit_on: true,
    dance_on: true,
    talk_on: true,
    hide_for: Vec::new(),
    last_app: String::new(),
    shortcut: false,
    nap_after: 1,
    body: Body::new(),
    flip: (false, false),
    placed: false,
    stroke: 0,
    last_mouse: None,
    pet: 0,
    facts_on: true,
    show_speed: true,
    show_up: true,
    bits: false,
    show_pc: false,
    minimal: false,
    hidden: false,
    on_top: true,
    through: false,
    hover: false,
    hover_rect: NO_RECT,
    pos: None,
    focus: false,
    pomo_preset: 0,
    pomo_phase: 0,
    pomo_running: false,
    pomo_end: None,
    pomo_left: u64::MAX,
    pomo_done: 0,
    frects: Vec::new(),
    todos: Vec::new(),
    next_id: 1,
    alert: None,
    view: Vec::new(),
    todo_sel: None,
    todo_scroll: 0,
    todo_prev: String::new(),
    btns: Vec::new(),
    pet_rect: NO_RECT,
    fact_rect: None,
    menu: false,
    page: 0,
    menu_ticks: 0,
    mrects: Vec::new(),
    down: false,
    dragging: false,
    cur0: (0, 0),
    win0: (0, 0),
    day_key: 0,
    day_rx: 0,
    day_tx: 0,
    mon_key: 0,
    mon_rx: 0,
    mon_tx: 0,
});

pub(crate) static PET_WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static TODO_WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static STATS_WND: AtomicIsize = AtomicIsize::new(0);
pub(crate) static STATS_HOVER: AtomicI32 = AtomicI32::new(-1);
pub(crate) static TODO_EDIT: AtomicIsize = AtomicIsize::new(0);

pub(crate) fn hw(a: &AtomicIsize) -> HWND {
    a.load(Relaxed) as HWND
}

pub(crate) fn eff_scale(s: &State) -> f32 {
    (s.scale * SIZES[s.size]).min(s.max_scale)
}

pub(crate) fn win_dims(scale: f32, focus: bool, menu: bool) -> (i32, i32) {
    let h = if focus { FOCUS_H } else if menu { MENU_H } else { WIN_H };
    ((WIN_W as f32 * scale).round() as i32, (h as f32 * scale).round() as i32)
}

pub(crate) fn win_dims_s(s: &State) -> (i32, i32) {
    win_dims(eff_scale(s), s.focus, s.menu)
}

pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub(crate) fn contains(r: &RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}
