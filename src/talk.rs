//! What the pet says: facts, speech bubbles and mood lines.

use crate::prelude::*;

// Facts come from data/facts.txt (bundled into the exe): one per line, under "# Topic" lines.
pub(crate) static FACTS_TXT: &str = include_str!("../data/facts.txt");

/// The topics, and each fact with the topic it is under.
fn facts() -> &'static (Vec<&'static str>, Vec<(usize, &'static str)>) {
    static FACTS: std::sync::OnceLock<(Vec<&'static str>, Vec<(usize, &'static str)>)> = std::sync::OnceLock::new();
    FACTS.get_or_init(|| {
        let (mut topics, mut list) = (Vec::new(), Vec::new());
        for l in FACTS_TXT.lines().map(str::trim).filter(|l| !l.is_empty()) {
            match l.strip_prefix('#') {
                Some(name) => topics.push(name.trim()),
                None => list.push((topics.len().saturating_sub(1), l)),
            }
        }
        if topics.is_empty() {
            topics.push("Facts");
        }
        (topics, list)
    })
}

/// The names of the fact topics, in the order of the file.
pub(crate) fn fact_topics() -> &'static [&'static str] {
    &facts().0
}

#[cfg(test)]
fn fact_count() -> usize {
    facts().1.len().max(1)
}

pub(crate) fn fact_text(i: usize) -> String {
    format!("Did you know {}", facts().1.get(i).map_or("that facts are fun?", |f| f.1))
}

/// The facts of the topics that are not turned off (bit `t` of `off` is topic `t`), or all of
/// them if every topic is.
fn fact_pool(off: u32) -> Vec<usize> {
    let list = &facts().1;
    let on: Vec<usize> = (0..list.len()).filter(|&i| off & (1 << list[i].0.min(31)) == 0).collect();
    if on.is_empty() { (0..list.len().max(1)).collect() } else { on }
}

/// A fact to begin with, from a number that differs each time.
pub(crate) fn first_fact(seed: usize, off: u32) -> usize {
    let pool = fact_pool(off);
    pool[seed % pool.len()]
}

/// How many fact topics are turned off.
pub(crate) fn topics_off(off: u32) -> usize {
    (0..fact_topics().len()).filter(|t| off & (1 << t) != 0).count()
}

/// The topics that are off, by name, for the settings file.
pub(crate) fn topics_off_text(off: u32) -> String {
    fact_topics().iter().enumerate().filter(|(t, _)| off & (1 << t) != 0).map(|(_, n)| *n).collect::<Vec<_>>().join("|")
}

/// The other way; names it no longer has are skipped.
pub(crate) fn topics_off_from(text: &str) -> u32 {
    text.split('|').filter_map(|n| fact_topics().iter().position(|t| t.eq_ignore_ascii_case(n.trim()))).fold(0, |m, t| m | 1 << t)
}

/// The pet says something in its own words (picked by species and mood event), unless it is
/// meant to be quiet.
pub(crate) fn say(s: &mut State, ev: Ev) {
    if !presence::says(s.minimal, s.hidden, ev) || (!s.talk_on && ev != Ev::Focus) {
        return;
    }
    s.say_n += 1;
    let pet = &PETS[s.pet];
    // its own words for this, or for anything, if it has some
    let own: Vec<&str> = pet.says.iter().filter(|(e, _)| *e == Some(ev)).map(|(_, t)| *t).collect();
    let own = if own.is_empty() { pet.says.iter().filter(|(e, _)| e.is_none()).map(|(_, t)| *t).collect() } else { own };
    s.msg = if own.is_empty() { mood::line(pet.voice, ev, s.say_n) } else { own[s.say_n as usize % own.len()] }.to_string();
    s.msg_left = 14;
    s.fact_left = 0;
}

/// A plain-text message from the pet (reminders, alerts).
pub(crate) fn say_text(s: &mut State, text: &str, ticks: u32) {
    s.msg = keys::named(text);
    s.msg_left = ticks;
    s.fact_left = 0;
}

pub(crate) fn next_fact(s: &mut State) {
    s.fact = fact_after(s.fact, s.tick as usize, s.fact_off);
}

/// The fact to tell after `fact`, skipping ahead by an amount that changes with `tick`.
fn fact_after(fact: usize, tick: usize, off: u32) -> usize {
    let pool = fact_pool(off);
    let n = pool.len();
    let at = pool.iter().position(|&i| i >= fact).unwrap_or(0);
    let skip = if n > 1 { 1 + (tick * 7) % (n - 1) } else { 0 };
    pool[(at + skip) % n]
}

#[cfg(test)]
mod content_tests {
    use super::*;

    #[test]
    fn facts_are_well_formed() {
        let lines: Vec<&str> = facts().1.iter().map(|f| f.1).collect();
        assert!(lines.len() >= 100, "keep a healthy pool of facts");
        for l in &lines {
            assert!(l.starts_with("that "), "fact must continue 'Did you know ...': {l}");
            assert!(l.ends_with('?'), "fact must end with a question mark: {l}");
            assert!(l.chars().count() <= 110, "fact too long for the speech bubble: {l}");
            assert!(!l.contains("()") && !l.contains("[["), "stray markup: {l}");
        }
        let mut uniq = lines.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), lines.len(), "duplicate facts");
    }

    #[test]
    fn every_topic_has_facts_and_a_short_name() {
        let (topics, list) = facts();
        assert!(FACTS_TXT.trim_start().starts_with('#'), "the first fact needs a topic");
        assert!(topics.len() >= 2 && topics.len() <= 12, "the topics page fits up to 12");
        for (t, name) in topics.iter().enumerate() {
            assert!(!name.is_empty() && name.len() <= 26 && !name.contains('|'), "topic name {name:?}");
            assert!(list.iter().filter(|f| f.0 == t).count() >= 5, "topic {name} has too few facts");
        }
    }

    #[test]
    fn facts_come_only_from_topics_that_are_on() {
        let n = fact_topics().len();
        for t in 0..n {
            let off = !(1u32 << t); // only topic t
            let mut fact = first_fact(12345, off);
            for tick in 0..300 {
                fact = fact_after(fact, tick, off);
                assert_eq!(facts().1[fact].0, t);
            }
        }
        // every topic off is the same as none
        assert_eq!(fact_pool(u32::MAX).len(), fact_count());
        let off = 0b101;
        assert_eq!(topics_off_from(&topics_off_text(off)), off);
        assert_eq!(topics_off(off), 2);
        assert_eq!(topics_off_from("no such topic|"), 0);
    }

    #[test]
    fn next_fact_moves_on() {
        let mut fact = 0;
        for tick in 0..50 {
            let was = fact;
            fact = fact_after(fact, tick, 0);
            assert_ne!(fact, was);
        }
    }
}
