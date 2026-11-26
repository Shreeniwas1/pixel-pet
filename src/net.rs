//! Asking the internet for something. Nothing here runs by itself: every feature that uses it is off
//! until the user switches it on, or is one press of a button.
//!
//! It is `curl` that does the asking (Windows, macOS and nearly every Linux bring it). What is secret
//! goes to it through a pipe, never on its command line, where other programs could read it.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

#[derive(Default, Clone)]
pub(crate) struct Ask {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    /// May the answer come from another address? Never when a key is sent along.
    pub follow: bool,
    pub most: u64, // bytes
    pub secs: u32,
}

pub(crate) struct Answer {
    pub code: u32,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Tests talk to a server of their own.
fn testing() -> bool {
    cfg!(test) || std::env::var_os("PIXEL_PET_TEST").is_some()
}

/// Is this an address we would ask? Only https, with nothing odd in it.
pub(crate) fn fine(url: &str) -> bool {
    let rest = match url.strip_prefix("https://") {
        Some(r) => r,
        // a model on one's own machine, and tests
        None => match url.strip_prefix("http://") {
            Some(r) if is_here(r) => r,
            _ => return false,
        },
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    url.len() <= 2000
        && !host.is_empty()
        && !host.contains('@')
        && host.bytes().all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
        && url.bytes().all(|b| (0x21..0x7f).contains(&b) && !b"\"\\<>^`{|}".contains(&b))
}

/// This very computer.
fn is_here(rest: &str) -> bool {
    let host = rest.split('/').next().unwrap_or("");
    let name = host.rsplit_once(':').map_or(host, |(n, p)| if p.bytes().all(|b| b.is_ascii_digit()) { n } else { host });
    matches!(name, "localhost" | "127.0.0.1" | "[::1]")
}

/// In quotes for curl's settings.
fn quoted(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// What curl reads: one setting a line.
fn settings(a: &Ask) -> String {
    let mut s = format!("url = {}\n", quoted(&a.url));
    for (k, v) in &a.headers {
        // a line break would be another header
        let v: String = v.chars().filter(|c| !c.is_control()).collect();
        s.push_str(&format!("header = {}\n", quoted(&format!("{k}: {v}"))));
    }
    if let Some(b) = &a.body {
        s.push_str(&format!("data-raw = {}\n", quoted(b)));
    }
    s
}

fn curl() -> std::path::PathBuf {
    // by its full path, or Windows would look for it in the pet's own folder first
    #[cfg(windows)]
    if let Some(w) = std::env::var_os("SystemRoot") {
        return std::path::Path::new(&w).join("System32").join("curl.exe");
    }
    if cfg!(target_os = "macos") { "/usr/bin/curl".into() } else { "curl".into() }
}

/// Ask, and wait for the answer. Not for the thread that moves the pet.
pub(crate) fn ask(a: &Ask) -> Result<Answer, String> {
    if !fine(&a.url) {
        return Err("That address does not look right.".into());
    }
    let here = a.url.starts_with("http://");
    let mut c = Command::new(curl());
    // -q first: no settings of the user's own
    c.args(["-q", "--silent", "--show-error", "--globoff", "--proto"]).arg(if here { "=http" } else { "=https" });
    c.arg("--max-time").arg(a.secs.clamp(3, 180).to_string()).args(["--connect-timeout", "12"]);
    c.arg("--max-filesize").arg(a.most.to_string());
    if a.follow {
        c.args(["--location", "--max-redirs", "4", "--proto-redir", if here { "=http" } else { "=https" }]);
    }
    c.args(["--user-agent", concat!("PixelPet/", env!("CARGO_PKG_VERSION"))]);
    c.args(["--write-out", "\n%{http_code}", "--config", "-"]);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    let mut child = c.spawn().map_err(|_| "I need curl for that, and cannot find it.".to_string())?;
    let text = settings(a);
    if let Some(mut to) = child.stdin.take() {
        // on a thread, so that neither waits for the other
        std::thread::spawn(move || {
            let _ = to.write_all(text.as_bytes());
        });
    }
    let mut err = child.stderr.take();
    let complaint = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.take(2000).read_to_string(&mut s);
        }
        s
    });
    let mut out = Vec::new();
    if let Some(from) = child.stdout.take() {
        let _ = from.take(a.most + 16).read_to_end(&mut out);
    }
    if out.len() as u64 > a.most + 8 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("The answer was too long.".into());
    }
    let ok = child.wait().map_err(|e| e.to_string())?.success();
    let complaint = complaint.join().unwrap_or_default();
    let at = out.iter().rposition(|b| *b == b'\n').unwrap_or(0);
    let code = std::str::from_utf8(&out[at..]).ok().and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(0);
    if !ok || code == 0 {
        return Err(said(&complaint));
    }
    out.truncate(at);
    Ok(Answer { code, body: out })
}

/// curl's words in ours.
fn said(complaint: &str) -> String {
    let c = complaint.to_lowercase();
    if c.contains("resolve") || c.contains("connect to") || c.contains("network") || c.contains("unreachable") {
        "I cannot reach the internet.".into()
    } else if c.contains("timed out") || c.contains("timeout") {
        "That took too long.".into()
    } else if c.contains("ssl") || c.contains("certificate") {
        "That connection is not safe, so I left it.".into()
    } else if c.contains("filesize") || c.contains("file size") {
        "The answer was too long.".into()
    } else {
        "That did not work.".into()
    }
}

/// The addresses of ours, which a test may replace.
pub(crate) fn base(var: &str, real: &str) -> String {
    match std::env::var(var) {
        Ok(v) if testing() && v.starts_with("http://127.0.0.1:") => v,
        _ => real.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A server that gives the same answer to everybody, and what it was asked.
    pub(crate) fn serve(status: &str, body: &str, times: usize) -> (String, std::sync::mpsc::Receiver<String>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let (status, body) = (status.to_string(), body.to_string());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for _ in 0..times {
                let Ok((mut s, _)) = l.accept() else { return };
                let mut got = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = s.read(&mut buf).unwrap_or(0);
                    got.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&got).into_owned();
                    let Some(end) = text.find("\r\n\r\n") else {
                        if n == 0 {
                            break;
                        }
                        continue;
                    };
                    let len = text.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                    if n == 0 || got.len() >= end + 4 + len {
                        break;
                    }
                }
                let _ = tx.send(String::from_utf8_lossy(&got).into_owned());
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        (url, rx)
    }

    #[test]
    fn only_proper_addresses_are_asked() {
        for u in ["https://a.b", "https://a.b/c?d=e&f=%20", "https://a.b:8443/x", "http://localhost:11434/v1", "http://127.0.0.1:5/x", "http://[::1]:5/x"] {
            assert!(fine(u), "{u}");
        }
        for u in ["http://a.b", "ftp://a.b", "file:///c:/x", "https://", "https://user:pw@a.b", "https://a b", "https://a.b/\"x", "https://a.b/\nheader", "http://localhost.evil.com/x", "http://127.0.0.1.evil.com", "-o x", "https://a.b/{1,2}"] {
            assert!(!fine(u), "{u}");
        }
    }

    #[test]
    fn nothing_can_break_out_of_the_settings() {
        let a = Ask { url: "https://a.b".into(), headers: vec![("X".into(), "k\r\nEvil: 1".into())], body: Some("say \"hi\"\\ \n output = \"/tmp/x\"".into()), ..Ask::default() };
        let s = settings(&a);
        assert_eq!(s.lines().count(), 3, "{s}");
        assert!(s.contains("header = \"X: kEvil: 1\"") && s.contains(r#"data-raw = "say \"hi\"\\ \n output = \"/tmp/x\"""#), "{s}");
    }

    #[test]
    fn it_asks_and_hears_the_answer() {
        let (url, rx) = serve("200 OK", "{\"a\": 1}\nsecond line", 1);
        let body = "line one\nwith \"quotes\" and \\ and \u{20ac}";
        let a = ask(&Ask { url: format!("{url}/x"), headers: vec![("X-Key".into(), "secret".into())], body: Some(body.into()), most: 1000, secs: 10, ..Ask::default() }).unwrap();
        assert_eq!((a.code, a.text().as_str()), (200, "{\"a\": 1}\nsecond line"));
        let got = rx.recv().unwrap();
        assert!(got.starts_with("POST /x ") && got.contains("X-Key: secret") && got.ends_with(body), "{got}");
    }

    #[test]
    fn errors_and_long_answers_are_told() {
        let (url, _rx) = serve("404 Not Found", "nope", 1);
        assert_eq!(ask(&Ask { url, most: 100, secs: 10, ..Ask::default() }).unwrap().code, 404);
        let (url, _rx) = serve("200 OK", &"x".repeat(5000), 1);
        assert!(ask(&Ask { url, most: 100, secs: 10, ..Ask::default() }).is_err());
        assert!(ask(&Ask { url: "http://127.0.0.1:1/".into(), most: 100, secs: 5, ..Ask::default() }).is_err());
        assert!(ask(&Ask { url: "http://a.b/".into(), most: 100, secs: 5, ..Ask::default() }).is_err());
    }

    #[test]
    fn a_key_is_not_taken_to_another_address() {
        let (other, seen) = serve("200 OK", "stolen", 1);
        let (url, _rx) = serve(&format!("302 Found\r\nLocation: {other}/"), "", 1);
        let a = ask(&Ask { url, headers: vec![("Authorization".into(), "Bearer k".into())], most: 100, secs: 10, ..Ask::default() }).unwrap();
        assert_eq!(a.code, 302);
        assert!(seen.try_recv().is_err());
    }
}
