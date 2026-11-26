//! The cast: every pet's head and torso as small pixel grids, plus how its legs, arms, tail or
//! wheels are attached. `rig.rs` draws the moving parts, `anim.rs` decides how they move.

use crate::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Build {
    Legs,
    Wheels,
    Hover,
}

pub(crate) struct Legs {
    pub hip: f32, // distance of each leg (or wheel) from the centre, in cells
    pub len: i32, // leg length, wheel diameter, or how high it hovers
    pub c: u8,
    pub foot: u8,
}

pub(crate) struct Arms {
    pub len: i32,
    pub c: u8,
    pub hand: u8,
    pub fat: f32, // thickness in cells
}

pub(crate) struct Tail {
    pub len: i32,
    pub c: u8,
    pub tip: u8,
}

/// Something worn on the back.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Back {
    None,
    Cape(u8),
    Wings(u8, u8),
    Disc, // a round shield
    Pack(u8),
    Swords,
    Limbs(u8, bool), // four arms more, made of metal or not
}

/// What holds somebody up who can fly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Aloft {
    Jets,
    Board,
    Fire,
    FireWings,
    Orbs,
    Ring,
}

/// Something held in the front hand; `grip` is the cell the hand holds.
pub(crate) struct Tool {
    pub grid: &'static [&'static [u8]],
    pub grip: (i32, i32),
}

pub(crate) struct Gear {
    pub back: Back,
    pub tool: Option<&'static Tool>,
    pub glow: u8, // colour of its powers
    pub gift: Gift, // how it gets about and saves itself
    pub aloft: Aloft,
    pub door: bool,             // goes through a portal when it warps
    pub disc: &'static [u8],    // the rings of its shield from the outside in, and the dot
    pub lights: &'static [u8],  // colours of what it throws or holds
    pub board: [u8; 3],         // what it stands on in the air: body, top, flame (0 = none)
}

const NO_GEAR: Gear = Gear { back: Back::None, tool: None, glow: b'Y', gift: Gift::None, aloft: Aloft::Jets, door: false, disc: &[], lights: &[], board: [b'V', b'i', 0] };

pub(crate) struct Pet {
    pub name: &'static str,
    pub eye: u8, // colour of the eyes
    pub face: u8, // fill colour behind the eyes, used when they move or close
    pub head: &'static [&'static [u8]],
    pub torso: &'static [&'static [u8]], // the head overlaps its first row
    pub build: Build,
    pub legs: Legs,
    pub arms: Option<Arms>,
    pub tail: Option<Tail>,
    pub gear: Gear,
    pub mouth: (f32, i32), // x from the centre, row in the head
    pub voice: usize,      // which set of lines it speaks (mood.rs)
    pub says: &'static [(Option<Ev>, &'static str)], // its own lines, said instead
    pub foods: [usize; 3],
    pub tricks: &'static [Clip], // its own acts, on top of COMMON
}

/// The biggest a standing pet may be, in cells; the windows are sized for this.
#[cfg(test)]
pub(crate) const PET_MAX_W: i32 = 22;
#[cfg(test)]
pub(crate) const PET_MAX_H: i32 = 20;

impl Pet {
    pub fn machine(&self) -> bool {
        self.voice == VOICE_MACHINE
    }

    /// Nominal size in cells, standing.
    pub fn w(&self) -> i32 {
        (self.head[0].len().max(self.torso.first().map_or(0, |t| t.len()))) as i32
    }

    pub fn h(&self) -> i32 {
        self.body_h() + self.lift()
    }

    /// Head and torso together.
    pub fn body_h(&self) -> i32 {
        self.head.len() as i32 + (self.torso.len() as i32 - 1).max(0)
    }

    /// How far the body is above the floor.
    pub fn lift(&self) -> i32 {
        match self.build {
            Build::Legs => self.legs.len + 1, // plus the outline under the feet
            Build::Wheels => self.legs.len / 2,
            Build::Hover => self.legs.len,
        }
    }
}

pub(crate) const VOICE_MACHINE: usize = 4;

/// The picker's tabs for these. Everybody else comes in a pack (pack.rs).
pub(crate) const OUR_TABS: &[(&str, &[usize])] = &[("CREATURES", &[0, 1, 2, 6, 7, 8, 11, 12]), ("PEOPLE", &[3, 9, 10, 14]), ("MACHINES", &[4, 5, 13, 15])];

pub(crate) const fn legs(hip: f32, len: i32, c: u8, foot: u8) -> Legs {
    Legs { hip, len, c, foot }
}

const fn arms(len: i32, c: u8, hand: u8) -> Option<Arms> {
    Some(Arms { len, c, hand, fat: 1.6 })
}

/// Everyone there is: the ones that come with the app, then the hero pack.
pub(crate) static OURS: [Pet; 16] = [
    Pet {
        name: "Cat",
        eye: b'K',
        face: b'O',
        head: &[
            b"..KK........KK..",
            b".KOOK......KOOK.",
            b".KOPOKKKKKKOPOK.",
            b".KOOOoOooOoOOOK.",
            b"KOOOOOOooOOOOOOK",
            b"KOOEEOOOOOOEEOOK",
            b"KOOEEOOOOOOEEOOK",
            b"KOZOOOWPPWOOOZOK",
            b"KOOOOWWKKWWOOOOK",
            b".KOOOOWWWWOOOOK.",
            b"..KKKKKKKKKKKK..",
        ],
        torso: &[b".KKKKKKKK.", b"KOOWWWWOOK", b"KOoWWWWoOK", b"KOOWWWWOOK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 2, b'O', b'W'),
        arms: arms(3, b'O', b'W'),
        tail: Some(Tail { len: 6, c: b'O', tip: b'o' }),
        gear: NO_GEAR,
        mouth: (0.0, 8),
        voice: 0,
        says: &[],
        foods: [FISH, MILK, MEAT],
        tricks: &[Clip::Groom, Clip::Pounce, Clip::Spin],
    },
    Pet {
        name: "Dog",
        eye: b'K',
        face: b'D',
        head: &[
            b"...KKKKKKKKKK...",
            b"..KDDDDDDDDDDK..",
            b".KdKDDDDDDDDKdK.",
            b"KddKDDDDDDDDKddK",
            b"KddKDEEDDEEDKddK",
            b"KddKDEEDDEEDKddK",
            b"KddKDDWWWWDDKddK",
            b".KdKDWKKKKWDKdK.",
            b"..KKWWWKKWWWKK..",
            b"...KWWKWWKWWK...",
            b"....KKKPPKKK....",
        ],
        torso: &[b".KKKKKKKK.", b"KRRRRRRRRK", b"KDDWYYWDDK", b"KDDWWWWDDK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 2, b'D', b'W'),
        arms: arms(3, b'D', b'W'),
        tail: Some(Tail { len: 4, c: b'D', tip: b'd' }),
        gear: NO_GEAR,
        mouth: (0.0, 9),
        voice: 1,
        says: &[],
        foods: [BONE, MEAT, COOKIE],
        tricks: &[Clip::Wag, Clip::Spin, Clip::Pounce],
    },
    Pet {
        name: "Mochi",
        eye: b'K',
        face: b'W',
        head: &[
            b"....KKKKKK....",
            b"..KKWWWWWWKK..",
            b".KWWWWWWWWWWK.",
            b".KWWWWWWWWWWK.",
            b"KWWWEEWWEEWWWK",
            b"KWWWEEWWEEWWWK",
            b"KWZZWWWWWWZZWK",
            b"KWWWWWPPWWWWWK",
            b"KWWWWWWWWWWWWK",
            b".KWWWWWWWWWWK.",
            b"..KKKKKKKKKK..",
        ],
        torso: &[],
        build: Build::Legs,
        legs: legs(2.5, 1, b'P', b'P'),
        arms: arms(2, b'W', b'P'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 7),
        voice: 2,
        says: &[],
        foods: [COOKIE, TEA, CAKE],
        tricks: &[Clip::Jiggle, Clip::Hops],
    },
    Pet {
        name: "Agent",
        eye: b'K',
        face: b'S',
        head: &[
            b"....KKKKKKKK....",
            b"...KHHHHHHHHK...",
            b"..KHHhhHHHHHHK..",
            b"..KHHHHHHHHHHK..",
            b"..KHSSSSSSSSHK..",
            b"..KSSSSSSSSSSK..",
            b"..KBGBBKKBGBBK..",
            b"..KSBBBSSBBBSK..",
            b"..KSSSSsSSSSSK..",
            b"..KSSSSSSKKSSK..",
            b"...KsSSSSSSsK...",
            b"....KKKKKKKK....",
        ],
        torso: &[b".KKKKKKKK.", b"KTTWRRWTTK", b"KTtWRrWtTK", b"KTTtRrtTTK", b"KTTTTTTTTK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 2, b'T', b'B'),
        arms: arms(3, b'T', b'S'),
        tail: None,
        gear: NO_GEAR,
        mouth: (1.5, 9),
        voice: 3,
        says: &[],
        foods: [PIZZA, COFFEE, BURGER],
        tricks: &[Clip::Phone, Clip::Sip, Clip::Backflip],
    },
    Pet {
        name: "Super Car",
        eye: b'K',
        face: b'G',
        head: &[
            b".......KKKKKKKK.......",
            b".....KKRRRRRRRRKK.....",
            b"....KRGGGGKGGGGGRK....",
            b"KK.KRRGEEGKGGEEGRRKK..",
            b"KRKKRRGEEGKGGEEGRRRRK.",
            b"KRRRRRRRRRRRRRRRRRRRRK",
            b"KrRRRRRRRRRRRRRRRRRRYK",
            b"KrWWWWWWWWWWWWWWWWRRYK",
            b"KrRRRRRRRRRRRRRRRRRRRK",
            b".KKKKKKKKKKKKKKKKKKKK.",
        ],
        torso: &[],
        build: Build::Wheels,
        legs: legs(6.5, 5, b'N', b'V'),
        arms: None,
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 7),
        voice: VOICE_MACHINE,
        says: &[],
        foods: [FUEL, OIL, NITRO],
        tricks: &[Clip::Rev, Clip::Drift, Clip::Honk],
    },
    Pet {
        name: "Robot",
        eye: b'K',
        face: b'q',
        head: &[
            b".......KK.......",
            b"......KRRK......",
            b".......KK.......",
            b"..KKKKKKKKKKKK..",
            b".KmmmmmmmmmmmmK.",
            b"KKmKKKKKKKKKKmKK",
            b"KnmKqEEqqEEqKmnK",
            b"KnmKqEEqqEEqKmnK",
            b"KnmKqqqqqqqqKmnK",
            b"KKmKqqKKKKqqKmKK",
            b".KmKKKKKKKKKKmK.",
            b"..KKKKKKKKKKKK..",
        ],
        torso: &[b".KKKKKKKK.", b"KmmmmmmmmK", b"KmRYgmnnmK", b"KmmmmmnnmK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 3, b'n', b'm'),
        arms: arms(3, b'n', b'V'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 9),
        voice: VOICE_MACHINE,
        says: &[],
        foods: [BATTERY, BOLT, OIL],
        tricks: &[Clip::Glitch, Clip::Backflip],
    },
    Pet {
        name: "Penguin",
        eye: b'K',
        face: b'i',
        head: &[
            b"....KKKKKKKK....",
            b"..KKuuuuuuuuKK..",
            b".KuuuuuuuuuuuuK.",
            b".KuiiiuuuuiiiuK.",
            b"KuiiiiiuuiiiiiuK",
            b"KuiEEiiiiiiEEiuK",
            b"KuiEEiiyyiiEEiuK",
            b"KuZiiiyyyyiiiZuK",
            b"KuuiiiiyyiiiiuuK",
            b".KuuiiiiiiiiuuK.",
            b"..KKKKKKKKKKKK..",
        ],
        torso: &[b"..KKKKKKKK..", b".KuiiiiiiuK.", b"KuiiiiiiiiuK", b"KuiiiiiiiiuK", b".KKKKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 1, b'y', b'y'),
        arms: arms(4, b'u', b'u'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 8),
        voice: 1,
        says: &[],
        foods: [FISH, ICE_POP, CAKE],
        tricks: &[Clip::Slide, Clip::Dance],
    },
    Pet {
        name: "Dino",
        eye: b'K',
        face: b'g',
        head: &[
            b"....KOK.KOK.....",
            b"...KKOKKKOKKK...",
            b"..KggggggggggK..",
            b".KggjggggggjggK.",
            b".KggggggggggggK.",
            b"KggEEggggggEEggK",
            b"KggEEggggggEEggK",
            b"KgZgggjggjgggZgK",
            b"KggKKKKKKKKKKggK",
            b".KggKiKiKiKKggK.",
            b"..KKgggggggKKK..",
            b"....KKKKKKKK....",
        ],
        torso: &[b".KKKKKKKK.", b"KggWWWWggK", b"KgjWWWWjgK", b"KggWWWWggK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 2, b'g', b'j'),
        arms: arms(2, b'g', b'j'),
        tail: Some(Tail { len: 6, c: b'g', tip: b'j' }),
        gear: NO_GEAR,
        mouth: (0.0, 8),
        voice: 1,
        says: &[],
        foods: [MEAT, APPLE, FISH],
        tricks: &[Clip::Roar, Clip::Pounce],
    },
    Pet {
        name: "Ghost",
        eye: b'K',
        face: b'w',
        head: &[
            b"....KKKKKK....",
            b"..KKwwwwwwKK..",
            b".KwwwwwwwwwwK.",
            b".KwwwwwwwwwwK.",
            b"KwwwEEwwEEwwwK",
            b"KwwwEEwwEEwwwK",
            b"KwZZwwwwwwZZwK",
            b"KwwwwwKKwwwwwK",
            b"KwwwwwwwwwwwwK",
            b"KwwwwwwwwwwwwK",
            b"KwwwwwwwwwwwwK",
        ],
        torso: &[],
        build: Build::Hover,
        legs: legs(0.0, 4, b'w', b'K'),
        arms: arms(2, b'w', b'w'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 7),
        voice: 0,
        says: &[],
        foods: [CANDY, CAKE, COOKIE],
        tricks: &[Clip::Spook, Clip::Loop],
    },
    Pet {
        name: "Ninja",
        eye: b'K',
        face: b'S',
        head: &[
            b"....KKKKKKKK....",
            b"..KKxxxxxxxxKK..",
            b".KxxxxxxxxxxxxK.",
            b".KRRRRRRRRRRRRK.",
            b"KRRRRRRRRRRRRRRK",
            b"KxKSSSSSSSSSSKxK",
            b"KxSSEESSSSEESSxK",
            b"KxSSEESSSSEESSxK",
            b"KxxxxxxxxxxxxxxK",
            b".KxxxxxxxxxxxxK.",
            b"..KKKKKKKKKKKK..",
        ],
        torso: &[b".KKKKKKKK.", b"KxxxKKxxxK", b"KxxxxKxxxK", b"KRRRRRRRRK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 3, b'x', b'B'),
        arms: arms(3, b'x', b'S'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 8),
        voice: 3,
        says: &[],
        foods: [RAMEN, TEA, FISH],
        tricks: &[Clip::Vanish, Clip::Backflip],
    },
    Pet {
        name: "Astronaut",
        eye: b'K',
        face: b'S',
        head: &[
            b"....KKKKKKKK....",
            b"..KKiiiiiiiiKK..",
            b".KiiKKKKKKKKiiK.",
            b"KiiKHHHHHHHHKiiK",
            b"KiKHHSSSSSSHHKiK",
            b"KRKSSEESSEESSKRK",
            b"KRKSSEESSEESSKRK",
            b"KiKSZSSSSSSZSKiK",
            b"KiiKSSSKKSSSKiiK",
            b".KiiKKKKKKKKiiK.",
            b"..KKiiiiiiiiKK..",
            b"....KKKKKKKK....",
        ],
        torso: &[b".KKKKKKKK.", b"KiiiiiiiiK", b"KiVRVbVViK", b"KiiiiiiiiK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 2, b'i', b'V'),
        arms: arms(3, b'i', b'R'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 8),
        voice: 3,
        says: &[],
        foods: [PIZZA, ICE_POP, APPLE],
        tricks: &[Clip::Float, Clip::Loop],
    },
    Pet {
        name: "Bunny",
        eye: b'K',
        face: b'i',
        head: &[
            b"..KKK......KKK..",
            b".KiPiK....KiPiK.",
            b".KiPiK....KiPiK.",
            b".KiPiKKKKKKiPiK.",
            b"..KiiiiiiiiiiK..",
            b".KiiiiiiiiiiiiK.",
            b"KiiEEiiiiiiEEiiK",
            b"KiiEEiiiiiiEEiiK",
            b"KiZiiiiPPiiiiZiK",
            b"KiiiiiKiiKiiiiiK",
            b".KiiiiiKKiiiiiK.",
            b"..KKKKKKKKKKKK..",
        ],
        torso: &[b".KKKKKKKK.", b"KiiVVVViiK", b"KiiVVVViiK", b"KiiiVViiiK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 1, b'i', b'P'),
        arms: arms(3, b'i', b'i'),
        tail: Some(Tail { len: 2, c: b'i', tip: b'i' }),
        gear: NO_GEAR,
        mouth: (0.0, 9),
        voice: 2,
        says: &[],
        foods: [CARROT, APPLE, CAKE],
        tricks: &[Clip::Hops, Clip::Groom],
    },
    Pet {
        name: "Frog",
        eye: b'K',
        face: b'i',
        head: &[
            b"..KKKK....KKKK..",
            b".KiiiiK..KiiiiK.",
            b".KiEEiKKKKiEEiK.",
            b".KiEEiggggiEEiK.",
            b"..KgggggggggggK.",
            b".KggggjggjggggK.",
            b"KggggggggggggggK",
            b"KgZKggggggggKZgK",
            b"KgggKKKKKKKKgggK",
            b".KggggggggggggK.",
            b"..KKKKKKKKKKKK..",
        ],
        torso: &[b".KKKKKKKK.", b"KggYYYYggK", b"KgjYYYYjgK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(3.0, 2, b'g', b'j'),
        arms: arms(3, b'g', b'j'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 8),
        voice: 0,
        says: &[],
        foods: [FLY, APPLE, CANDY],
        tricks: &[Clip::Tongue, Clip::Hops],
    },
    Pet {
        name: "UFO",
        eye: b'K',
        face: b'g',
        head: &[
            b"......KKKKKKKK......",
            b"....KKccccccccKK....",
            b"...KcccKggggKcccK...",
            b"...KccKggggggKccK...",
            b"...KccKgEggEgKccK...",
            b"...KccKgEggEgKccK...",
            b"..KKccKggKKggKccKK..",
            b".KVVKKKKKKKKKKKKVVK.",
            b"KVVVVVVVVVVVVVVVVVVK",
            b"KnYnnRnnYnnRnnYnnRnK",
            b".KKnnnnnnnnnnnnnnKK.",
            b"...KKKKKKKKKKKKKK...",
        ],
        torso: &[],
        build: Build::Hover,
        legs: legs(0.0, 5, b'q', b'q'),
        arms: None,
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 6),
        voice: VOICE_MACHINE,
        says: &[],
        foods: [PLASMA, CANDY, BATTERY],
        tricks: &[Clip::Loop, Clip::Beam, Clip::Glitch],
    },
    Pet {
        name: "Coder",
        eye: b'K',
        face: b'S',
        head: &[
            b"....KKKKKKKK....",
            b"..KKHHHHHHHHKK..",
            b".KHHHHhhHHHHHHK.",
            b".KKKKKKKKKKKKKK.",
            b"KKKHHSHHHSSSHKKK",
            b"KRKSSSSSSSSSSKRK",
            b"KRKSEESSSSEESKRK",
            b"KRKSEESSSSEESKRK",
            b"KKKZSSSSSSSSZKKK",
            b"..KSSSSKKSSSSK..",
            b"...KSSSSSSSSK...",
            b"....KKKKKKKK....",
        ],
        torso: &[b".KKKKKKKK.", b"KvvvKKvvvK", b"KvvWvvWvvK", b"KvvvvvvvvK", b"KvvKKKKvvK", b".KKKKKKKK."],
        build: Build::Legs,
        legs: legs(2.5, 2, b'T', b'i'),
        arms: arms(3, b'v', b'S'),
        tail: None,
        gear: NO_GEAR,
        mouth: (0.0, 9),
        voice: 3,
        says: &[],
        foods: [COFFEE, RAMEN, PIZZA],
        tricks: &[Clip::Sip, Clip::Phone, Clip::Dance],
    },
    Pet {
        name: "Truck",
        eye: b'K',
        face: b'G',
        head: &[
            b"..........KKKKKKKKK...",
            b".........KbbbbbbbbbK..",
            b".........KbGGGGGGGbK..",
            b"KKKKKKKKKKbGEEGEEGbK..",
            b"KuKbbKbbKbbGEEGEEGbKKK",
            b"KuKbbKbbKbbGGGGGGGbbbK",
            b"KubbbbbbbbbbbbbbbbbbYK",
            b"KubbbbbbbbWWbbbbbbbbYK",
            b"KVVVVVVVVVVVVVVVVVVVVK",
            b".KKKKKKKKKKKKKKKKKKKK.",
        ],
        torso: &[],
        build: Build::Wheels,
        legs: legs(6.5, 7, b'N', b'Y'),
        arms: None,
        tail: None,
        gear: NO_GEAR,
        mouth: (3.0, 5),
        voice: VOICE_MACHINE,
        says: &[],
        foods: [FUEL, OIL, NITRO],
        tricks: &[Clip::Wheelie, Clip::Rev, Clip::Honk],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(name: &str, grid: &[&[u8]]) {
        for (j, row) in grid.iter().enumerate() {
            assert_eq!(row.len(), grid[0].len(), "{name} row {j} has a different width");
            for &ch in row.iter() {
                assert!(ch == b'.' || pet_color(ch).is_some(), "{name} uses unknown colour {}", ch as char);
            }
        }
    }

    #[test]
    fn every_pet_is_clean_fits_and_only_uses_known_colours() {
        for p in PETS.iter() {
            clean(p.name, p.head);
            clean(p.name, p.torso);
            assert!(p.w() <= PET_MAX_W && p.h() <= PET_MAX_H, "{} is {}x{}", p.name, p.w(), p.h());
            let parts = [Some(p.face), Some(p.legs.c), Some(p.legs.foot), p.arms.as_ref().map(|a| a.c), p.arms.as_ref().map(|a| a.hand), p.tail.as_ref().map(|t| t.c), p.tail.as_ref().map(|t| t.tip)];
            for c in parts.into_iter().flatten() {
                assert!(pet_color(c).is_some(), "{} uses unknown colour {}", p.name, c as char);
            }
            assert!((p.mouth.1 as usize) < p.head.len(), "{}", p.name);
            assert!(!p.tricks.is_empty(), "{} has no tricks of its own", p.name);
        }
    }

    #[test]
    fn names_are_unique_and_fit_the_picker() {
        for (i, p) in PETS.iter().enumerate() {
            assert!(p.name.len() <= 12, "{}", p.name);
            assert!(PETS.iter().take(i).all(|q| q.name != p.name), "{}", p.name);
        }
    }

    #[test]
    fn every_pet_is_on_exactly_one_tab() {
        for i in 0..PETS.len() {
            let n = pack::tabs().iter().filter(|t| t.pets.contains(&i)).count();
            assert_eq!(n, 1, "{} is on {n} tabs", PETS[i].name);
        }
    }

    #[test]
    fn saved_choices_keep_meaning_the_same_pet() {
        // the settings file stores a number for these, so their order must stay
        let names: Vec<_> = PETS.iter().take(16).map(|p| p.name).collect();
        assert_eq!(names, ["Cat", "Dog", "Mochi", "Agent", "Super Car", "Robot", "Penguin", "Dino", "Ghost", "Ninja", "Astronaut", "Bunny", "Frog", "UFO", "Coder", "Truck"]);
    }
}
