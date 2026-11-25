//! Reading and writing JSON, just as much as is needed.

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Json>),
    Map(Vec<(String, Json)>),
}

const DEEP: u32 = 64;

impl Json {
    pub fn parse(text: &str) -> Option<Json> {
        let mut r = Reader { b: text.as_bytes(), at: 0 };
        let v = r.value(0)?;
        r.space();
        (r.at == r.b.len()).then_some(v)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// "a.0.b": maps by name, lists by number.
    pub fn at(&self, path: &str) -> Option<&Json> {
        path.split('.').try_fold(self, |v, p| match v {
            Json::List(l) => l.get(p.parse::<usize>().ok()?),
            v => v.get(p),
        })
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn list(&self) -> &[Json] {
        match self {
            Json::List(l) => l,
            _ => &[],
        }
    }

    /// The text at a path, or nothing.
    pub fn text(&self, path: &str) -> &str {
        self.at(path).and_then(Json::str).unwrap_or("")
    }
}

/// A text as JSON writes it, with its quotes.
pub(crate) fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self.b.get(self.at).is_some_and(|c| c.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Option<()> {
        self.space();
        (self.b.get(self.at) == Some(&c)).then(|| self.at += 1)
    }

    fn word(&mut self, w: &str, v: Json) -> Option<Json> {
        self.b[self.at..].starts_with(w.as_bytes()).then(|| {
            self.at += w.len();
            v
        })
    }

    fn value(&mut self, deep: u32) -> Option<Json> {
        if deep > DEEP {
            return None;
        }
        self.space();
        match *self.b.get(self.at)? {
            b'n' => self.word("null", Json::Null),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'"' => self.string().map(Json::Str),
            b'[' => {
                self.at += 1;
                let mut l = Vec::new();
                if self.eat(b']').is_some() {
                    return Some(Json::List(l));
                }
                loop {
                    l.push(self.value(deep + 1)?);
                    if self.eat(b',').is_none() {
                        self.eat(b']')?;
                        return Some(Json::List(l));
                    }
                }
            }
            b'{' => {
                self.at += 1;
                let mut m = Vec::new();
                if self.eat(b'}').is_some() {
                    return Some(Json::Map(m));
                }
                loop {
                    self.space();
                    let k = self.string()?;
                    self.eat(b':')?;
                    m.push((k, self.value(deep + 1)?));
                    if self.eat(b',').is_none() {
                        self.eat(b'}')?;
                        return Some(Json::Map(m));
                    }
                }
            }
            _ => {
                let from = self.at;
                while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit() || b"+-.eE".contains(c)) {
                    self.at += 1;
                }
                std::str::from_utf8(&self.b[from..self.at]).ok()?.parse().ok().map(Json::Num)
            }
        }
    }

    fn hex(&mut self) -> Option<u32> {
        let h = self.b.get(self.at..self.at + 4)?;
        self.at += 4;
        u32::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
    }

    fn string(&mut self) -> Option<String> {
        if self.b.get(self.at) != Some(&b'"') {
            return None;
        }
        self.at += 1;
        let mut o = Vec::new();
        loop {
            let c = *self.b.get(self.at)?;
            self.at += 1;
            match c {
                b'"' => return String::from_utf8(o).ok(),
                b'\\' => {
                    let e = *self.b.get(self.at)?;
                    self.at += 1;
                    let ch = match e {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'u' => {
                            let mut u = self.hex()?;
                            if (0xd800..0xdc00).contains(&u) && self.b[self.at..].starts_with(b"\\u") {
                                let back = self.at;
                                self.at += 2;
                                let low = self.hex()?;
                                if (0xdc00..0xe000).contains(&low) {
                                    u = 0x10000 + ((u - 0xd800) << 10) + (low - 0xdc00);
                                } else {
                                    self.at = back;
                                }
                            }
                            char::from_u32(u).unwrap_or('\u{fffd}')
                        }
                        e => e as char,
                    };
                    let mut buf = [0; 4];
                    o.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c => o.push(c),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_what_others_write() {
        let j = Json::parse(r#" {"a": [1, -2.5e1, {"b": "x\n\"\u00e9\ud83d\ude00"}], "c": null, "d": true, "e": {}, "f": []} "#).unwrap();
        assert_eq!(j.at("a.1").and_then(Json::num), Some(-25.0));
        assert_eq!(j.text("a.2.b"), "x\n\"\u{e9}\u{1f600}");
        assert_eq!(j.get("c"), Some(&Json::Null));
        assert_eq!(j.get("a").unwrap().list().len(), 3);
        assert_eq!((j.text("nothing.here"), j.text("d")), ("", ""));
    }

    #[test]
    fn broken_text_is_refused() {
        for t in ["", "{", "[1,", "\"abc", "{\"a\"}", "[1] x", "nul", "{\"a\":}", "\"\\u12\"", "[1 2]", "-"] {
            assert!(Json::parse(t).is_none(), "{t}");
        }
        assert!(Json::parse(&"[".repeat(100_000)).is_none(), "too deep");
        // cut anywhere it is refused, and never worse
        let t = r#"{"a":[1,{"b":"\u00e9\ud83d\ude00"}],"c":false}"#;
        for i in 0..t.len() {
            assert!(Json::parse(&t[..i]).is_none(), "{i}");
        }
    }

    #[test]
    fn what_it_writes_it_can_read() {
        for s in ["", "plain", "q\"b\\s/n\nr\rt\tz\u{1}\u{20ac}\u{1f600}"] {
            assert_eq!(Json::parse(&quote(s)), Some(Json::Str(s.into())));
        }
    }
}
