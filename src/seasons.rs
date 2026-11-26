//! Day and night, the seasons, and a few days that are not like the others: what the pet wears,
//! what drifts past it, and what it says when the day comes. Pure logic, no Win32.

use crate::wardrobe;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Daytime {
    Morning,
    Day,
    Evening,
    Night,
}

/// A day to celebrate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Special {
    Birthday,     // yours
    PetDay(u32),  // the pet's, and how old it is
    NewYear,
    Halloween,
}

/// How things are outside right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Look {
    pub season: Season,
    pub daytime: Daytime,
    pub special: Option<Special>,
}

/// What drifts past the pet now and then.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Fall {
    Flake,
    Leaf,
    Petal,
    Firefly,
}

/// A date written as yyyymmdd.
pub(crate) fn split(date: u32) -> (u32, u32, u32) {
    (date / 10000, date / 100 % 100, date % 100)
}

fn leap(y: u32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub(crate) fn days_in(month: u32, y: u32) -> u32 {
    match month {
        2 if leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub(crate) fn season(month: u32, south: bool) -> Season {
    // the south has the same seasons half a year later
    let m = if south { (month + 5) % 12 + 1 } else { month };
    match m {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Autumn,
        _ => Season::Winter,
    }
}

pub(crate) fn daytime(hour: u32) -> Daytime {
    match hour {
        5..=10 => Daytime::Morning,
        11..=17 => Daytime::Day,
        18..=22 => Daytime::Evening,
        _ => Daytime::Night,
    }
}

/// Is this (month, day) the one asked for? Somebody born on 29 February has it on the 28th in
/// the other years.
fn same_day((m, d): (u32, u32), (wm, wd): (u32, u32), y: u32) -> bool {
    (m, d) == (wm, wd) || (!leap(y) && (wm, wd) == (2, 29) && (m, d) == (2, 28))
}

/// Which special day today is, if any. `birthday` is (month, day); `born` the day the pet came.
pub(crate) fn special(date: u32, hour: u32, birthday: Option<(u32, u32)>, born: u32) -> Option<Special> {
    let (y, m, d) = split(date);
    if birthday.is_some_and(|b| same_day((m, d), b, y)) {
        return Some(Special::Birthday);
    }
    if born > 0 {
        let (by, bm, bd) = split(born);
        if y > by && same_day((m, d), (bm, bd), y) {
            return Some(Special::PetDay(y - by));
        }
    }
    if (m == 12 && d == 31 && hour >= 18) || (m == 1 && d == 1) {
        return Some(Special::NewYear);
    }
    if m == 10 && d == 31 {
        return Some(Special::Halloween);
    }
    None
}

/// How things are now, or None when the seasons are turned off.
pub(crate) fn look(on: bool, date: u32, hour: u32, south: bool, birthday: Option<(u32, u32)>, born: u32) -> Option<Look> {
    on.then(|| Look { season: season(split(date).1, south), daytime: daytime(hour), special: special(date, hour, birthday, born) })
}

/// The hat on its head: the one chosen, unless the day or the night asks for another.
pub(crate) fn hat(look: Option<Look>, asleep: bool, chosen: usize) -> usize {
    let Some(l) = look else { return chosen };
    match l.special {
        Some(Special::Birthday | Special::PetDay(_) | Special::NewYear) => wardrobe::PARTY,
        Some(Special::Halloween) => wardrobe::WITCH,
        None if asleep && l.daytime == Daytime::Night => wardrobe::NIGHTCAP,
        None => chosen,
    }
}

/// A scarf round its neck in winter.
pub(crate) fn scarf(look: Option<Look>) -> bool {
    look.is_some_and(|l| l.season == Season::Winter)
}

pub(crate) fn falls(l: Look) -> Option<Fall> {
    match (l.season, l.daytime) {
        (Season::Winter, _) => Some(Fall::Flake),
        (Season::Autumn, _) => Some(Fall::Leaf),
        (Season::Spring, _) => Some(Fall::Petal),
        (Season::Summer, Daytime::Evening | Daytime::Night) => Some(Fall::Firefly),
        (Season::Summer, _) => None,
    }
}

/// Late enough to yawn now and then.
pub(crate) fn sleepy(look: Option<Look>) -> bool {
    look.is_some_and(|l| matches!(l.daytime, Daytime::Evening | Daytime::Night))
}

pub(crate) fn greeting(s: Special) -> String {
    match s {
        Special::Birthday => "Happy birthday! I hope today is a lovely one.".into(),
        Special::PetDay(n) => format!("I'm {n} year{} old today! Thank you for having me.", if n == 1 { "" } else { "s" }),
        Special::NewYear => "Happy New Year! Here's to a good one.".into(),
        Special::Halloween => "Boo! Happy Halloween.".into(),
    }
}

/// A party is a reason for confetti.
pub(crate) fn party(s: Special) -> bool {
    s != Special::Halloween
}

pub(crate) fn arrived(s: Season) -> &'static str {
    match s {
        Season::Winter => "Winter is here. I found my scarf!",
        Season::Spring => "Spring is here. Everything is growing again!",
        Season::Summer => "Summer is here. Remember to drink some water!",
        Season::Autumn => "Autumn is here. Look at all the leaves!",
    }
}

pub(crate) fn index(s: Season) -> u8 {
    s as u8 + 1
}

pub(crate) const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/// "3-14", or nothing.
pub(crate) fn birthday_from(t: &str) -> Option<(u32, u32)> {
    let (m, d) = t.split_once('-')?;
    let (m, d) = (m.trim().parse::<u32>().ok()?, d.trim().parse::<u32>().ok()?);
    ((1..=12).contains(&m) && (1..=days_in(m, 2024)).contains(&d)).then_some((m, d))
}

pub(crate) fn birthday_text(b: Option<(u32, u32)>) -> String {
    b.map_or(String::new(), |(m, d)| format!("{m}-{d}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seasons_turn_and_the_south_has_them_the_other_way_round() {
        let north: Vec<Season> = (1..=12).map(|m| season(m, false)).collect();
        assert_eq!(north[0], Season::Winter);
        assert_eq!(north[3], Season::Spring);
        assert_eq!(north[6], Season::Summer);
        assert_eq!(north[9], Season::Autumn);
        assert_eq!(north[11], Season::Winter);
        assert_eq!(season(1, true), Season::Summer);
        assert_eq!(season(7, true), Season::Winter);
        assert_eq!(season(4, true), Season::Autumn);
        assert_eq!(season(10, true), Season::Spring);
        for m in 1..=12 {
            assert_eq!(north.iter().filter(|s| **s == north[m - 1]).count(), 3);
        }
    }

    #[test]
    fn the_day_has_four_parts() {
        assert_eq!(daytime(0), Daytime::Night);
        assert_eq!(daytime(4), Daytime::Night);
        assert_eq!(daytime(5), Daytime::Morning);
        assert_eq!(daytime(12), Daytime::Day);
        assert_eq!(daytime(18), Daytime::Evening);
        assert_eq!(daytime(23), Daytime::Night);
    }

    #[test]
    fn special_days_come_once_a_year() {
        let none = None;
        assert_eq!(special(20261231, 17, none, 0), None);
        assert_eq!(special(20261231, 19, none, 0), Some(Special::NewYear));
        assert_eq!(special(20270101, 9, none, 0), Some(Special::NewYear));
        assert_eq!(special(20271031, 9, none, 0), Some(Special::Halloween));
        assert_eq!(special(20270314, 9, Some((3, 14)), 0), Some(Special::Birthday));
        assert_eq!(special(20270315, 9, Some((3, 14)), 0), None);
        // a birthday wins over everything else
        assert_eq!(special(20270101, 9, Some((1, 1)), 0), Some(Special::Birthday));
        // 29 February, in a year without one
        assert_eq!(special(20270228, 9, Some((2, 29)), 0), Some(Special::Birthday));
        assert_eq!(special(20280228, 9, Some((2, 29)), 0), None);
        assert_eq!(special(20280229, 9, Some((2, 29)), 0), Some(Special::Birthday));
    }

    #[test]
    fn the_pet_has_a_birthday_from_its_first_year_on() {
        assert_eq!(special(20261126, 9, None, 20261126), None); // the day it came
        assert_eq!(special(20271126, 9, None, 20261126), Some(Special::PetDay(1)));
        assert_eq!(special(20291126, 9, None, 20261126), Some(Special::PetDay(3)));
        assert!(greeting(Special::PetDay(1)).contains("1 year old"));
        assert!(greeting(Special::PetDay(3)).contains("3 years old"));
    }

    #[test]
    fn what_it_wears() {
        let l = |season, daytime, special| Some(Look { season, daytime, special });
        let plain = l(Season::Summer, Daytime::Day, None);
        assert_eq!(hat(plain, false, 4), 4);
        assert_eq!(hat(plain, true, 4), 4); // a nap in the afternoon
        assert_eq!(hat(l(Season::Summer, Daytime::Night, None), true, 4), wardrobe::NIGHTCAP);
        assert_eq!(hat(l(Season::Summer, Daytime::Night, None), false, 4), 4);
        assert_eq!(hat(l(Season::Winter, Daytime::Day, Some(Special::Birthday)), false, 0), wardrobe::PARTY);
        assert_eq!(hat(l(Season::Autumn, Daytime::Day, Some(Special::Halloween)), false, 0), wardrobe::WITCH);
        assert_eq!(hat(None, true, 7), 7); // the seasons are off
        assert!(scarf(l(Season::Winter, Daytime::Day, None)));
        assert!(!scarf(plain) && !scarf(None));
    }

    #[test]
    fn something_drifts_past_in_every_season_but_a_summer_day() {
        let l = |season, daytime| Look { season, daytime, special: None };
        assert_eq!(falls(l(Season::Winter, Daytime::Day)), Some(Fall::Flake));
        assert_eq!(falls(l(Season::Autumn, Daytime::Night)), Some(Fall::Leaf));
        assert_eq!(falls(l(Season::Spring, Daytime::Morning)), Some(Fall::Petal));
        assert_eq!(falls(l(Season::Summer, Daytime::Evening)), Some(Fall::Firefly));
        assert_eq!(falls(l(Season::Summer, Daytime::Day)), None);
    }

    #[test]
    fn a_birthday_is_kept_as_text_and_nonsense_is_not() {
        assert_eq!(birthday_from("3-14"), Some((3, 14)));
        assert_eq!(birthday_from("2-29"), Some((2, 29)));
        assert_eq!(birthday_from(&birthday_text(Some((12, 31)))), Some((12, 31)));
        for bad in ["", "13-1", "2-30", "4-31", "0-5", "x-y", "3"] {
            assert_eq!(birthday_from(bad), None, "{bad}");
        }
    }
}
