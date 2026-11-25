//! Things to feed the pets: food for the living, fuel and spare parts for the machines.

pub(crate) struct Food {
    pub name: &'static str,
    pub gain: f32,  // how filling it is, 0..100
    pub joy: f32,   // how much happier it makes the pet
    pub boost: f32, // seconds of running around afterwards
    pub crumb: u8,  // colour of the crumbs
    pub grid: &'static [&'static [u8]],
}

pub(crate) const FISH: usize = 0;
pub(crate) const MILK: usize = 1;
pub(crate) const BONE: usize = 2;
pub(crate) const MEAT: usize = 3;
pub(crate) const COOKIE: usize = 4;
pub(crate) const TEA: usize = 5;
pub(crate) const PIZZA: usize = 6;
pub(crate) const BURGER: usize = 7;
pub(crate) const COFFEE: usize = 8;
pub(crate) const FUEL: usize = 9;
pub(crate) const OIL: usize = 10;
pub(crate) const NITRO: usize = 11;
pub(crate) const BATTERY: usize = 12;
pub(crate) const BOLT: usize = 13;
pub(crate) const ICE_POP: usize = 14;
pub(crate) const CANDY: usize = 15;
pub(crate) const CARROT: usize = 16;
pub(crate) const APPLE: usize = 17;
pub(crate) const FLY: usize = 18;
pub(crate) const PLASMA: usize = 19;
pub(crate) const RAMEN: usize = 20;
pub(crate) const CAKE: usize = 21;

const fn food(name: &'static str, gain: f32, joy: f32, boost: f32, crumb: u8, grid: &'static [&'static [u8]]) -> Food {
    Food { name, gain, joy, boost, crumb, grid }
}

pub(crate) static FOODS: [Food; 25] = [
    food("FISH", 30.0, 6.0, 0.0, b'c', &[b"....KKKK.", b"KK.KbbbbK", b"KbKbbbbEK", b"KbbbcccbK", b"KbKcccccK", b"KK.KccccK", b"....KKKK."]),
    food("MILK", 18.0, 5.0, 0.0, b'i', &[b".KKKK.", b".KiiK.", b"KiiiiK", b"KbbbbK", b"KiiiiK", b"KiiiiK", b"KKKKKK"]),
    food("BONE", 16.0, 8.0, 0.0, b'W', &[b".K....K.", b"KWK..KWK", b"KWWKKWWK", b".KWWWWK.", b"KWWKKWWK", b"KWK..KWK", b".K....K."]),
    food("MEAT", 40.0, 7.0, 0.0, b'd', &[b"..KKKK..", b".KDDDDK.", b"KDdDDDDK", b"KDDDDDDK", b".KDDDDKK", b"..KKKWWK", b".....KWK", b"......K."]),
    food("COOKIE", 14.0, 8.0, 0.0, b'o', &[b"..KKK..", b".KoooK.", b"KodoooK", b"KoooodK", b"KodoooK", b".KoodK.", b"..KKK.."]),
    food("TEA", 8.0, 7.0, 0.0, b'g', &[b"..V.V...", b".V.V....", b"KKKKKKK.", b"KgggggKK", b"KiiiiiKK", b"KiiiiiK.", b".KKKKK.."]),
    food("PIZZA", 34.0, 8.0, 0.0, b'Y', &[b"KKKKKKKK", b"KooooooK", b"KYYRYYYK", b".KYYYYK.", b".KYRYYK.", b"..KYYK..", b"..KYYK..", b"...KK..."]),
    food("BURGER", 42.0, 7.0, 0.0, b'o', &[b".KKKKKK.", b"KoooYooK", b"KoYooooK", b"KggggggK", b"KddddddK", b"KYYYYYYK", b"KooooooK", b".KKKKKK."]),
    food("COFFEE", 6.0, 4.0, 20.0, b'd', &[b"..V.V..", b".KKKKK.", b"KiiiiiK", b".KoooK.", b".KoooK.", b".KoooK.", b".KKKKK."]),
    food("FUEL", 45.0, 6.0, 0.0, b'Y', &[b".KKKK.KK", b"KRRRRKRK", b"KRRRRRRK", b"KRYRRYRK", b"KRRYYRRK", b"KRYRRYRK", b"KRRRRRRK", b"KKKKKKKK"]),
    food("OIL", 18.0, 8.0, 0.0, b'B', &[b"..KKK...", b"..KYK...", b".KKKKKK.", b"KNNNNNNK", b"KNYYYYNK", b"KNYBBYNK", b"KNYYYYNK", b"KKKKKKKK"]),
    food("NITRO", 12.0, 8.0, 14.0, b'q', &[b"..KKK.", b".KVVVK", b".KKKK.", b"KbbbbK", b"KqbbqK", b"KbqqbK", b"KbbbbK", b".KKKK."]),
    food("BATTERY", 45.0, 5.0, 0.0, b'q', &[b"..KKK..", b".KKKKK.", b"KVVVVVK", b"KqqqqqK", b"KqqqqqK", b"KgggggK", b"KgggggK", b"KKKKKKK"]),
    food("BOLT", 14.0, 8.0, 0.0, b'V', &[b".KKKKKK.", b"KVVVVVVK", b".KKVVKK.", b"..KnnK..", b"..KVVK..", b"..KnnK..", b"..KVVK..", b"...KK..."]),
    food("ICE POP", 14.0, 9.0, 0.0, b'c', &[b".KKKK.", b"KccccK", b"KciccK", b"KciccK", b"KppppK", b"KppppK", b".KKKK.", b"..KWK."]),
    food("CANDY", 10.0, 9.0, 6.0, b'p', &[b"KK....KK", b"KpKKKKpK", b".KpiipK.", b".KipPiK.", b".KpiipK.", b"KpKKKKpK", b"KK....KK"]),
    food("CARROT", 28.0, 7.0, 0.0, b'O', &[b".gK.gK..", b"..gKg...", b".KKgKK..", b".KOOOK..", b".KOoOK..", b"..KOOK..", b"..KoK...", b"...K...."]),
    food("APPLE", 22.0, 6.0, 0.0, b'R', &[b"...KgK.", b".KKKKK.", b"KRRRRRK", b"KRiRRRK", b"KRRRRRK", b"KrRRRrK", b".KrKrK.", b"..K.K.."]),
    food("FLY", 24.0, 8.0, 0.0, b'n', &[b".cc.cc.", b"ccc.ccc", b".ccKcc.", b"..KKK..", b".KRKRK.", b"..KKK..", b"..K.K.."]),
    food("PLASMA", 40.0, 7.0, 0.0, b'q', &[b"...KK...", b"..KqqK..", b"KKqiiqKK", b"KqiiiiqK", b"KKqiiqKK", b"..KqqK..", b"...KK..."]),
    food("RAMEN", 38.0, 8.0, 0.0, b'Y', &[b".....K.K", b"....K.K.", b"KKKKKKKK", b"KYWYWYgK", b"KRRRRRRK", b".KRiiRK.", b"..KKKK.."]),
    food("CAKE", 24.0, 12.0, 0.0, b'p', &[b"...KR...", b"..KRRK..", b".KKKKKK.", b"KiiiiiiK", b"KppppppK", b"KooooooK", b"KppppppK", b"KKKKKKKK"]),
    food("DONUT", 16.0, 9.0, 4.0, b'p', &[b"..KKKK..", b".KppppK.", b"KpipppiK", b"KppKKppK", b"KppKKipK", b"KooppooK", b".KooooK.", b"..KKKK.."]),
    food("SHAWARMA", 44.0, 9.0, 0.0, b'D', &[b"..KKKK..", b".KgRgRK.", b"KWDDDDWK", b"KWWWWWWK", b".KWoWWK.", b".KWWoWK.", b"..KWWK..", b"...KK..."]),
    food("WATER", 22.0, 6.0, 0.0, b'c', &[b"...KK...", b"..KccK..", b"..KccK..", b".KciccK.", b"KciccccK", b"KccccccK", b"KbccccbK", b".KKKKKK."]),
];

/// The food a pack means: "ice-pop" is the ICE POP.
pub(crate) fn food_named(name: &str) -> Option<usize> {
    FOODS.iter().position(|f| f.name.to_lowercase().replace(' ', "-") == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn every_food_is_a_clean_little_sprite_that_does_something() {
        for f in FOODS.iter() {
            let w = f.grid[0].len();
            assert!(w <= 9 && f.grid.len() <= 8, "{} is {}x{}", f.name, w, f.grid.len());
            for row in f.grid {
                assert_eq!(row.len(), w, "{}", f.name);
                for &c in row.iter() {
                    assert!(c == b'.' || pet_color(c).is_some(), "{} uses unknown colour {}", f.name, c as char);
                }
            }
            assert!(f.gain > 0.0 && f.gain <= 100.0 && f.joy > 0.0, "{}", f.name);
            assert!(pet_color(f.crumb).is_some(), "{}", f.name);
            assert!(f.name.len() <= 8, "{} does not fit its tile", f.name);
        }
    }

    #[test]
    fn every_pet_has_three_different_things_to_eat() {
        for p in PETS.iter() {
            let [a, b, c] = p.foods;
            assert!(a != b && b != c && a != c, "{}", p.name);
            assert!(p.foods.iter().all(|f| *f < FOODS.len()), "{}", p.name);
        }
    }

    #[test]
    fn machines_run_on_fuel_and_people_eat_food() {
        let by = |n: &str| PETS.iter().find(|p| p.name == n).unwrap();
        assert!(by("Super Car").foods.contains(&FUEL) && by("Truck").foods.contains(&FUEL));
        assert!(by("Agent").foods.contains(&PIZZA) && !by("Agent").foods.contains(&FUEL));
    }
}
