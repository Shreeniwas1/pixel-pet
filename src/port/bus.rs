//! Linux: the session bus (D-Bus), for what is playing and for an icon in the tray. Just enough of its
//! protocol to ask other programs and to answer them.

use super::*;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

// ---- values, and how they are written ----

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    Byte(u8),
    Bool(bool),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
    Str(String),
    Path(String),
    Sig(String),
    Var(Box<Val>),
    List(String, Vec<Val>), // what is in it, and that
    Rec(Vec<Val>),
    Pair(Box<Val>, Box<Val>),
}

use Val::*;

impl Val {
    pub fn s(t: &str) -> Val {
        Str(t.to_string())
    }

    fn var(v: Val) -> Val {
        Var(Box::new(v))
    }

    pub fn sig(&self) -> String {
        match self {
            Byte(_) => "y".into(),
            Bool(_) => "b".into(),
            I32(_) => "i".into(),
            U32(_) => "u".into(),
            I64(_) => "x".into(),
            U64(_) => "t".into(),
            F64(_) => "d".into(),
            Str(_) => "s".into(),
            Path(_) => "o".into(),
            Sig(_) => "g".into(),
            Var(_) => "v".into(),
            List(e, _) => format!("a{e}"),
            Rec(v) => format!("({})", v.iter().map(Val::sig).collect::<String>()),
            Pair(a, b) => format!("{{{}{}}}", a.sig(), b.sig()),
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Str(t) | Path(t) | Sig(t) => Some(t),
            Var(v) => v.text(),
            _ => None,
        }
    }

    pub fn list(&self) -> &[Val] {
        match self {
            List(_, v) => v,
            Var(v) => v.list(),
            _ => &[],
        }
    }

    /// What a map has under a name.
    pub fn get(&self, key: &str) -> Option<&Val> {
        self.list().iter().find_map(|p| match p {
            Pair(k, v) if k.text() == Some(key) => Some(&**v),
            _ => None,
        })
    }
}

fn pad(o: &mut Vec<u8>, n: usize) {
    while o.len() % n != 0 {
        o.push(0);
    }
}

fn align(sig: u8) -> usize {
    match sig {
        b'y' | b'g' | b'v' => 1,
        b'n' | b'q' => 2,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,
        _ => 4,
    }
}

fn put(o: &mut Vec<u8>, v: &Val) {
    match v {
        Byte(b) => o.push(*b),
        Bool(b) => put(o, &U32(*b as u32)),
        I32(n) => {
            pad(o, 4);
            o.extend(n.to_le_bytes());
        }
        U32(n) => {
            pad(o, 4);
            o.extend(n.to_le_bytes());
        }
        I64(n) => {
            pad(o, 8);
            o.extend(n.to_le_bytes());
        }
        U64(n) => {
            pad(o, 8);
            o.extend(n.to_le_bytes());
        }
        F64(n) => {
            pad(o, 8);
            o.extend(n.to_le_bytes());
        }
        Str(t) | Path(t) => {
            put(o, &U32(t.len() as u32));
            o.extend(t.bytes());
            o.push(0);
        }
        Sig(t) => {
            o.push(t.len() as u8);
            o.extend(t.bytes());
            o.push(0);
        }
        Var(v) => {
            put(o, &Sig(v.sig()));
            put(o, v);
        }
        List(e, items) => {
            put(o, &U32(0));
            let len_at = o.len() - 4;
            pad(o, align(e.as_bytes()[0]));
            let from = o.len();
            for i in items {
                put(o, i);
            }
            let n = (o.len() - from) as u32;
            o[len_at..len_at + 4].copy_from_slice(&n.to_le_bytes());
        }
        Rec(v) => {
            pad(o, 8);
            v.iter().for_each(|v| put(o, v));
        }
        Pair(a, b) => {
            pad(o, 8);
            put(o, a);
            put(o, b);
        }
    }
}

/// The first whole type of a signature, and the rest.
fn first(sig: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = match sig.first()? {
        b'a' => 1 + first(&sig[1..])?.0.len(),
        open @ (b'(' | b'{') => {
            let close = if *open == b'(' { b')' } else { b'}' };
            let mut deep = 0;
            let mut end = None;
            for (i, c) in sig.iter().enumerate() {
                deep += (c == open) as i32;
                deep -= (*c == close) as i32;
                if deep == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            end?
        }
        _ => 1,
    };
    Some(sig.split_at(end))
}

struct Dec<'a> {
    b: &'a [u8],
    at: usize,
}

impl Dec<'_> {
    fn skip(&mut self, n: usize) -> Option<()> {
        self.at = self.at.div_ceil(n) * n;
        (self.at <= self.b.len()).then_some(())
    }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let r = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(r)
    }

    fn num<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.skip(N)?;
        self.take(N)?.try_into().ok()
    }

    fn u32(&mut self) -> Option<u32> {
        self.num().map(u32::from_le_bytes)
    }

    fn val(&mut self, sig: &[u8], deep: u32) -> Option<Val> {
        if deep > 24 {
            return None;
        }
        Some(match *sig.first()? {
            b'y' => Byte(self.take(1)?[0]),
            b'b' => Bool(self.u32()? != 0),
            b'n' => I32(i16::from_le_bytes(self.num()?) as i32),
            b'q' => U32(u16::from_le_bytes(self.num()?) as u32),
            b'i' => I32(i32::from_le_bytes(self.num()?)),
            b'u' | b'h' => U32(self.u32()?),
            b'x' => I64(i64::from_le_bytes(self.num()?)),
            b't' => U64(u64::from_le_bytes(self.num()?)),
            b'd' => F64(f64::from_le_bytes(self.num()?)),
            c @ (b's' | b'o') => {
                let n = self.u32()? as usize;
                let t = String::from_utf8_lossy(self.take(n)?).into_owned();
                self.take(1)?;
                if c == b's' { Str(t) } else { Path(t) }
            }
            b'g' => {
                let n = self.take(1)?[0] as usize;
                let t = String::from_utf8_lossy(self.take(n)?).into_owned();
                self.take(1)?;
                Sig(t)
            }
            b'v' => {
                let Sig(s) = self.val(b"g", deep + 1)? else { return None };
                let (one, rest) = first(s.as_bytes())?;
                if !rest.is_empty() {
                    return None;
                }
                Var(Box::new(self.val(one, deep + 1)?))
            }
            b'a' => {
                let n = self.u32()? as usize;
                let e = first(&sig[1..])?.0;
                self.skip(align(e[0]))?;
                let end = self.at.checked_add(n).filter(|e| *e <= self.b.len())?;
                let mut v = Vec::new();
                while self.at < end {
                    v.push(self.val(e, deep + 1)?);
                }
                List(String::from_utf8_lossy(e).into_owned(), v)
            }
            b'(' => {
                self.skip(8)?;
                let mut inner = sig.get(1..sig.len() - 1)?;
                let mut v = Vec::new();
                while !inner.is_empty() {
                    let (one, rest) = first(inner)?;
                    v.push(self.val(one, deep + 1)?);
                    inner = rest;
                }
                Rec(v)
            }
            b'{' => {
                self.skip(8)?;
                let (k, rest) = first(sig.get(1..sig.len() - 1)?)?;
                Pair(Box::new(self.val(k, deep + 1)?), Box::new(self.val(rest, deep + 1)?))
            }
            _ => return None,
        })
    }
}

// ---- messages ----

pub(crate) const CALL: u8 = 1;
pub(crate) const REPLY: u8 = 2;
pub(crate) const ERROR: u8 = 3;
pub(crate) const SIGNAL: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Msg {
    pub kind: u8,
    pub serial: u32,
    pub path: String,
    pub face: String,
    pub member: String,
    pub error: String,
    pub to: String,
    pub from: String,
    pub answers: u32,
    pub body: Vec<Val>,
}

fn write(m: &Msg) -> Vec<u8> {
    let mut body = Vec::new();
    m.body.iter().for_each(|v| put(&mut body, v));
    let mut f = Vec::new();
    let mut add = |n: u8, v: Val| f.push(Rec(vec![Byte(n), Val::var(v)]));
    for (n, t) in [(2u8, &m.face), (3, &m.member), (4, &m.error), (6, &m.to)] {
        if !t.is_empty() {
            add(n, Str(t.clone()));
        }
    }
    if !m.path.is_empty() {
        add(1, Path(m.path.clone()));
    }
    if m.answers != 0 {
        add(5, U32(m.answers));
    }
    if !m.body.is_empty() {
        add(8, Sig(m.body.iter().map(Val::sig).collect()));
    }
    let mut o = vec![b'l', m.kind, 0, 1];
    put(&mut o, &U32(body.len() as u32));
    put(&mut o, &U32(m.serial));
    put(&mut o, &List("(yv)".into(), f));
    pad(&mut o, 8);
    o.extend(body);
    o
}

/// How long the message at the start of this is, if that can be told yet. None = not a message.
fn length(b: &[u8]) -> Option<Option<usize>> {
    if b.len() < 16 {
        return Some(None);
    }
    if b[0] != b'l' || b[3] != 1 {
        return None;
    }
    let n = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
    let total = (16 + n(12)).div_ceil(8) * 8 + n(4);
    (total <= 8 << 20).then_some(Some(total))
}

fn read(b: &[u8]) -> Option<Msg> {
    let mut d = Dec { b, at: 4 };
    let mut m = Msg { kind: b[1], ..Msg::default() };
    d.u32()?;
    m.serial = d.u32()?;
    let mut sig = String::new();
    for f in d.val(b"a(yv)", 0)?.list() {
        let Rec(f) = f else { return None };
        let [Byte(n), Var(v)] = &f[..] else { return None };
        let t = v.text().unwrap_or("").to_string();
        match n {
            1 => m.path = t,
            2 => m.face = t,
            3 => m.member = t,
            4 => m.error = t,
            5 => m.answers = if let U32(n) = **v { n } else { 0 },
            6 => m.to = t,
            7 => m.from = t,
            8 => sig = t,
            _ => {}
        }
    }
    d.skip(8)?;
    let mut s = sig.as_bytes();
    // within the body things are placed as if it stood alone
    let mut body = Dec { b: &b[d.at..], at: 0 };
    while !s.is_empty() {
        let (one, rest) = first(s)?;
        m.body.push(body.val(one, 0)?);
        s = rest;
    }
    Some(m)
}

// ---- the connection ----

extern "C" {
    fn getuid() -> u32;
}

pub(crate) struct Bus {
    to: UnixStream,
    got: Vec<u8>,
    serial: u32,
    others: VecDeque<Msg>,
}

fn connect() -> Option<UnixStream> {
    let uid = unsafe { getuid() };
    let all = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_else(|_| format!("unix:path=/run/user/{uid}/bus"));
    for one in all.split(';') {
        let Some(rest) = one.strip_prefix("unix:") else { continue };
        for part in rest.split(',') {
            let found = match part.split_once('=') {
                Some(("path", p)) => UnixStream::connect(p).ok(),
                #[cfg(target_os = "linux")]
                Some(("abstract", n)) => {
                    use std::os::linux::net::SocketAddrExt;
                    std::os::unix::net::SocketAddr::from_abstract_name(n).ok().and_then(|a| UnixStream::connect_addr(&a).ok())
                }
                _ => None,
            };
            if found.is_some() {
                return found;
            }
        }
    }
    None
}

impl Bus {
    pub fn open() -> Option<Bus> {
        let mut to = connect()?;
        to.set_read_timeout(Some(Duration::from_millis(1500))).ok()?;
        to.set_write_timeout(Some(Duration::from_millis(1500))).ok()?;
        let uid: String = unsafe { getuid() }.to_string().bytes().map(|b| format!("{b:02x}")).collect();
        to.write_all(format!("\0AUTH EXTERNAL {uid}\r\n").as_bytes()).ok()?;
        let mut line = Vec::new();
        let mut c = [0u8];
        while !line.ends_with(b"\r\n") && line.len() < 512 {
            to.read_exact(&mut c).ok()?;
            line.push(c[0]);
        }
        if !line.starts_with(b"OK") {
            return None;
        }
        to.write_all(b"BEGIN\r\n").ok()?;
        let mut b = Bus { to, got: Vec::new(), serial: 0, others: VecDeque::new() };
        b.ask("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello", vec![])?;
        Some(b)
    }

    pub fn send(&mut self, mut m: Msg) -> Option<u32> {
        self.serial += 1;
        m.serial = self.serial;
        self.to.write_all(&write(&m)).ok()?;
        Some(self.serial)
    }

    /// The next message, waiting as long as reads do. Err = nothing came; None = it is over.
    pub fn next(&mut self) -> Option<Result<Msg, ()>> {
        if let Some(m) = self.others.pop_front() {
            return Some(Ok(m));
        }
        self.read()
    }

    fn read(&mut self) -> Option<Result<Msg, ()>> {
        loop {
            if let Some(n) = length(&self.got)? {
                if self.got.len() >= n {
                    let m = read(&self.got[..n]);
                    self.got.drain(..n);
                    match m {
                        Some(m) => return Some(Ok(m)),
                        None => continue, // nothing we understand
                    }
                }
            }
            let mut buf = [0u8; 16384];
            match self.to.read(&mut buf) {
                Ok(0) => return None,
                Ok(n) => self.got.extend_from_slice(&buf[..n]),
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return Some(Err(())),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return None,
            }
        }
    }

    pub fn ask(&mut self, to: &str, path: &str, face: &str, member: &str, body: Vec<Val>) -> Option<Vec<Val>> {
        let serial = self.send(Msg { kind: CALL, to: to.into(), path: path.into(), face: face.into(), member: member.into(), body, ..Msg::default() })?;
        let end = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < end {
            match self.read()? {
                Ok(m) if m.answers == serial => return (m.kind == REPLY).then_some(m.body),
                Ok(m) => {
                    self.others.push_back(m);
                    if self.others.len() > 200 {
                        self.others.pop_front();
                    }
                }
                Err(()) => return None,
            }
        }
        None
    }
}

// ---- what is playing (MPRIS) ----

/// Programs that show films, not music.
fn is_video(name: &str) -> bool {
    ["vlc", "mpv", "totem", "celluloid", "kodi", "haruna", "dragonplayer", "smplayer", "showtime", "clapper"].iter().any(|v| name.to_lowercase().contains(v))
}

/// Title, artist, playing, a film?
pub(crate) type Playing = (String, String, bool, bool);

/// What one player says about itself.
fn player(name: &str, all: &Val) -> Option<Playing> {
    let playing = all.get("PlaybackStatus")?.text()? == "Playing";
    let meta = all.get("Metadata")?;
    let title = meta.get("xesam:title").and_then(Val::text).unwrap_or("").to_string();
    let artist = meta.get("xesam:artist").map(|a| a.list().iter().filter_map(Val::text).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    let browser = ["firefox", "chrom", "brave", "edge", "opera", "vivaldi"].iter().any(|b| name.to_lowercase().contains(b));
    // a browser plays songs that say who sings, and films that do not
    let video = is_video(name) || (browser && artist.is_empty());
    Some((title, artist, playing, video))
}

struct Player {
    bus: Option<Bus>,
    tried: Option<Instant>,
    name: String,
}

static PLAYER: Mutex<Player> = Mutex::new(Player { bus: None, tried: None, name: String::new() });

const PATH: &str = "/org/mpris/MediaPlayer2";
const FACE: &str = "org.mpris.MediaPlayer2.Player";

pub(crate) fn playing() -> Option<Playing> {
    let mut p = PLAYER.lock().unwrap();
    if p.bus.is_none() {
        if p.tried.is_some_and(|t| t.elapsed() < Duration::from_secs(30)) {
            return None;
        }
        p.tried = Some(Instant::now());
        p.bus = Bus::open();
    }
    let bus = p.bus.as_mut()?;
    let Some(names) = bus.ask("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", vec![]) else {
        p.bus = None; // it has gone
        return None;
    };
    bus.others.clear();
    let names: Vec<String> = names.first().map_or(Vec::new(), |l| l.list().iter().filter_map(Val::text).filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).take(8).map(str::to_string).collect());
    let mut best: Option<(String, Playing)> = None;
    for n in names {
        let Some(a) = bus.ask(&n, PATH, "org.freedesktop.DBus.Properties", "GetAll", vec![Val::s(FACE)]) else { continue };
        let Some(now) = a.first().and_then(|a| player(&n, a)) else { continue };
        if best.as_ref().is_none_or(|b| now.2 && !b.1 .2) {
            best = Some((n, now));
        }
    }
    p.name = best.as_ref().map_or(String::new(), |b| b.0.clone());
    best.map(|b| b.1)
}

/// 1 play or pause, 2 the next, 3 the one before.
pub(crate) fn command(c: u32) {
    let mut p = PLAYER.lock().unwrap();
    let name = p.name.clone();
    let what = match c {
        1 => "PlayPause",
        2 => "Next",
        _ => "Previous",
    };
    if let (Some(b), false) = (p.bus.as_mut(), name.is_empty()) {
        b.ask(&name, PATH, FACE, what, vec![]);
    }
}

// ---- the icon in the tray (StatusNotifierItem) ----

/// What was done to the icon.
pub(crate) static EVENTS: Mutex<VecDeque<Ev>> = Mutex::new(VecDeque::new());
static TIP: Mutex<String> = Mutex::new(String::new());
static ON: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

const ITEM: &str = "org.kde.StatusNotifierItem";
const ITEM_PATH: &str = "/StatusNotifierItem";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";

/// A paw, `n` dots wide: each dot as four bytes, the see-through one first.
fn icon(n: usize) -> Val {
    let art = crate::art::ICON_PETS;
    let cell = n / 8;
    let (w, h) = (art[0].len() * cell, art.len() * cell);
    let (x0, y0) = ((n - w) / 2, (n - h) / 2);
    let mut px = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let lit = x >= x0 && y >= y0 && art.get((y - y0) / cell).and_then(|r| r.get((x - x0) / cell)) == Some(&b'X');
            px.extend(if lit { [0xffu8, 0xf2, 0xa6, 0x5a] } else { [0; 4] });
        }
    }
    Rec(vec![I32(n as i32), I32(n as i32), List("y".into(), px.into_iter().map(Byte).collect())])
}

fn property(name: &str) -> Option<Val> {
    let none = || List("(iiay)".into(), Vec::new());
    Some(match name {
        "Category" => Val::s("ApplicationStatus"),
        "Id" => Val::s("pixel-pet"),
        "Title" => Val::s("Pixel Pet"),
        "Status" => Val::s("Active"),
        "WindowId" => I32(0),
        "IconName" | "OverlayIconName" | "AttentionIconName" | "AttentionMovieName" | "IconThemePath" => Val::s(""),
        "IconPixmap" => List("(iiay)".into(), vec![icon(24), icon(48)]),
        "OverlayIconPixmap" | "AttentionIconPixmap" => none(),
        "ToolTip" => {
            let tip = TIP.lock().unwrap().clone();
            Rec(vec![Val::s(""), none(), Val::s("Pixel Pet"), Val::s(&tip)])
        }
        "ItemIsMenu" => Bool(false),
        "Menu" => Path("/NO_DBUSMENU".into()),
        _ => return None,
    })
}

const PROPERTIES: [&str; 15] = ["Category", "Id", "Title", "Status", "WindowId", "IconName", "IconThemePath", "IconPixmap", "OverlayIconName", "OverlayIconPixmap", "AttentionIconName", "AttentionIconPixmap", "ToolTip", "ItemIsMenu", "Menu"];

const ABOUT: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node><interface name="org.kde.StatusNotifierItem">
<property name="Category" type="s" access="read"/><property name="Id" type="s" access="read"/><property name="Title" type="s" access="read"/><property name="Status" type="s" access="read"/>
<property name="WindowId" type="i" access="read"/><property name="IconName" type="s" access="read"/><property name="IconPixmap" type="a(iiay)" access="read"/><property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
<property name="ItemIsMenu" type="b" access="read"/><property name="Menu" type="o" access="read"/>
<method name="Activate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
<method name="SecondaryActivate"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
<method name="ContextMenu"><arg name="x" type="i" direction="in"/><arg name="y" type="i" direction="in"/></method>
<method name="Scroll"><arg name="delta" type="i" direction="in"/><arg name="orientation" type="s" direction="in"/></method>
<signal name="NewToolTip"/></interface></node>"#;

/// The answer to a question about the icon: what to say, and what it means to the pet.
fn answer(m: &Msg) -> (Result<Vec<Val>, &'static str>, Option<u32>) {
    let arg = |i: usize| m.body.get(i).and_then(Val::text).unwrap_or("");
    match (m.face.as_str(), m.member.as_str()) {
        ("org.freedesktop.DBus.Properties", "Get") if m.path == ITEM_PATH => match property(arg(1)) {
            Some(v) => (Ok(vec![Val::var(v)]), None),
            None => (Err("org.freedesktop.DBus.Error.UnknownProperty"), None),
        },
        ("org.freedesktop.DBus.Properties", "GetAll") if m.path == ITEM_PATH => {
            let all = PROPERTIES.iter().filter_map(|n| Some(Pair(Box::new(Val::s(n)), Box::new(Val::var(property(n)?))))).collect();
            (Ok(vec![List("{sv}".into(), all)]), None)
        }
        ("org.freedesktop.DBus.Introspectable", "Introspect") => (Ok(vec![Val::s(if m.path == ITEM_PATH { ABOUT } else { "<node/>" })]), None),
        ("org.freedesktop.DBus.Peer", "Ping") => (Ok(vec![]), None),
        // WM_LBUTTONUP, WM_RBUTTONUP
        (ITEM | "", "Activate") => (Ok(vec![]), Some(0x0202)),
        (ITEM | "", "ContextMenu" | "SecondaryActivate") => (Ok(vec![]), Some(0x0205)),
        (ITEM | "", "Scroll") => (Ok(vec![]), None),
        _ => (Err("org.freedesktop.DBus.Error.UnknownMethod"), None),
    }
}

fn show_icon() -> Option<()> {
    let mut bus = Bus::open()?;
    let name = format!("{ITEM}-{}-1", std::process::id());
    let me = ("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus");
    bus.ask(me.0, me.1, me.2, "RequestName", vec![Val::s(&name), U32(4)])?;
    // the tray may come (back) later
    bus.ask(me.0, me.1, me.2, "AddMatch", vec![Val::s(&format!("type='signal',member='NameOwnerChanged',arg0='{WATCHER}'"))]);
    let join = |bus: &mut Bus| {
        bus.send(Msg { kind: CALL, to: WATCHER.into(), path: "/StatusNotifierWatcher".into(), face: WATCHER.into(), member: "RegisterStatusNotifierItem".into(), body: vec![Val::s(&name)], ..Msg::default() })
    };
    join(&mut bus)?;
    let mut tip = String::new();
    while ON.load(Relaxed) {
        let now = TIP.lock().unwrap().clone();
        if now != tip {
            tip = now;
            bus.send(Msg { kind: SIGNAL, path: ITEM_PATH.into(), face: ITEM.into(), member: "NewToolTip".into(), ..Msg::default() })?;
        }
        match bus.next()? {
            Ok(m) if m.kind == CALL => {
                let (a, done) = answer(&m);
                let mut r = Msg { to: m.from.clone(), answers: m.serial, ..Msg::default() };
                match a {
                    Ok(body) => (r.kind, r.body) = (REPLY, body),
                    Err(e) => (r.kind, r.error) = (ERROR, e.into()),
                }
                bus.send(r)?;
                if let Some(msg) = done {
                    EVENTS.lock().unwrap().push_back(Ev::Tray(msg));
                }
            }
            Ok(m) if m.kind == SIGNAL && m.member == "NameOwnerChanged" && m.body.get(2).and_then(Val::text).is_some_and(|o| !o.is_empty()) => {
                join(&mut bus)?;
            }
            _ => {}
        }
    }
    Some(())
}

/// Put the icon there, change what it says, or take it away.
pub(crate) fn tray(on: bool, tip: &str) -> bool {
    *TIP.lock().unwrap() = tip.lines().skip(0).collect::<Vec<_>>().join("<br/>");
    ON.store(on, Relaxed);
    if on && !RUNNING.swap(true, Relaxed) {
        std::thread::spawn(|| {
            // if the bus goes away, it is asked for again after a while
            while ON.load(Relaxed) {
                show_icon();
                for _ in 0..60 {
                    if !ON.load(Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
            RUNNING.store(false, Relaxed);
        });
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(l: Vec<(&str, Val)>) -> Val {
        List("{sv}".into(), l.into_iter().map(|(k, v)| Pair(Box::new(Val::s(k)), Box::new(Val::var(v)))).collect())
    }

    #[test]
    fn what_is_written_can_be_read() {
        let m = Msg {
            kind: CALL,
            serial: 7,
            path: "/a/b".into(),
            face: "c.d".into(),
            member: "E".into(),
            to: ":1.5".into(),
            body: vec![Byte(1), Bool(true), Val::s("h\u{e9}llo"), I64(-5), F64(1.5), map(vec![("x", U32(3)), ("yy", Rec(vec![Byte(9), U64(8), List("s".into(), vec![Val::s("z")])]))]), List("(iiay)".into(), vec![icon(24)]), List("x".into(), vec![]), Path("/p".into()), Sig("a{sv}".into()), I32(-2)],
            ..Msg::default()
        };
        let b = write(&m);
        assert_eq!(length(&b), Some(Some(b.len())));
        assert_eq!(read(&b), Some(m));
        // as the specification has it: a header, a body, everything in its place
        let hello = write(&Msg { kind: CALL, serial: 1, path: "/org/freedesktop/DBus".into(), face: "org.freedesktop.DBus".into(), member: "Hello".into(), to: "org.freedesktop.DBus".into(), ..Msg::default() });
        assert_eq!((&hello[..12], hello.len() % 8), (&b"l\x01\x00\x01\0\0\0\0\x01\0\0\0"[..], 0));
    }

    #[test]
    fn broken_messages_do_no_harm() {
        let good = write(&Msg { kind: REPLY, serial: 2, answers: 1, body: vec![map(vec![("a", List("s".into(), vec![Val::s("b")])), ("c", Val::s("d"))])], ..Msg::default() });
        for i in 0..good.len() {
            let _ = length(&good[..i]);
            if i >= 16 {
                let _ = read(&good[..i]);
            }
        }
        let mut seed = 9u32;
        for _ in 0..20000 {
            let mut b = good.clone();
            for _ in 0..3 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let i = (seed >> 8) as usize % b.len();
                b[i] = (seed >> 3) as u8;
            }
            let _ = length(&b);
            let _ = read(&b);
        }
        assert_eq!(length(b"HTTP/1.1 200 OK\r\n"), None);
        assert_eq!(first(b"a{s(iv)}u"), Some((&b"a{s(iv)}"[..], &b"u"[..])));
        assert_eq!(first(b"(ii"), None);
    }

    #[test]
    fn players_tell_what_they_play() {
        let song = |status: &str, artist: &str| {
            map(vec![("PlaybackStatus", Val::s(status)), ("Metadata", map(vec![("xesam:title", Val::s("A Song")), ("xesam:artist", List("s".into(), if artist.is_empty() { vec![] } else { vec![Val::s(artist)] }))]))])
        };
        assert_eq!(player("org.mpris.MediaPlayer2.spotify", &song("Playing", "Pets")), Some(("A Song".into(), "Pets".into(), true, false)));
        assert_eq!(player("org.mpris.MediaPlayer2.spotify", &song("Paused", "Pets")).map(|p| p.2), Some(false));
        assert_eq!(player("org.mpris.MediaPlayer2.vlc", &song("Playing", "Pets")).map(|p| p.3), Some(true));
        assert_eq!(player("org.mpris.MediaPlayer2.firefox.instance1", &song("Playing", "")).map(|p| p.3), Some(true));
        assert_eq!(player("org.mpris.MediaPlayer2.firefox.instance1", &song("Playing", "Pets")).map(|p| p.3), Some(false));
        assert_eq!(player("x", &map(vec![])), None);
    }

    /// Has a name on the bus and answers whatever it is asked.
    fn stand_in(name: &str, reply: impl Fn(&Msg) -> Vec<Val> + Send + 'static) -> std::sync::mpsc::Receiver<Msg> {
        let mut b = Bus::open().expect("there is a bus");
        b.ask("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", vec![Val::s(name), U32(4)]).expect("the name is free");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Some(m) = b.next() {
                let Ok(m) = m else { continue };
                if m.kind == CALL {
                    let _ = b.send(Msg { kind: REPLY, to: m.from.clone(), answers: m.serial, body: reply(&m), ..Msg::default() });
                    let _ = tx.send(m);
                }
            }
        });
        rx
    }

    /// Needs a bus of its own: dbus-run-session -- cargo test --bin pixel-pet on_a_real_bus -- --ignored
    #[test]
    #[ignore]
    fn on_a_real_bus() {
        // a player
        let asked = stand_in("org.mpris.MediaPlayer2.test", |m| {
            if m.member == "GetAll" {
                vec![map(vec![("PlaybackStatus", Val::s("Playing")), ("CanPlay", Bool(true)), ("Metadata", map(vec![("xesam:title", Val::s("A Song")), ("mpris:length", I64(5)), ("xesam:artist", List("s".into(), vec![Val::s("The"), Val::s("Pets")]))]))])]
            } else {
                vec![]
            }
        });
        assert_eq!(playing(), Some(("A Song".into(), "The, Pets".into(), true, false)));
        command(2);
        let done: Vec<String> = (0..2).filter_map(|_| asked.recv_timeout(Duration::from_secs(2)).ok()).map(|m| m.member).collect();
        assert_eq!(done, ["GetAll", "Next"]);
        // a tray
        let joined = stand_in(WATCHER, |_| vec![]);
        tray(true, "Down 1 KB/s");
        let m = joined.recv_timeout(Duration::from_secs(5)).expect("the icon did not come");
        let name = m.body[0].text().unwrap().to_string();
        assert_eq!((m.member.as_str(), name.as_str()), ("RegisterStatusNotifierItem", format!("{ITEM}-{}-1", std::process::id()).as_str()));
        let mut b = Bus::open().unwrap();
        let all = b.ask(&name, ITEM_PATH, "org.freedesktop.DBus.Properties", "GetAll", vec![Val::s(ITEM)]).expect("it answers");
        assert_eq!(all[0].get("Title").and_then(Val::text), Some("Pixel Pet"));
        b.ask(&name, ITEM_PATH, ITEM, "Activate", vec![I32(5), I32(6)]).expect("it can be clicked");
        assert!(matches!(EVENTS.lock().unwrap().pop_front(), Some(Ev::Tray(0x0202))));
        // another program's view of it
        let said = std::process::Command::new("busctl").args(["--user", "get-property", &name, ITEM_PATH, ITEM, "ToolTip"]).output().expect("busctl");
        println!("busctl: {}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
        assert!(String::from_utf8_lossy(&said.stdout).contains("Down 1 KB/s"));
        let said = std::process::Command::new("busctl").args(["--user", "introspect", &name, ITEM_PATH]).output().expect("busctl");
        println!("{}", String::from_utf8_lossy(&said.stdout).lines().take(14).collect::<Vec<_>>().join("
"));
        assert!(String::from_utf8_lossy(&said.stdout).contains(".IconPixmap"));
        tray(false, "");
    }

    #[test]
    fn the_icon_answers_for_itself() {
        let ask = |face: &str, member: &str, body: Vec<Val>| answer(&Msg { kind: CALL, path: ITEM_PATH.into(), face: face.into(), member: member.into(), body, ..Msg::default() });
        let (all, _) = ask("org.freedesktop.DBus.Properties", "GetAll", vec![Val::s(ITEM)]);
        let all = all.unwrap();
        assert_eq!(all[0].sig(), "a{sv}");
        assert_eq!(all[0].get("ToolTip").map(|v| if let Var(v) = v { v.sig() } else { String::new() }), Some("(sa(iiay)ss)".into()));
        assert_eq!(all[0].get("IconPixmap").map(|v| if let Var(v) = v { v.sig() } else { String::new() }), Some("a(iiay)".into()));
        assert_eq!(ask("org.freedesktop.DBus.Properties", "Get", vec![Val::s(ITEM), Val::s("Id")]).0, Ok(vec![Val::var(Val::s("pixel-pet"))]));
        assert!(ask("org.freedesktop.DBus.Properties", "Get", vec![Val::s(ITEM), Val::s("Nope")]).0.is_err());
        assert_eq!(ask(ITEM, "Activate", vec![I32(0), I32(0)]).1, Some(0x0202));
        assert_eq!(ask(ITEM, "ContextMenu", vec![I32(0), I32(0)]).1, Some(0x0205));
        assert!(ask("some.thing", "Else", vec![]).0.is_err());
        let Rec(i) = icon(24) else { panic!() };
        assert_eq!(i[2].list().len(), 24 * 24 * 4);
        assert!(i[2].list().contains(&Byte(0xf2)));
    }
}
