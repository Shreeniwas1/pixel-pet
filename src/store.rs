//! Saving and loading settings, usage, todos and focus stats (all in %APPDATA%).

use crate::prelude::*;

// ======================================================================
// files: settings, usage, todos (all in %APPDATA%)
// ======================================================================

pub(crate) fn data_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join(name))
}

pub(crate) fn load_cfg(s: &mut State) {
    let Some(txt) = data_path("pixel-pet.cfg").and_then(|p| std::fs::read_to_string(p).ok()) else { return };
    for (n, line) in txt.lines().enumerate() {
        // older versions wrote bare lines: pet, facts, upload
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (["pet", "facts", "up"].get(n).copied().unwrap_or(""), line.trim()),
        };
        match k {
            "pet" => {
                if let Some(i) = pack::find(v) {
                    s.pet = i;
                }
            }
            "facts" => s.facts_on = v != "0",
            "up" => s.show_up = v != "0",
            "speed" => s.show_speed = v != "0",
            "bits" => s.bits = v == "1",
            "pc" => s.show_pc = v == "1",
            "min" => s.minimal = v == "1",
            "top" => s.on_top = v != "0",
            "through" => s.through = v == "1",
            "focus" => s.focus = v == "1",
            "care" => s.care_on = v != "0",
            "clips" => {
                s.clips_on = v == "1";
                clips::ON.store(s.clips_on && !friend::is_friend(), Relaxed);
            }
            "calendar" => s.cal_before = v.parse::<usize>().unwrap_or(calendar::USUAL).min(calendar::BEFORE.len() - 1),
            "repos" => builds::from_text(v),
            "updates" => {
                let (how, told) = v.split_once(':').unwrap_or((v, ""));
                s.updates = how.parse::<u8>().unwrap_or(update::ASK).min(update::NO);
                s.update_told = told.to_string();
            }
            "breaks" => s.care_every = care::from_text(v),
            "water" => {
                let day = unsafe { local_now().1 };
                s.water = (day, care::glasses(v, day));
            }
            "dnd" => s.dnd_on = v != "0",
            "seasons" => s.seasons_on = v != "0",
            "south" => s.south = v == "1",
            "birthday" => s.birthday = seasons::birthday_from(v),
            "born" => s.born = v.parse().unwrap_or(0),
            "greeted" => s.greeted = v.parse().unwrap_or(0),
            "season" => s.season_said = v.parse().unwrap_or(0),
            "garden" => s.garden = garden::Garden::from_text(v),
            "quiet" => {
                let (until, step) = v.split_once(':').unwrap_or((v, "0"));
                s.quiet_until = until.parse().unwrap_or(0);
                s.quiet_step = step.parse::<u8>().unwrap_or(0).min(QUIET_FOR.len() as u8);
            }
            "quietfocus" => s.quiet_focus = v != "0",
            "quietmeet" => s.quiet_meetings = v != "0",
            "walk" => s.walk_on = v != "0",
            "tricks" => s.tricks_on = v != "0",
            "gravity" => s.gravity = v != "0",
            "hits" => s.click_hits = v != "0",
            "visit" => s.visit_on = v != "0",
            "dance" => s.dance_on = v != "0",
            "react" => s.react_on = v != "0",
            "buddy" => s.buddy_on = v != "0",
            "perch" => s.perch_on = v == "1",
            "now" => s.show_now = v == "1",
            "keys" => s.keys = keys::from_text(v),
            "friend" => {
                s.friend = match v.parse::<usize>() {
                    Ok(0) => 0,
                    Ok(n) => pack::find(&(n - 1).to_string()).map_or(0, |i| i + 1),
                    Err(_) => pack::find(v).map_or(0, |i| i + 1),
                }
            }
            "xp" => s.xp = v.parse().unwrap_or(0),
            "hat" => s.hat = v.parse::<usize>().unwrap_or(0).min(wardrobe::HATS.len() - 1),
            "sound" => s.sound_on = v == "1",
            "talk" => s.talk_on = v != "0",
            "hidefor" => s.hide_for = v.split('|').filter(|e| !e.is_empty()).map(str::to_string).collect(),
            "napafter" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.nap_after = i.min(NAP_AFTER.len() - 1);
                }
            }
            "food" => {
                if let Ok(f) = v.parse::<f32>() {
                    s.mood.food = f.clamp(0.0, 100.0);
                }
            }
            "ai" => s.tokens_on = v != "0",
            "cache" => s.cache_on = v == "1",
            "theme" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.theme = i.min(THEMES.len() - 1);
                }
            }
            "opacity" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.opacity = i.min(OPACITIES.len() - 1);
                }
            }
            "intro" => {
                if v == "1" {
                    s.intro = intro::DONE;
                }
            }
            "sys" => s.sys_on = v != "0",
            "goal" => {
                if let Ok(g) = v.parse::<u32>() {
                    s.stats.goal = g.clamp(1, 20);
                }
            }
            "happy" => {
                if let Ok(f) = v.parse::<f32>() {
                    s.mood.happy = f.clamp(0.0, 100.0);
                }
            }
            "energy" => {
                if let Ok(f) = v.parse::<f32>() {
                    s.mood.energy = f.clamp(0.0, 100.0);
                }
            }
            "lastrun" => {
                if let Ok(m) = v.parse::<i64>() {
                    s.mood.last_run = m;
                }
            }
            "pomo" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.pomo_preset = i.min(POMO.len() - 1);
                }
            }
            "size" => {
                if let Ok(i) = v.parse::<usize>() {
                    s.size = i.min(SIZES.len() - 1);
                }
            }
            "x" => {
                if let Ok(x) = v.parse::<i32>() {
                    s.pos = Some((x, s.pos.map_or(0, |p| p.1)));
                }
            }
            "y" => {
                if let Ok(y) = v.parse::<i32>() {
                    s.pos = Some((s.pos.map_or(0, |p| p.0), y));
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn save_cfg(s: &State) {
    if friend::is_friend() {
        return; // it all belongs to the first pet
    }
    if let Some(p) = data_path("pixel-pet.cfg") {
        let mut t = format!(
            "pet={}\nfacts={}\nup={}\nbits={}\nsize={}\nfocus={}\npomo={}\nhappy={:.1}\nenergy={:.1}\nlastrun={}\ncare={}\nsys={}\ngoal={}\ndnd={}\nintro={}\nwalk={}\nai={}\ncache={}\ntheme={}\nopacity={}\nspeed={}\npc={}\nmin={}\ntop={}\nthrough={}\ntricks={}\ngravity={}\nhits={}\nnapafter={}\nfood={:.1}\nvisit={}\ndance={}\ntalk={}\nhidefor={}\nsound={}\nxp={}\nhat={}\nreact={}\nbuddy={}\nperch={}\nnow={}\nkeys={}\nfriend={}\n",
            pack::key_of(s.pet),
            s.facts_on as u8,
            s.show_up as u8,
            s.bits as u8,
            s.size,
            s.focus as u8,
            s.pomo_preset,
            s.mood.happy,
            s.mood.energy,
            s.mood.last_run,
            s.care_on as u8,
            s.sys_on as u8,
            s.stats.goal,
            s.dnd_on as u8,
            (s.intro == intro::DONE) as u8,
            s.walk_on as u8,
            s.tokens_on as u8,
            s.cache_on as u8,
            s.theme,
            s.opacity,
            s.show_speed as u8,
            s.show_pc as u8,
            s.minimal as u8,
            s.on_top as u8,
            s.through as u8,
            s.tricks_on as u8,
            s.gravity as u8,
            s.click_hits as u8,
            s.nap_after,
            s.mood.food,
            s.visit_on as u8,
            s.dance_on as u8,
            s.talk_on as u8,
            s.hide_for.join("|"),
            s.sound_on as u8,
            s.xp,
            s.hat,
            s.react_on as u8,
            s.buddy_on as u8,
            s.perch_on as u8,
            s.show_now as u8,
            keys::to_text(&s.keys),
            match s.friend {
                n if n <= OURS.len() => n.to_string(),
                n => pack::key_of(n - 1),
            }
        );
        t.push_str(&format!("seasons={}\nsouth={}\nbirthday={}\nborn={}\ngreeted={}\nseason={}\n", s.seasons_on as u8, s.south as u8, seasons::birthday_text(s.birthday), s.born, s.greeted, s.season_said));
        t.push_str(&format!("garden={}\nquiet={}:{}\nquietfocus={}\nquietmeet={}\n", s.garden.to_text(), s.quiet_until, s.quiet_step, s.quiet_focus as u8, s.quiet_meetings as u8));
        t.push_str(&format!("breaks={}\nwater={}:{}\nclips={}\ncalendar={}\nrepos={}\nupdates={}:{}\n", care::to_text(&s.care_every), s.water.0, s.water.1, s.clips_on as u8, s.cal_before, builds::to_text(), s.updates, s.update_told));
        if let Some((x, y)) = s.pos {
            t.push_str(&format!("x={x}\ny={y}\n"));
        }
        let _ = std::fs::write(p, t);
    }
}

pub(crate) fn load_usage(s: &mut State) {
    let Some(txt) = data_path("pixel-pet-usage.txt").and_then(|p| std::fs::read_to_string(p).ok()) else { return };
    let v: Vec<u64> = txt.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if v.len() == 6 {
        s.day_key = v[0] as u32;
        s.day_rx = v[1];
        s.day_tx = v[2];
        s.mon_key = v[3] as u32;
        s.mon_rx = v[4];
        s.mon_tx = v[5];
    }
}

pub(crate) fn save_usage(s: &State) {
    if friend::is_friend() {
        return;
    }
    if let Some(p) = data_path("pixel-pet-usage.txt") {
        let _ = std::fs::write(
            p,
            format!("{} {} {} {} {} {}\n", s.day_key, s.day_rx, s.day_tx, s.mon_key, s.mon_rx, s.mon_tx),
        );
    }
}

pub(crate) fn save_todos(s: &State) {
    if friend::is_friend() {
        return;
    }
    if let Some(p) = data_path("pixel-pet-todos.txt") {
        todo::save(&p, &s.todos);
    }
}

pub(crate) fn save_stats(s: &State) {
    if friend::is_friend() {
        return;
    }
    if let Some(p) = data_path("pixel-pet-focus.txt") {
        let _ = std::fs::write(p, s.stats.to_text());
    }
}
