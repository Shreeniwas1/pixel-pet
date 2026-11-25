//! The gallery: packs to have with one click, from the `packs` folder of the project on GitHub. It is
//! asked when its page is opened, and not otherwise.

use crate::prelude::*;
use std::sync::atomic::AtomicBool;

/// Sent to the pet when a pack has come.
pub(crate) const WM_GOT: u32 = WM_APP + 7;
pub(crate) const MOST: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Offer {
    pub id: String,
    pub name: String,
    pub count: u32,
    pub file: String,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Shelf {
    Closed,
    Looking,
    Failed(String),
    Open(Vec<Offer>),
}

static SHELF: Mutex<Shelf> = Mutex::new(Shelf::Closed);
/// The pack on its way.
static COMING: Mutex<Option<String>> = Mutex::new(None);
static GOT: Mutex<Option<Result<Vec<u8>, String>>> = Mutex::new(None);
static BUSY: AtomicBool = AtomicBool::new(false);

fn base() -> String {
    net::base("PIXEL_PET_GALLERY", "https://raw.githubusercontent.com/Shreeniwas1/pixel-pet/main/packs")
}

/// A file name, and nothing that leads anywhere else.
fn is_file(f: &str) -> bool {
    f.strip_suffix(".petpack").is_some_and(|n| (1..=24).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
}

pub(crate) fn offers(list: &str) -> Option<Vec<Offer>> {
    let j = Json::parse(list)?;
    Some(
        j.get("packs")?
            .list()
            .iter()
            .filter_map(|p| {
                let (id, file) = (p.text("id"), p.text("file"));
                let name: String = p.text("name").chars().filter(|c| !c.is_control()).take(24).collect();
                let count = p.get("count").and_then(Json::num).unwrap_or(0.0).clamp(0.0, 128.0) as u32;
                (is_file(file) && file.strip_suffix(".petpack") == Some(id) && !name.trim().is_empty()).then(|| Offer { id: id.into(), name, count, file: file.into() })
            })
            .take(MOST)
            .collect(),
    )
}

/// The page was opened.
pub(crate) fn open() {
    if BUSY.swap(true, Relaxed) {
        return;
    }
    *SHELF.lock().unwrap() = Shelf::Looking;
    std::thread::spawn(|| {
        let got = net::ask(&net::Ask { url: format!("{}/index.json", base()), most: 200_000, secs: 25, ..Default::default() });
        *SHELF.lock().unwrap() = match got {
            Ok(a) if a.code == 200 => offers(&a.text()).map_or(Shelf::Failed("I do not understand the list.".into()), Shelf::Open),
            Ok(_) => Shelf::Failed("The gallery is not open yet.".into()),
            Err(e) => Shelf::Failed(e),
        };
        BUSY.store(false, Relaxed);
    });
}

pub(crate) fn shelf() -> Shelf {
    SHELF.lock().unwrap().clone()
}

pub(crate) fn coming() -> Option<String> {
    COMING.lock().unwrap().clone()
}

/// Fetch a pack; the pet gets `WM_GOT` when it is there.
pub(crate) fn get(hwnd: HWND, i: usize) {
    let Shelf::Open(l) = shelf() else { return };
    let Some(o) = l.get(i).cloned() else { return };
    {
        let mut c = COMING.lock().unwrap();
        if c.is_some() {
            return;
        }
        *c = Some(o.id.clone());
    }
    let to = hwnd as isize;
    std::thread::spawn(move || {
        let got = net::ask(&net::Ask { url: format!("{}/{}", base(), o.file), most: 4 << 20, secs: 60, ..Default::default() });
        *GOT.lock().unwrap() = Some(match got {
            Ok(a) if a.code == 200 => Ok(a.body),
            Ok(_) => Err("That pack is not there any more.".into()),
            Err(e) => Err(e),
        });
        *COMING.lock().unwrap() = None;
        unsafe { PostMessageW(to as HWND, WM_GOT, 0, 0) };
    });
}

/// What has come.
pub(crate) fn got() -> Option<Result<Vec<u8>, String>> {
    GOT.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_of_the_project_is_right() {
        let text = include_str!("../packs/index.json");
        let l = offers(text).unwrap();
        assert_eq!(l.len(), Json::parse(text).unwrap().get("packs").unwrap().list().len(), "something in it is not understood");
        assert!(!l.is_empty());
        for o in &l {
            let file = std::path::Path::new("packs").join(&o.file);
            let p = pack::parse(&std::fs::read_to_string(&file).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", o.file));
            assert_eq!((p.id.as_str(), p.name.as_str(), p.len() as u32), (o.id.as_str(), o.name.as_str(), o.count), "{}", o.file);
        }
    }

    #[test]
    fn nothing_outside_the_gallery_can_be_asked_for() {
        let l = |id: &str, file: &str| offers(&format!(r#"{{"packs": [{{"id": "{id}", "name": "X", "count": 1, "file": "{file}"}}]}}"#)).unwrap().len();
        assert_eq!(l("snacks", "snacks.petpack"), 1);
        for (id, file) in [("x", "../x.petpack"), ("x", "https://evil.com/x.petpack"), ("x", "x.exe"), ("x", "y.petpack"), ("X", "X.petpack"), ("", ".petpack"), ("a/b", "a/b.petpack"), ("x?y", "x?y.petpack")] {
            assert_eq!(l(id, file), 0, "{file}");
        }
        assert!(offers("[]").is_none() && offers("").is_none());
        let many = format!(r#"{{"packs": [{}]}}"#, (0..50).map(|i| format!(r#"{{"id": "p{i}", "name": "X", "file": "p{i}.petpack"}}"#)).collect::<Vec<_>>().join(","));
        assert_eq!(offers(&many).unwrap().len(), MOST);
    }
}
