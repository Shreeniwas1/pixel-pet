//! The pet window: clicks, hub actions and the window procedure.

use crate::prelude::*;

// ======================================================================
// pet window: mouse, menu, messages
// ======================================================================

pub(crate) unsafe fn handle_click(hwnd: HWND, x: i32, y: i32) {
    enum Act {
        Focus(FAct),
        Alert(u8),
        Menu(MAct),
        Pet,
        Fact,
        None,
    }
    let act = {
        let s = ST.lock().unwrap();
        if let Some((_, a)) = s.frects.iter().find(|(r, _)| contains(r, x, y)) {
            Act::Focus(*a)
        } else if let Some((_, a)) = s.btns.iter().find(|(r, _)| contains(r, x, y)) {
            Act::Alert(*a)
        } else if let Some((_, a)) = s.mrects.iter().find(|(r, _)| contains(r, x, y)).filter(|_| s.menu) {
            Act::Menu(*a)
        } else if contains(&s.pet_rect, x, y) {
            Act::Pet
        } else if s.fact_rect.map_or(false, |r| contains(&r, x, y)) {
            Act::Fact
        } else {
            Act::None
        }
    };
    match act {
        Act::Focus(a) => match a {
            FAct::Toggle => pomo_toggle(&mut ST.lock().unwrap()),
            FAct::Reset => {
                let mut s = ST.lock().unwrap();
                gave_up(&mut s);
                pomo_reset(&mut s);
            }
            FAct::Skip => {
                let mut s = ST.lock().unwrap();
                gave_up(&mut s);
                pomo_advance(&mut s, false);
            }
            FAct::Exit => reanchor(hwnd, |s| s.focus = false),
            FAct::Task(id) => {
                {
                    let mut s = ST.lock().unwrap();
                    let (now, ..) = local_now();
                    todo::finish(&mut s.todos, id, now);
                    if s.alert == Some(id) {
                        s.alert = None;
                    }
                    let ev = s.mood.on_task_done();
                    body::gain(&mut s, wardrobe::XP_TASK);
                    say(&mut s, ev);
                    save_todos(&s);
                }
                refresh_list();
            }
        },
        Act::Alert(a) => {
            let (now, ..) = local_now();
            {
                let mut s = ST.lock().unwrap();
                if s.alert.is_none() && s.question {
                    s.question = false;
                    match a {
                        0 => s.updates = update::YES,
                        1 => s.updates = update::NO,
                        _ => s.tick = 1, // asked again a while later
                    }
                    if a < 2 {
                        say_text(&mut s, if a == 0 { "Thanks! I will only tell you, never install anything." } else { "Fine. You can change that in the settings." }, 10);
                    }
                    save_cfg(&s);
                } else if let Some(id) = s.alert.take() {
                    match a {
                        0 => {
                            if todo::finish(&mut s.todos, id, now) {
                                let ev = s.mood.on_task_done();
                                body::gain(&mut s, wardrobe::XP_TASK);
                                say(&mut s, ev);
                            }
                        }
                        1 => {
                            if let Some(t) = s.todos.iter_mut().find(|t| t.id == id) {
                                t.due = Some(now + 10);
                                t.fired = false;
                            }
                        }
                        _ => {} // later: stays in the list, marked overdue
                    }
                    save_todos(&s);
                }
            }
            refresh_list();
        }
        Act::Menu(a) => menu_action(hwnd, a),
        Act::Pet => {
            let hits = {
                let s = ST.lock().unwrap();
                s.click_hits && !s.menu
            };
            if hits {
                let mut wr: RECT = zeroed();
                GetWindowRect(hwnd, &mut wr);
                let mut s = ST.lock().unwrap();
                let mid = (s.pet_rect.left + s.pet_rect.right) / 2;
                let from = if x < mid { -1.0 } else { 1.0 };
                body::hit(&mut s, from, ((wr.left + x) as f32, (wr.top + y) as f32));
            } else {
                ask_for_hub(hwnd);
            }
        }
        Act::Fact => {
            let mut s = ST.lock().unwrap();
            s.fact_left = 0;
            s.msg_left = 0;
        }
        Act::None => {}
    }
    InvalidateRect(hwnd, null(), 0);
}

/// Give the pet something; `None` is its favourite.
pub(crate) unsafe fn feed_pet(hwnd: HWND, food: Option<usize>) {
    let mut wr: RECT = zeroed();
    GetWindowRect(hwnd, &mut wr);
    let mut s = ST.lock().unwrap();
    let food = food.unwrap_or(PETS[s.pet].foods[0]);
    let (fx, fy) = geo(&s).feet();
    let feet = (wr.left + fx, wr.top + fy);
    let floor = if s.gravity { work_at(feet.0, feet.1 - 10).bottom } else { feet.1 };
    body::feed(&mut s, food, (feet.0 as f32, feet.1 as f32), floor as f32);
}

/// The second pet has no hub of its own; it asks the first to open the page that changes it.
unsafe fn ask_for_hub(hwnd: HWND) {
    match friend::other().filter(|_| friend::is_friend()) {
        Some(w) => {
            PostMessageW(w, friend::WM_CHOOSE, 0, 0);
        }
        None => toggle_hub(hwnd),
    }
}

/// A pet that just stands there does not need thirty looks a second.
unsafe fn pace(hwnd: HWND, lively: bool) {
    static FAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
    if FAST.swap(lively, Relaxed) != lively {
        SetTimer(hwnd, 2, if lively { body::FRAME_MS } else { body::SLOW_MS }, None);
    }
}

/// While a new shortcut key is being chosen: was one pressed, or was the page left?
unsafe fn catch_key(hwnd: HWND) {
    let mut s = ST.lock().unwrap();
    let Some(i) = s.key_wait else { return };
    let key = keys::pressed();
    if let Some(k) = key {
        keys::choose(&mut s.keys, i, k);
        sound::play(Sound::Happy);
        save_cfg(&s);
    } else if s.menu && s.page == PAGE_KEYS {
        s.menu_ticks = 0; // the hub waits too
        return;
    }
    s.key_wait = None;
    keys::register(hwnd, &s.keys, s.shortcut);
    drop(s);
    InvalidateRect(hwnd, null(), 0);
}

/// Put something on the quick launch list. True if it is new.
fn add_launch(target: &str) -> bool {
    let mut s = ST.lock().unwrap();
    let new = launch::add(&mut s.launch, target);
    if new {
        launch::save(&s.launch);
        sound::play(Sound::Happy);
    }
    new
}

unsafe fn open_code(hwnd: HWND) {
    if !tools::open_code() {
        say_text(&mut ST.lock().unwrap(), "I can't find VS Code on this PC. Is it installed?", 16);
        InvalidateRect(hwnd, null(), 0);
    }
}

pub(crate) fn close_menu(s: &mut State) {
    s.menu = false;
    s.page = 0;
}

/// Packs came or went: nobody who is no longer there is shown.
pub(crate) unsafe fn settle(hwnd: HWND) {
    let (pet, friend) = {
        let s = ST.lock().unwrap();
        (s.pet, s.friend)
    };
    let friend_now = if friend == 0 || pack::still(friend - 1) == friend - 1 { friend } else { pack::find(&pack::key_of(friend - 1)).map_or(0, |i| i + 1) };
    if pack::still(pet) != pet {
        reanchor(hwnd, |s| {
            s.body.stop();
            s.body.meal = None;
            s.pet = pack::still(pet);
        });
    }
    if friend_now != friend {
        let mut s = ST.lock().unwrap();
        s.friend = friend_now;
        save_cfg(&s);
        drop(s);
        friend::reload();
    }
}

/// Take in a pack of characters, and show them.
pub(crate) unsafe fn import_pack(hwnd: HWND, path: &std::path::Path) {
    imported(hwnd, pack::import(path));
}

unsafe fn imported(hwnd: HWND, how: Result<pack::Added, String>) {
    match how {
        Ok(a) => {
            settle(hwnd);
            sound::play(Sound::Level);
            reanchor(hwnd, |s| {
                say_text(s, &format!("{}: {} new character{}!", a.name, a.count, if a.count == 1 { "" } else { "s" }), 12);
                s.menu = true;
                s.focus = false;
                s.menu_ticks = 0;
                s.page = PAGE_PICK;
                s.picking_friend = false;
                s.pet_page = pack::tab_of(a.first);
            });
        }
        Err(e) => say_text(&mut ST.lock().unwrap(), &e, 18),
    }
    InvalidateRect(hwnd, null(), 0);
}

/// A focus round that had begun is stopped before its end: the stats count it.
unsafe fn gave_up(s: &mut State) {
    let spent = pomo_len(s).saturating_sub(pomo_remaining(s));
    if s.pomo_phase == 0 && spent >= 60 {
        s.stats.stop((local_now().0 / 1440) as u32);
        save_stats(s);
    }
}

/// Water the garden, and say how that went.
fn water(s: &mut State, day: u32) {
    let w = s.garden.water(day);
    let text = match w.bloomed {
        Some(seed) => format!("The {} is in flower! Click it to pick it.", garden::SEEDS[seed as usize].name.to_lowercase()),
        None if w.grew > 0 => "Glug glug. They are growing!".to_string(),
        None if w.revived > 0 => "Phew, just in time. They will grow again tomorrow.".to_string(),
        None if w.had > 0 => "They have had water today. Come back tomorrow!".to_string(),
        None => "Plant a seed first: click an empty pot.".to_string(),
    };
    if w.grew + w.revived > 0 {
        body::gain(s, 1);
    }
    say_text(s, &text, 10);
}

/// Something in the pixel hub was clicked.
pub(crate) unsafe fn menu_action(hwnd: HWND, a: MAct) {
    ST.lock().unwrap().menu_ticks = 0;
    sound::play(Sound::Click);
    match a {
        MAct::Close => reanchor(hwnd, close_menu),
        MAct::Back => {
            let mut s = ST.lock().unwrap();
            s.page = parent_page(s.page);
        }
        MAct::Pets => {
            let mut s = ST.lock().unwrap();
            s.page = PAGE_PICK;
            s.picking_friend = false;
            s.pet_page = pack::tab_of(s.pet);
        }
        MAct::PetPage(p) => ST.lock().unwrap().pet_page = p,
        MAct::Builds => ST.lock().unwrap().page = PAGE_BUILDS,
        MAct::BuildAdd => {
            let text = match launch::copied_text().as_deref().and_then(builds::repo_of) {
                None => "Copy the link of a repository on GitHub first.".to_string(),
                Some(r) => {
                    let name = r.clone();
                    if builds::add(r) { format!("I watch the builds of {name} now.") } else { "I watch that one already, or have too many.".to_string() }
                }
            };
            let mut s = ST.lock().unwrap();
            say_text(&mut s, &text, 12);
            save_cfg(&s);
        }
        MAct::BuildOpen(i) => {
            if let Some((_, _, url)) = builds::list().get(i).filter(|b| !b.2.is_empty()) {
                launch::open(&launch::Entry { name: String::new(), target: url.clone() });
                reanchor(hwnd, close_menu);
            }
        }
        MAct::BuildForget(i) => {
            builds::remove(i);
            save_cfg(&ST.lock().unwrap());
        }
        MAct::TokenAdd => {
            let text = match launch::copied_text().filter(|t| builds::is_token(t)) {
                None => "Copy a GitHub token first. It starts with ghp_ or github_pat_.",
                Some(t) if builds::set_token(&t) => "I keep that token safe. Private repositories work now.",
                Some(_) => "I could not keep that token.",
            };
            say_text(&mut ST.lock().unwrap(), text, 14);
        }
        MAct::TokenForget => {
            builds::set_token("");
        }
        MAct::UpdateDaily => {
            let mut s = ST.lock().unwrap();
            s.updates = if s.updates == update::YES { update::NO } else { update::YES };
            s.question = false;
            save_cfg(&s);
        }
        MAct::UpdateNow => update::check(),
        MAct::UpdateGet => {
            if let Some((_, page)) = update::newest() {
                launch::open(&launch::Entry { name: String::new(), target: page });
                reanchor(hwnd, close_menu);
            }
        }
        MAct::Cal => ST.lock().unwrap().page = PAGE_CAL,
        MAct::CalBefore => {
            let mut s = ST.lock().unwrap();
            s.cal_before = (s.cal_before + 1) % calendar::BEFORE.len();
            save_cfg(&s);
        }
        MAct::CalLink => {
            let text = match launch::copied_link().as_deref().and_then(calendar::link) {
                None => "Copy the link of the calendar first. It starts with https.",
                Some(l) if calendar::add(&l) => "I will remind you of what is in that calendar.",
                Some(_) => "I know that one already, or have too many.",
            };
            say_text(&mut ST.lock().unwrap(), text, 12);
        }
        MAct::CalFile => {
            reanchor(hwnd, close_menu);
            pack::choose(hwnd, pack::Want::Calendar);
        }
        MAct::CalForget(i) => calendar::remove(i),
        MAct::Breaks => ST.lock().unwrap().page = PAGE_BREAKS,
        MAct::Garden => ST.lock().unwrap().page = PAGE_GARDEN,
        MAct::GardenSeed => {
            let mut s = ST.lock().unwrap();
            s.garden.next_seed();
            save_cfg(&s);
        }
        MAct::GardenPot(i) => {
            let mut s = ST.lock().unwrap();
            let day = (local_now().0 / 1440) as u32;
            match s.garden.state(i, day) {
                garden::State::Empty => {
                    s.garden.plant(i, day);
                }
                garden::State::Flower => {
                    if let Some(seed) = s.garden.pick(i) {
                        let name = garden::SEEDS[seed as usize].name.to_lowercase();
                        say_text(&mut s, &format!("A {name} for the shelf in my room!"), 12);
                        body::gain(&mut s, garden::XP_PICK);
                    }
                }
                garden::State::Thirsty | garden::State::Wilted => water(&mut s, day),
                garden::State::Growing => say_text(&mut s, "It has had water today. Come back tomorrow!", 10),
            }
            save_cfg(&s);
        }
        MAct::GardenWater => {
            let mut s = ST.lock().unwrap();
            let day = (local_now().0 / 1440) as u32;
            water(&mut s, day);
            save_cfg(&s);
        }
        MAct::BirthMonth | MAct::BirthDay | MAct::BirthForget => {
            let mut s = ST.lock().unwrap();
            s.birthday = match (a, s.birthday) {
                (MAct::BirthForget, _) => None,
                (_, None) => Some((1, 1)),
                (MAct::BirthMonth, Some((m, d))) => {
                    let m = m % 12 + 1;
                    Some((m, d.min(seasons::days_in(m, 2024))))
                }
                (_, Some((m, d))) => Some((m, d % seasons::days_in(m, 2024) + 1)),
            };
            save_cfg(&s);
        }
        MAct::FactTopic(t) => {
            let mut s = ST.lock().unwrap();
            let off = s.fact_off ^ (1 << t);
            // one topic stays on, or there would be nothing to tell
            if topics_off(off) < fact_topics().len() {
                s.fact_off = off;
                save_cfg(&s);
            } else {
                say_text(&mut s, "Keep one topic, or I'd have nothing to tell! Turn SHOW FACTS off instead.", 14);
            }
        }
        MAct::BreakOn => {
            let mut s = ST.lock().unwrap();
            s.care_on = !s.care_on;
            save_cfg(&s);
        }
        MAct::BreakEvery(i) => {
            let mut s = ST.lock().unwrap();
            s.care_every[i] = (s.care_every[i] + 1) % care::EVERY[i].len();
            s.care_on = true;
            save_cfg(&s);
        }
        MAct::Drank => {
            let mut s = ST.lock().unwrap();
            let day = local_now().1;
            s.water = (day, if s.water.0 == day { s.water.1 + 1 } else { 1 });
            s.coach.done(care::Care::Water);
            body::gain(&mut s, 1);
            save_cfg(&s);
        }
        MAct::EyesNow => {
            reanchor(hwnd, close_menu);
            let mut s = ST.lock().unwrap();
            s.coach.done(care::Care::Eyes);
            s.eye_left = care::EYE_REST;
        }
        MAct::PetTab(p) => {
            let mut s = ST.lock().unwrap();
            s.page = PAGE_PICK;
            s.picking_friend = false;
            s.pet_page = p;
        }
        MAct::Packs => {
            pack::refresh();
            settle(hwnd);
            let mut s = ST.lock().unwrap();
            s.page = PAGE_PACKS;
            s.pack_sure = None;
            s.pack_note.clear();
        }
        MAct::Gallery => {
            ST.lock().unwrap().page = PAGE_GALLERY;
            gallery::open();
        }
        MAct::GalleryGet(i) => gallery::get(hwnd, i),
        MAct::PackAdd => {
            // out of the way of the dialog
            reanchor(hwnd, close_menu);
            pack::choose(hwnd, pack::Want::Pack);
        }
        MAct::PackFolder => {
            if let Some(d) = pack::folder() {
                let _ = std::fs::create_dir_all(&d);
                launch::open(&launch::Entry { name: String::new(), target: d.to_string_lossy().into_owned() });
            }
            reanchor(hwnd, close_menu);
        }
        MAct::PackForget(i) => {
            let sure = std::mem::replace(&mut ST.lock().unwrap().pack_sure, Some(i)) == Some(i);
            if sure {
                let gone = pack::remove(i);
                settle(hwnd);
                let mut s = ST.lock().unwrap();
                s.pack_sure = None;
                s.pack_note = if gone { String::new() } else { "I could not remove it.".into() };
            }
        }
        MAct::Options => ST.lock().unwrap().page = PAGE_OPTIONS,
        MAct::Group(g) => ST.lock().unwrap().page = PAGE_GROUP + g,
        MAct::Minimal => {
            reanchor(hwnd, close_menu);
            toggle_minimal(hwnd);
        }
        MAct::Tasks => {
            reanchor(hwnd, close_menu);
            show_todo();
        }
        MAct::Focus => reanchor(hwnd, |s| {
            close_menu(s);
            s.focus = !s.focus;
        }),
        MAct::Stats => {
            reanchor(hwnd, close_menu);
            show_stats();
        }
        MAct::Corner(n) => {
            reanchor(hwnd, close_menu);
            let mut wr: RECT = zeroed();
            GetWindowRect(hwnd, &mut wr);
            let wa = work_at((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
            put_in_corner(hwnd, n % 2 == 0, n < 2, wa);
        }
        MAct::NextMonitor => {
            reanchor(hwnd, close_menu);
            next_monitor(hwnd);
        }
        MAct::Nap => {
            reanchor(hwnd, close_menu);
            let mut s = ST.lock().unwrap();
            let focusing = s.focus && s.pomo_running;
            if !focusing {
                s.mood.start_nap();
            }
        }
        MAct::FeedPage => ST.lock().unwrap().page = PAGE_FEED,
        MAct::Feed(i) => {
            reanchor(hwnd, close_menu);
            feed_pet(hwnd, Some(i));
        }
        MAct::Ball => {
            reanchor(hwnd, close_menu);
            ball::toggle();
        }
        MAct::Media(c) => media::command(c),
        MAct::PickKey(i) => {
            let mut s = ST.lock().unwrap();
            s.key_wait = if s.key_wait == Some(i) { None } else { Some(i) };
            if s.key_wait.is_some() {
                keys::unregister(hwnd); // so pressing a key does not set something off
            } else {
                keys::register(hwnd, &s.keys, s.shortcut);
            }
        }
        MAct::NoKey(i) => {
            let mut s = ST.lock().unwrap();
            s.keys[i] = 0;
            s.key_wait = None;
            keys::register(hwnd, &s.keys, s.shortcut);
            save_cfg(&s);
        }
        MAct::Page(p) => ST.lock().unwrap().page = p,
        MAct::PickFriend => {
            let mut s = ST.lock().unwrap();
            s.page = PAGE_PICK;
            s.picking_friend = true;
            s.pet_page = pack::tab_of(s.friend.saturating_sub(1));
        }
        MAct::PickFor(f) => ST.lock().unwrap().picking_friend = f,
        MAct::NoFriend => {
            let mut s = ST.lock().unwrap();
            s.friend = 0;
            save_cfg(&s);
            drop(s);
            friend::reload();
        }
        MAct::LaunchPage => ST.lock().unwrap().page = PAGE_LAUNCH,
        MAct::Launch(i) => {
            let e = ST.lock().unwrap().launch.get(i).cloned();
            reanchor(hwnd, close_menu);
            if e.is_some_and(|e| !launch::open(&e)) {
                say_text(&mut ST.lock().unwrap(), "That did not open.", 12);
            }
        }
        MAct::Forget(i) => {
            let mut s = ST.lock().unwrap();
            if i < s.launch.len() {
                s.launch.remove(i);
                launch::save(&s.launch);
            }
        }
        MAct::AddFront => {
            let t = ST.lock().unwrap().last_path.clone();
            add_launch(&t);
        }
        MAct::AddLink => {
            if let Some(t) = launch::copied_link() {
                add_launch(&t);
            }
        }
        MAct::Wardrobe => ST.lock().unwrap().page = PAGE_WARDROBE,
        MAct::Wear(i) => {
            let mut s = ST.lock().unwrap();
            if wardrobe::worn(i, s.xp) == i {
                s.hat = i;
                save_cfg(&s);
            }
        }
        MAct::Calc => {
            reanchor(hwnd, close_menu);
            tools::show_calc();
        }
        MAct::Notes => {
            reanchor(hwnd, close_menu);
            tools::show_notes();
        }
        MAct::Maker => {
            reanchor(hwnd, close_menu);
            maker::show();
        }
        MAct::Clips => {
            reanchor(hwnd, close_menu);
            clips::show();
        }
        MAct::Code => {
            reanchor(hwnd, close_menu);
            open_code(hwnd);
        }
        MAct::Hide => toggle_hidden(hwnd),
        MAct::Trick => {
            reanchor(hwnd, close_menu);
            body::perform(&mut ST.lock().unwrap());
        }
        MAct::Quit => {
            DestroyWindow(hwnd);
        }
        MAct::SetPet(i) => {
            if ST.lock().unwrap().picking_friend {
                let mut s = ST.lock().unwrap();
                // picking the one that is there sends it home
                s.friend = if s.friend == i + 1 { 0 } else { i + 1 };
                save_cfg(&s);
                drop(s);
                friend::reload();
            } else {
                reanchor(hwnd, |s| {
                    s.body.stop();
                    s.body.meal = None;
                    s.pet = pack::still(i);
                });
            }
        }
        MAct::Opt(i) => {
            if i == 6 {
                let next = (ST.lock().unwrap().size + 1) % SIZES.len();
                set_size(hwnd, next);
                return;
            }
            if i == 9 {
                ST.lock().unwrap().page = PAGE_POSITION;
                return;
            }
            if i == 11 {
                reanchor(hwnd, close_menu);
                let mut s = ST.lock().unwrap();
                s.intro = 0;
                s.intro_wait = 2;
                s.msg_left = 0;
                return;
            }
            if i == 38 {
                ST.lock().unwrap().page = PAGE_UPDATE;
                return;
            }
            if i == 41 {
                ST.lock().unwrap().page = PAGE_BIRTHDAY;
                return;
            }
            if i == 46 {
                ST.lock().unwrap().page = PAGE_FACTS;
                return;
            }
            if i == 42 {
                let mut s = ST.lock().unwrap();
                quiet_next(&mut s, local_now().0);
                save_cfg(&s);
                drop(s);
                InvalidateRect(hwnd, null(), 0);
                return;
            }
            if i == 37 {
                ST.lock().unwrap().page = PAGE_KEYS;
                return;
            }
            if i == 10 {
                reanchor(hwnd, close_menu);
                let mut s = ST.lock().unwrap();
                next_fact(&mut s);
                s.msg_left = 0;
                s.fact_left = FACT_FOR;
                return;
            }
            if i == 20 {
                {
                    let mut s = ST.lock().unwrap();
                    s.on_top = !s.on_top;
                    save_cfg(&s);
                }
                presence::sync(hwnd);
                InvalidateRect(hwnd, null(), 0);
                return;
            }
            if i == 30 {
                // Windows can only listen for one of the two
                let on = !ST.lock().unwrap().shortcut;
                if on {
                    UnregisterHotKey(hwnd, keys::CLOSE as i32);
                }
                let now = instance::set_shortcut(on);
                let mut s = ST.lock().unwrap();
                s.shortcut = now;
                keys::register(hwnd, &s.keys, now);
                drop(s);
                InvalidateRect(hwnd, null(), 0);
                return;
            }
            if i == 21 {
                let on = {
                    let mut s = ST.lock().unwrap();
                    s.through = !s.through;
                    save_cfg(&s);
                    s.through
                };
                if on {
                    // the pet can no longer be clicked, so say how to get back here
                    reanchor(hwnd, close_menu);
                    say_text(&mut ST.lock().unwrap(), "Clicks now pass through me. Ctrl+Alt+P or my tray icon opens the hub.", 24);
                }
                InvalidateRect(hwnd, null(), 0);
                return;
            }
            let mut s = ST.lock().unwrap();
            match i {
                0 => {
                    s.facts_on = !s.facts_on;
                    if !s.facts_on {
                        s.fact_left = 0;
                    }
                }
                1 => s.show_up = !s.show_up,
                2 => s.bits = !s.bits,
                3 => s.care_on = !s.care_on,
                4 => s.sys_on = !s.sys_on,
                5 => s.dnd_on = !s.dnd_on,
                39 => s.seasons_on = !s.seasons_on,
                40 => s.south = !s.south,
                43 => s.quiet_focus = !s.quiet_focus,
                44 => s.quiet_meetings = !s.quiet_meetings,
                45 => {
                    s.files_on = !s.files_on;
                    downloads::ON.store(s.files_on, Relaxed);
                }
                12 => {
                    s.walk_on = !s.walk_on;
                    s.body.stop();
                }
                13 => s.tokens_on = !s.tokens_on,
                14 => s.cache_on = !s.cache_on,
                16 => {
                    s.theme = (s.theme + 1) % THEMES.len();
                    set_theme(s.theme);
                }
                17 => {
                    s.opacity = (s.opacity + 1) % OPACITIES.len();
                    crate::tick::OPACITY_PREVIEW.store(5, Relaxed); // show it for ~2 s even though the hub is open
                }
                18 => s.show_speed = !s.show_speed,
                19 => s.show_pc = !s.show_pc,
                22 => {
                    s.tricks_on = !s.tricks_on;
                    s.body.stop();
                }
                26 => s.dance_on = !s.dance_on,
                27 => s.talk_on = !s.talk_on,
                31 => s.react_on = !s.react_on,
                32 => s.buddy_on = !s.buddy_on,
                34 => s.perch_on = !s.perch_on,
                36 => s.show_now = !s.show_now,
                33 => {
                    s.sound_on = !s.sound_on;
                    sound::ON.store(s.sound_on, Relaxed);
                    sound::play(Sound::Happy);
                }
                28 => s.visit_on = !s.visit_on,
                29 => {
                    let app = s.last_app.clone();
                    if rules::is_listed(&s.hide_for, &app) {
                        s.hide_for.retain(|e| !e.eq_ignore_ascii_case(&app));
                    } else if !app.is_empty() && !app.contains('|') {
                        s.hide_for.push(app);
                    }
                }
                23 => s.gravity = !s.gravity,
                24 => s.click_hits = !s.click_hits,
                25 => s.nap_after = (s.nap_after + 1) % NAP_AFTER.len(),
                15 => {
                    let want = !s.autostart;
                    // only believe the toggle if Windows accepted the change
                    if autostart::set_enabled(want) {
                        s.autostart = want;
                    }
                }
                7 => {
                    let goals = [2u32, 4, 6, 8];
                    let at = goals.iter().position(|g| *g == s.stats.goal).unwrap_or(1);
                    s.stats.goal = goals[(at + 1) % goals.len()];
                }
                _ => {
                    s.pomo_preset = (s.pomo_preset + 1) % POMO.len();
                    pomo_reset(&mut s);
                }
            }
            save_cfg(&s);
            drop(s);
            friend::reload();
        }
    }
    InvalidateRect(hwnd, null(), 0);
}

pub(crate) unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let (mx, my) = ((lp & 0xFFFF) as i16 as i32, ((lp >> 16) & 0xFFFF) as i16 as i32);
    match msg {
        WM_CREATE => {
            SetTimer(hwnd, 1, TICK_MS, None);
            SetTimer(hwnd, 2, body::FRAME_MS, None);
            0
        }
        WM_TIMER => {
            if wp == 2 {
                track_hover(hwnd);
                catch_key(hwnd);
                let dt = body::step(hwnd);
                ball::step(hwnd, dt);
                pace(hwnd, body::lively());
                return 0;
            }
            on_tick(hwnd);
            friend::tend(hwnd);
            0
        }
        WM_HOTKEY => {
            match wp {
                1 => show_todo(),
                2 => reanchor(hwnd, |s| s.focus = !s.focus),
                3 => show_stats(),
                4 => toggle_minimal(hwnd),
                5 => toggle_hidden(hwnd),
                6 => toggle_hub(hwnd),
                7 => {
                    feed_pet(hwnd, None);
                    if let Some(w) = friend::other().filter(|_| !friend::is_friend()) {
                        PostMessageW(w, WM_HOTKEY, 7, 0); // one for each
                    }
                }
                8 => {
                    DestroyWindow(hwnd);
                }
                9 => tools::show_calc(),
                10 => tools::show_notes(),
                11 => open_code(hwnd),
                12 => clips::show(),
                97 if std::env::var_os("PIXEL_PET_TEST").is_some() => {
                    body::climb(&mut ST.lock().unwrap());
                }
                98 if std::env::var_os("PIXEL_PET_TEST").is_some() => body::perform(&mut ST.lock().unwrap()),
                99 if std::env::var_os("PIXEL_PET_TEST").is_some() => {
                    body::travel(&mut ST.lock().unwrap());
                }
                _ => {}
            }
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DROPFILES => {
            let files = launch::dropped(wp as _);
            if friend::is_friend() {
                return 0; // not its list
            }
            if let Some(p) = files.iter().find(|f| pack::is_pack(f)) {
                import_pack(hwnd, std::path::Path::new(p));
                return 0;
            }
            let added: Vec<&String> = files.iter().filter(|f| add_launch(f)).collect();
            let text = match added[..] {
                [] => "I know that one already, or my list is full.".to_string(),
                [one] => format!("{} is in my quick launch now.", launch::name_of(one)),
                _ => format!("{} things are in my quick launch now.", added.len()),
            };
            let mut s = ST.lock().unwrap();
            say_text(&mut s, &text, 10);
            0
        }
        pack::WM_PACK => {
            match pack::answer() {
                Some((pack::Want::Pack, pack::Answer::File(p))) => import_pack(hwnd, &p),
                Some((pack::Want::Calendar, pack::Answer::File(p))) => {
                    let text = if calendar::add(&p.to_string_lossy()) { "I will remind you of what is in that calendar." } else { "I know that one already, or have too many." };
                    say_text(&mut ST.lock().unwrap(), text, 12);
                }
                Some((pack::Want::Calendar, pack::Answer::CannotAsk)) => say_text(&mut ST.lock().unwrap(), "I need zenity or kdialog to ask for a file.", 12),
                Some((pack::Want::Pack, pack::Answer::CannotAsk)) => {
                    say_text(&mut ST.lock().unwrap(), "Put the pack into my folder, then open Packs again.", 14);
                    menu_action(hwnd, MAct::PackFolder);
                }
                _ => {}
            }
            0
        }
        gallery::WM_GOT => {
            match gallery::got() {
                Some(Ok(b)) => imported(hwnd, pack::import_bytes(b)),
                Some(Err(e)) => say_text(&mut ST.lock().unwrap(), &e, 14),
                None => {}
            }
            0
        }
        builds::WM_TELL => {
            builds::heard(wp);
            if wp == 2 && ST.lock().unwrap().buddy_on {
                body::react(&mut ST.lock().unwrap(), Clip::Think);
            }
            0
        }
        friend::WM_CHOOSE => {
            if !ST.lock().unwrap().menu {
                toggle_hub(hwnd);
            }
            let mut s = ST.lock().unwrap();
            s.page = PAGE_PICK;
            s.picking_friend = true;
            s.pet_page = pack::tab_of(s.friend.saturating_sub(1));
            s.menu_ticks = 0;
            drop(s);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        friend::WM_RELOAD => {
            pack::refresh();
            reanchor(hwnd, |s| {
                let pos = s.pos;
                load_cfg(s);
                friend::trim(s);
                s.pos = pos;
                s.body.stop();
                set_theme(s.theme);
                sound::ON.store(s.sound_on, Relaxed);
            });
            0
        }
        friend::WM_PLAY => {
            if let Some(p) = friend::decode(wp) {
                body::invited(&mut ST.lock().unwrap(), p, lp as f32);
            }
            0
        }
        WM_DPICHANGED => 0, // body::step follows the monitor by itself
        instance::WM_COME_OUT => {
            ST.lock().unwrap().hidden = false;
            presence::sync(hwnd);
            body::perform(&mut ST.lock().unwrap());
            0
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            ST.lock().unwrap().body.screens.clear();
            place(hwnd);
            0
        }
        WM_LBUTTONDOWN => {
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let mut wr: RECT = zeroed();
            GetWindowRect(hwnd, &mut wr);
            SetCapture(hwnd);
            pace(hwnd, true);
            let mut s = ST.lock().unwrap();
            s.down = true;
            s.dragging = false;
            s.cur0 = (p.x, p.y);
            s.win0 = (wr.left, wr.top);
            s.body.on_pet = false;
            if !s.menu && contains(&s.pet_rect, mx, my) {
                let (fx, fy) = geo(&s).feet();
                body::grab(&mut s, (p.x, p.y), ((wr.left + fx) as f32, (wr.top + fy) as f32));
            }
            0
        }
        WM_MOUSEMOVE => {
            let mut p: POINT = zeroed();
            GetCursorPos(&mut p);
            let target = {
                let mut s = ST.lock().unwrap();
                if !s.down {
                    // stroking the pet with the mouse pets it
                    if contains(&s.pet_rect, mx, my) {
                        if let Some((lx, ly)) = s.last_mouse {
                            s.stroke += (mx - lx).abs() + (my - ly).abs();
                        }
                        s.last_mouse = Some((mx, my));
                        if s.stroke > (220.0 * s.scale) as i32 {
                            s.stroke = 0;
                            sound::play(Sound::Happy);
                            body::gain(&mut s, wardrobe::XP_PET);
                            if let Some(ev) = s.mood.on_petted() {
                                say(&mut s, ev);
                            }
                        }
                    } else {
                        s.last_mouse = None;
                        s.stroke = 0;
                    }
                    None
                } else {
                    let (dx, dy) = (p.x - s.cur0.0, p.y - s.cur0.1);
                    if !s.dragging && (dx.abs() > 4 || dy.abs() > 4) {
                        s.dragging = true;
                    }
                    if s.dragging && s.body.on_pet {
                        body::drag(&mut s, (dx, dy)); // the body follows in its own time
                        None
                    } else if s.dragging {
                        Some((s.win0.0 + dx, s.win0.1 + dy, geo(&s)))
                    } else {
                        None
                    }
                }
            };
            if let Some((x, y, g)) = target {
                let wa = work_at(p.x, p.y);
                let (x, y) = clamp_in(&g.bounds(&wa), x, y, g.w, g.h);
                SetWindowPos(hwnd, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            let (was_down, dragged) = {
                let mut s = ST.lock().unwrap();
                let r = (s.down, s.dragging);
                s.down = false;
                s.dragging = false;
                if r.1 && s.body.on_pet {
                    body::release(&mut s);
                }
                s.body.on_pet = false;
                r
            };
            ReleaseCapture();
            if was_down {
                if dragged {
                    let mut wr: RECT = zeroed();
                    GetWindowRect(hwnd, &mut wr);
                    let mut s = ST.lock().unwrap();
                    s.pos = Some((wr.left, wr.top));
                    save_cfg(&s);
                } else {
                    handle_click(hwnd, mx, my);
                }
            }
            0
        }
        WM_CAPTURECHANGED => {
            // something else took the mouse away in the middle of a drag
            let mut s = ST.lock().unwrap();
            s.down = false;
            s.dragging = false;
            0
        }
        WM_RBUTTONUP => {
            // right click opens the same pixel hub as left click (no native menus)
            ask_for_hub(hwnd);
            0
        }
        tray::WM_TRAY => {
            tray::on_message(hwnd, lp);
            0
        }
        WM_DESTROY => {
            if !friend::is_friend() {
                tray::remove(hwnd);
                if let Some(w) = friend::other() {
                    PostMessageW(w, WM_CLOSE, 0, 0);
                }
            }
            tools::save_note();
            {
                let s = ST.lock().unwrap();
                save_cfg(&s);
                save_usage(&s);
                save_todos(&s);
            }
            PostQuitMessage(0);
            0
        }
        m if tray::is_taskbar_created(m) => {
            tray::add(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
