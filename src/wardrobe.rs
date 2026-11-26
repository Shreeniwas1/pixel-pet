//! Levels and hats: looking after the pet earns experience, and levels unlock things to wear.

/// What earns experience, and how much.
pub(crate) const XP_PET: u32 = 2;
pub(crate) const XP_FED: u32 = 6;
pub(crate) const XP_BALL: u32 = 4;
pub(crate) const XP_TASK: u32 = 10;
pub(crate) const XP_FOCUS: u32 = 30;
pub(crate) const XP_CODE: u32 = 5;

/// Experience needed to reach a level: 0, 20, 60, 120, 200 ...
pub(crate) fn needed(level: u32) -> u32 {
    let n = level.saturating_sub(1);
    10 * n * (n + 1)
}

pub(crate) fn level(xp: u32) -> u32 {
    let mut l = 1;
    while l < 99 && xp >= needed(l + 1) {
        l += 1;
    }
    l
}

/// How far along the current level, 0..1.
pub(crate) fn progress(xp: u32) -> f32 {
    let l = level(xp);
    let (a, b) = (needed(l), needed(l + 1));
    ((xp.saturating_sub(a)) as f32 / (b - a).max(1) as f32).min(1.0)
}

pub(crate) struct Hat {
    pub name: &'static str,
    pub level: u32, // unlocked at
    pub sink: i32,  // how many rows it comes down over the head
    pub grid: &'static [&'static [u8]],
}

const fn hat(name: &'static str, level: u32, sink: i32, grid: &'static [&'static [u8]]) -> Hat {
    Hat { name, level, sink, grid }
}

pub(crate) static HATS: [Hat; 12] = [
    hat("NONE", 1, 0, &[]),
    hat("BOW", 1, 1, &[b"KK..KK", b"KpKKpK", b"KppppK", b"KpKKpK", b"KK..KK"]),
    hat("CAP", 2, 1, &[b"..KKKKK...", b".KRRRRRK..", b"KRRRRRRRKK", b"KKKKKKKKKK"]),
    hat("PARTY", 3, 1, &[b"...KK...", b"..KYYK..", b"..KppK..", b".KbYYbK.", b".KppppK.", b"KKKKKKKK"]),
    hat("FLOWER", 4, 1, &[b".KKKK.", b"KpYYpK", b"KYyyYK", b"KpYYpK", b".KKKK."]),
    hat("BEANIE", 5, 2, &[b"....KK....", b"...KiiK...", b"..KKKKKK..", b".KbbubbuK.", b"KbbubbubbK", b"KiiiiiiiiK", b"KKKKKKKKKK"]),
    hat("TOP HAT", 6, 1, &[b"..KKKKKK..", b"..KBBBBK..", b"..KBBBBK..", b"..KRRRRK..", b"KKKBBBBKKK", b"KKKKKKKKKK"]),
    hat("COWBOY", 8, 1, &[b"...KKKKKK...", b"..KddKKddK..", b"..KddddddK..", b"KKKDDDDDDKKK", b"KddddddddddK", b".KKKKKKKKKK."]),
    hat("WIZARD", 10, 1, &[b".....KK.....", b"....KvvK....", b"...KvYvvK...", b"..KvvvvvvK..", b".KvvYvvYvvK.", b"KKKKKKKKKKKK"]),
    hat("VIKING", 12, 2, &[b"KK......KK", b"KWK....KWK", b"KWWKKKKWWK", b".KKVVVVKK.", b".KVVVVVVK.", b"KKKKKKKKKK"]),
    hat("CROWN", 15, 1, &[b"K..KK..K", b"KKKYYKKK", b"KYYRRYYK", b"KYbYYbYK", b"KKKKKKKK"]),
    hat("HALO", 20, -2, &[b"..KKKKKK..", b".KYYYYYYK.", b"KYKKKKKKYK", b".KYYYYYYK.", b"..KKKKKK.."]),
];

/// Hats for days and nights, not chosen in the wardrobe: `EXTRA[i]` is hat `FIRST_EXTRA + i`.
pub(crate) static EXTRA: [Hat; 2] = [
    hat("NIGHTCAP", 0, 2, &[b"........KKK.", b".......KiiiK", b"......KbKiK.", b".....KbbbKK.", b"...KKbbbbK..", b"..KbbbbbbbK.", b".KiiiiiiiiiK", b".KKKKKKKKKKK"]),
    hat("WITCH", 0, 1, &[b".....KK.....", b"....KxxK....", b"....KxxxK...", b"...KxxxxK...", b"...KxxxxxK..", b"..KAAAAAAAK.", b"KKxxxxxxxxxK", b"KKKKKKKKKKKK"]),
];
pub(crate) const FIRST_EXTRA: usize = 100;
pub(crate) const PARTY: usize = 3;
pub(crate) const NIGHTCAP: usize = FIRST_EXTRA;
pub(crate) const WITCH: usize = FIRST_EXTRA + 1;

/// Any hat by its number, the wardrobe's and the others.
pub(crate) fn of(i: usize) -> Option<&'static Hat> {
    if i >= FIRST_EXTRA { EXTRA.get(i - FIRST_EXTRA) } else { HATS.get(i) }
}

/// The hat actually worn: one that is not unlocked (yet) is not worn.
pub(crate) fn worn(hat: usize, xp: u32) -> usize {
    if hat < HATS.len() && HATS[hat].level <= level(xp) {
        hat
    } else {
        0
    }
}

/// Hats that came with going from one amount of experience to another.
pub(crate) fn unlocked(before: u32, after: u32) -> Option<&'static str> {
    let (a, b) = (level(before), level(after));
    HATS.iter().skip(1).find(|h| h.level > a && h.level <= b).map(|h| h.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn levels_take_longer_and_longer() {
        assert_eq!((level(0), level(19), level(20), level(59), level(60)), (1, 1, 2, 2, 3));
        for l in 1..60 {
            assert_eq!(level(needed(l)), l);
            assert!(needed(l + 2) - needed(l + 1) > needed(l + 1) - needed(l) || l == 1);
        }
        assert_eq!(level(u32::MAX), 99);
    }

    #[test]
    fn progress_runs_from_nothing_to_full_within_a_level() {
        assert_eq!(progress(20), 0.0);
        assert!((progress(40) - 0.5).abs() < 0.01);
        for xp in 0..5000 {
            assert!((0.0..=1.0).contains(&progress(xp)));
        }
    }

    #[test]
    fn hats_are_clean_sprites_in_order_of_level() {
        for (i, h) in HATS.iter().enumerate() {
            assert!(h.name.len() <= 7, "{}", h.name);
            assert!(i == 0 || h.level >= HATS[i - 1].level, "{}", h.name);
            for row in h.grid {
                assert_eq!(row.len(), h.grid[0].len(), "{}", h.name);
                assert!(row.iter().all(|c| *c == b'.' || pet_color(*c).is_some()), "{}", h.name);
            }
        }
    }

    #[test]
    fn the_other_hats_are_clean_sprites_too() {
        assert_eq!(HATS[PARTY].name, "PARTY");
        assert_eq!(of(NIGHTCAP).map(|h| h.name), Some("NIGHTCAP"));
        assert_eq!(of(WITCH).map(|h| h.name), Some("WITCH"));
        assert!(of(FIRST_EXTRA + EXTRA.len()).is_none() && of(HATS.len()).is_none());
        for h in &EXTRA {
            for row in h.grid {
                assert_eq!(row.len(), h.grid[0].len(), "{}", h.name);
                assert!(row.iter().all(|c| *c == b'.' || pet_color(*c).is_some()), "{}", h.name);
            }
        }
    }

    #[test]
    fn a_hat_is_worn_only_once_unlocked() {
        assert_eq!(worn(10, 0), 0);
        assert_eq!(worn(10, needed(15)), 10);
        assert_eq!(worn(99, u32::MAX), 0);
        assert_eq!(unlocked(needed(3) - 1, needed(3)), Some("PARTY"));
        assert_eq!(unlocked(needed(6), needed(6) + 1), None);
    }
}
