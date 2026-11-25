//! What the pet says: facts, speech bubbles and mood lines.

use crate::prelude::*;

// Facts come from data/facts.txt (one per line, bundled into the exe).
pub(crate) static FACTS_TXT: &str = include_str!("../data/facts.txt");

pub(crate) fn fact_count() -> usize {
    FACTS_TXT.lines().count().max(1)
}

pub(crate) fn fact_text(i: usize) -> String {
    format!("Did you know {}", FACTS_TXT.lines().nth(i).unwrap_or("that facts are fun?"))
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
    s.fact = (s.fact + 1 + (s.tick as usize * 7) % (fact_count() - 1)) % fact_count();
}

#[cfg(test)]
mod content_tests {
    use super::*;

    #[test]
    fn facts_are_well_formed() {
        let lines: Vec<&str> = FACTS_TXT.lines().collect();
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
}
