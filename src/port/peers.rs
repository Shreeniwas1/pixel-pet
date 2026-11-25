//! Two pets are two processes. Each keeps a little file that says what windows it has, and a socket
//! the other can send messages to.

use std::collections::HashMap;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

/// What is known about a window of another pet.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Peer {
    pub pid: u32,
    pub id: u32,
    pub class: String,
    pub title: String,
    pub visible: bool,
    pub rect: (i32, i32, i32, i32),
    pub props: HashMap<String, isize>,
}

extern "C" {
    fn getuid() -> u32;
    fn kill(pid: i32, sig: i32) -> i32;
}

/// A folder only this user can look into. Pets with other settings keep to another one.
fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()).map(PathBuf::from).filter(|d| d.is_dir()).unwrap_or_else(std::env::temp_dir);
    let d = base.join(format!("pixel-pet-{}", unsafe { getuid() }));
    let _ = std::fs::DirBuilder::new().mode(0o700).create(&d);
    let m = std::fs::symlink_metadata(&d).ok()?;
    use std::os::unix::fs::MetadataExt;
    // not somebody else's, and nothing that points elsewhere
    (m.is_dir() && m.uid() == unsafe { getuid() } && m.permissions().mode() & 0o077 == 0).then_some(d)
}

fn file(pid: u32, ext: &str) -> Option<PathBuf> {
    Some(dir()?.join(format!("{pid}.{ext}")))
}

fn clean(t: &str) -> String {
    t.replace(['\t', '\n', '\r', '\u{1}'], " ")
}

pub(crate) fn to_text(list: &[Peer]) -> String {
    let mut out = String::new();
    for p in list {
        let props: Vec<String> = p.props.iter().map(|(k, v)| format!("{}={v}", clean(k))).collect();
        out += &format!("{}\t{}\t{}\t{},{},{},{}\t{}\t{}\n", p.id, clean(&p.class), p.visible as u8, p.rect.0, p.rect.1, p.rect.2, p.rect.3, clean(&p.title), props.join("\u{1}"));
    }
    out
}

pub(crate) fn from_text(pid: u32, t: &str) -> Vec<Peer> {
    t.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let [id, class, visible, rect, title, props] = f[..] else { return None };
            let r: Vec<i32> = rect.split(',').filter_map(|n| n.parse().ok()).collect();
            let [a, b, c, d] = r[..] else { return None };
            let props = props.split('\u{1}').filter_map(|p| p.rsplit_once('=')).filter_map(|(k, v)| Some((k.to_string(), v.parse().ok()?))).collect();
            Some(Peer { pid, id: id.parse().ok()?, class: class.into(), title: title.into(), visible: visible == "1", rect: (a, b, c, d), props })
        })
        .collect()
}

pub(crate) struct Post {
    socket: Option<UnixDatagram>,
    written: String,
    pid: u32,
}

impl Post {
    pub fn open() -> Post {
        let pid = std::process::id();
        let socket = file(pid, "sock").and_then(|p| {
            let _ = std::fs::remove_file(&p);
            let s = UnixDatagram::bind(&p).ok()?;
            s.set_nonblocking(true).ok()?;
            Some(s)
        });
        Post { socket, written: String::new(), pid }
    }

    /// Tell the others what there is now.
    pub fn publish(&mut self, list: &[Peer]) {
        let t = to_text(list);
        if t == self.written {
            return;
        }
        if let (Some(to), Some(tmp)) = (file(self.pid, "win"), file(self.pid, "tmp")) {
            if std::fs::write(&tmp, &t).is_ok() && std::fs::rename(&tmp, to).is_ok() {
                self.written = t;
            }
        }
    }

    /// Messages that have come: (window, message, wParam, lParam).
    pub fn take(&self) -> Vec<(u32, u32, usize, isize)> {
        let mut out = Vec::new();
        let mut b = [0u8; 64];
        while let Some(Ok(n)) = self.socket.as_ref().map(|s| s.recv(&mut b)) {
            if n == 24 {
                let q = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
                let w = |i: usize| i64::from_le_bytes(b[i..i + 8].try_into().unwrap());
                out.push((q(0), q(4), w(8) as usize, w(16) as isize));
            }
        }
        out
    }
}

impl Drop for Post {
    fn drop(&mut self) {
        for e in ["win", "sock", "tmp"] {
            if let Some(p) = file(self.pid, e) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

pub(crate) fn send(pid: u32, id: u32, msg: u32, wp: usize, lp: isize) -> bool {
    let mut b = Vec::with_capacity(24);
    b.extend(id.to_le_bytes());
    b.extend(msg.to_le_bytes());
    b.extend((wp as i64).to_le_bytes());
    b.extend((lp as i64).to_le_bytes());
    let Some(to) = file(pid, "sock") else { return false };
    UnixDatagram::unbound().is_ok_and(|s| s.send_to(&b, to).is_ok())
}

pub(crate) fn windows_of(pid: u32) -> Vec<Peer> {
    file(pid, "win").and_then(|p| std::fs::read_to_string(p).ok()).map_or(Vec::new(), |t| from_text(pid, &t))
}

/// The windows of all other pets that are running. What a crashed one left is cleared away.
pub(crate) fn all() -> Vec<Peer> {
    let mut out = Vec::new();
    let Some(d) = dir() else { return out };
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some((pid, kind)) = name.split_once('.').and_then(|(p, k)| Some((p.parse::<u32>().ok()?, k.to_string()))) else { continue };
        if pid == std::process::id() {
            continue;
        }
        if kind != "win" && unsafe { kill(pid as i32, 0) } == 0 {
            continue;
        }
        if unsafe { kill(pid as i32, 0) } != 0 || file(pid, "sock").and_then(|s| UnixDatagram::unbound().ok()?.connect(s).ok()).is_none() {
            for x in ["win", "sock", "tmp"] {
                if let Some(p) = file(pid, x) {
                    let _ = std::fs::remove_file(p);
                }
            }
            continue;
        }
        out.extend(windows_of(pid));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_published_is_what_is_read() {
        let mut p = Peer { pid: 7, id: 3, class: "PixelPetWnd".into(), title: "Pixel Pet Friend 12".into(), visible: true, rect: (-5, 6, 70, 80), props: HashMap::new() };
        p.props.insert("PixelPetX".into(), 1048600);
        p.props.insert("a=b".into(), -4);
        let q = Peer { pid: 7, title: "with\ttab\nand line".into(), ..Peer::default() };
        let back = from_text(7, &to_text(&[p.clone(), q]));
        assert_eq!(back[0], p);
        assert_eq!(back[1].title, "with tab and line");
        assert!(from_text(1, "rubbish\n1\t2\n").is_empty());
    }

    #[test]
    fn messages_arrive() {
        let post = Post::open();
        assert!(send(std::process::id(), 5, 0x8003, 42, -7));
        assert_eq!(post.take(), [(5, 0x8003, 42, -7)]);
        assert!(!send(1, 5, 0, 0, 0), "nobody there");
    }
}
