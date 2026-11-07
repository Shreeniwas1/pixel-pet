//! OpenCode token usage (v1 `message` table and v2 `session_message` table).
//!
//! OpenCode keeps its history in a SQLite database. Windows ships SQLite as `winsqlite3.dll`, so
//! we load that at run time instead of bundling a database engine: no extra dependency, no extra
//! exe size. Which tables and JSON fields hold the numbers (assistant messages, `tokens.input`,
//! `tokens.output`, `tokens.reasoning`, `tokens.cache.read/write`, the v1/v2 table split) was
//! learned from tokscale by Junho Yeo (MIT, https://github.com/junhoyeo/tokscale); the code is ours.
//! Only numbers are selected, never message text, and the database is opened read-only.

use crate::prelude::*;
use crate::tokens::Counts;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type Db = *mut c_void;
type Stmt = *mut c_void;

struct Api {
    open_v2: unsafe extern "C" fn(*const c_char, *mut Db, c_int, *const c_char) -> c_int,
    close: unsafe extern "C" fn(Db) -> c_int,
    busy_timeout: unsafe extern "C" fn(Db, c_int) -> c_int,
    prepare: unsafe extern "C" fn(Db, *const c_char, c_int, *mut Stmt, *mut *const c_char) -> c_int,
    bind_int64: unsafe extern "C" fn(Stmt, c_int, i64) -> c_int,
    step: unsafe extern "C" fn(Stmt) -> c_int,
    column_int64: unsafe extern "C" fn(Stmt, c_int) -> i64,
    column_text: unsafe extern "C" fn(Stmt, c_int) -> *const u8,
    finalize: unsafe extern "C" fn(Stmt) -> c_int,
    #[allow(dead_code)] // only the tests write databases
    exec: unsafe extern "C" fn(Db, *const c_char, *const c_void, *const c_void, *mut *mut c_char) -> c_int,
}

unsafe fn api() -> Option<&'static Api> {
    static API: std::sync::OnceLock<Option<Api>> = std::sync::OnceLock::new();
    API.get_or_init(|| {
        let lib = LoadLibraryW(wide("winsqlite3.dll").as_ptr());
        if lib.is_null() {
            return None;
        }
        macro_rules! f {
            ($n:expr) => {
                std::mem::transmute(GetProcAddress(lib, concat!($n, "\0").as_ptr())?)
            };
        }
        Some(Api {
            open_v2: f!("sqlite3_open_v2"),
            close: f!("sqlite3_close"),
            busy_timeout: f!("sqlite3_busy_timeout"),
            prepare: f!("sqlite3_prepare_v2"),
            bind_int64: f!("sqlite3_bind_int64"),
            step: f!("sqlite3_step"),
            column_int64: f!("sqlite3_column_int64"),
            column_text: f!("sqlite3_column_text"),
            finalize: f!("sqlite3_finalize"),
            exec: f!("sqlite3_exec"),
        })
    })
    .as_ref()
}

struct Conn {
    api: &'static Api,
    db: Db,
}

impl Conn {
    unsafe fn open(path: &Path, read_only: bool) -> Option<Conn> {
        let api = api()?;
        let p = CString::new(path.to_str()?).ok()?;
        let mut db: Db = null_mut();
        // 1 = READONLY, 2 = READWRITE, 4 = CREATE
        let rc = (api.open_v2)(p.as_ptr(), &mut db, if read_only { 1 } else { 6 }, null());
        if rc != 0 || db.is_null() {
            if !db.is_null() {
                (api.close)(db);
            }
            return None;
        }
        (api.busy_timeout)(db, 1500); // OpenCode may be writing right now
        Some(Conn { api, db })
    }

    #[allow(dead_code)]
    unsafe fn exec(&self, sql: &str) -> bool {
        let s = CString::new(sql).unwrap();
        (self.api.exec)(self.db, s.as_ptr(), null(), null(), null_mut()) == 0
    }

    /// Run `sql` with one integer parameter; call `row` for each result row (id, ints...).
    unsafe fn rows(&self, sql: &str, param: i64, mut row: impl FnMut(&str, &[i64])) -> bool {
        let s = CString::new(sql).unwrap();
        let mut st: Stmt = null_mut();
        if (self.api.prepare)(self.db, s.as_ptr(), -1, &mut st, null_mut()) != 0 || st.is_null() {
            return false; // table or column does not exist in this database version
        }
        (self.api.bind_int64)(st, 1, param);
        loop {
            match (self.api.step)(st) {
                100 => {
                    let t = (self.api.column_text)(st, 0);
                    let id = if t.is_null() { String::new() } else { CStr::from_ptr(t as *const c_char).to_string_lossy().into_owned() };
                    let v: Vec<i64> = (1..7).map(|i| (self.api.column_int64)(st, i)).collect();
                    row(&id, &v);
                }
                101 => break,
                _ => {
                    (self.api.finalize)(st);
                    return false;
                }
            }
        }
        (self.api.finalize)(st);
        true
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        unsafe {
            (self.api.close)(self.db);
        }
    }
}

/// Columns, in order: id, created (ms), input, output, reasoning, cache read, cache write.
fn query(table: &str, assistant_filter: &str) -> String {
    format!(
        "SELECT id, COALESCE(json_extract(data,'$.time.created'), time_created, 0), \
         COALESCE(json_extract(data,'$.tokens.input'),0), COALESCE(json_extract(data,'$.tokens.output'),0), \
         COALESCE(json_extract(data,'$.tokens.reasoning'),0), COALESCE(json_extract(data,'$.tokens.cache.read'),0), \
         COALESCE(json_extract(data,'$.tokens.cache.write'),0) \
         FROM {table} WHERE time_updated >= ?1 AND {assistant_filter} AND json_extract(data,'$.tokens') IS NOT NULL"
    )
}

/// One assistant message as OpenCode recorded it.
#[derive(Clone, Copy)]
pub struct Msg {
    pub day: i64,
    pub c: Counts,
}

/// Read every assistant message touched since `since_ms` from one database into `out`
/// (keyed so a message that is rewritten while it streams replaces its earlier numbers).
pub fn read_db(path: &Path, since_ms: i64, utc_to_local: i64, out: &mut std::collections::HashMap<String, Msg>) -> bool {
    unsafe {
        let Some(conn) = Conn::open(path, true) else { return false };
        let mut ok = false;
        for (table, filter) in [("message", "json_extract(data,'$.role') = 'assistant'"), ("session_message", "type = 'assistant'")] {
            let key = table.chars().next().unwrap();
            ok |= conn.rows(&query(table, filter), since_ms, |id, v| {
                let day = (v[0] / 60_000 + utc_to_local).div_euclid(1440);
                let c = Counts {
                    input: v[1].max(0) as u64,
                    output: (v[2].max(0) + v[3].max(0)) as u64, // reasoning is billed as output
                    cache_read: v[4].max(0) as u64,
                    cache_write: v[5].max(0) as u64,
                };
                out.insert(format!("{key}:{id}"), Msg { day, c });
            });
        }
        ok
    }
}

/// `opencode.db` and channel builds such as `opencode-stable.db`, in the default data folder
/// (or wherever `OPENCODE_DB` points).
pub fn databases() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = std::env::var_os("OPENCODE_DB").map(PathBuf::from).filter(|p| p.is_file()) {
        v.push(p);
    }
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else { return v };
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(home).join(".local").join("share"));
    if let Ok(rd) = std::fs::read_dir(base.join("opencode")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let ok = n == "opencode.db" || (n.starts_with("opencode-") && n.ends_with(".db"));
            if ok && !v.contains(&e.path()) {
                v.push(e.path());
            }
        }
    }
    v
}

pub fn days_of(msgs: &std::collections::HashMap<String, Msg>) -> BTreeMap<i64, Counts> {
    let mut d: BTreeMap<i64, Counts> = BTreeMap::new();
    for m in msgs.values() {
        let e = d.entry(m.day).or_default();
        e.input += m.c.input;
        e.output += m.c.output;
        e.cache_write += m.c.cache_write;
        e.cache_read += m.c.cache_read;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn temp_db(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("pp-oc-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    // 2026-11-07 10:00:00 UTC in ms
    const T: i64 = 1_794_045_600_000;

    #[test]
    fn reads_v1_and_v2_assistant_messages_and_skips_the_rest() {
        let p = temp_db("both");
        unsafe {
            let c = Conn::open(&p, false).expect("winsqlite3 available");
            assert!(c.exec("CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT)"));
            assert!(c.exec("CREATE TABLE session_message (id TEXT, session_id TEXT, type TEXT, time_created INTEGER, time_updated INTEGER, data TEXT)"));
            let a = format!(r#"{{"role":"assistant","time":{{"created":{T}}},"tokens":{{"input":100,"output":20,"reasoning":5,"cache":{{"read":900,"write":40}}}}}}"#);
            let u = r#"{"role":"user"}"#;
            let v2 = format!(r#"{{"time":{{"created":{T}}},"tokens":{{"input":7,"output":3,"cache":{{"read":1,"write":2}}}}}}"#);
            assert!(c.exec(&format!("INSERT INTO message VALUES ('m1','s',{T},{T},'{a}')")));
            assert!(c.exec(&format!("INSERT INTO message VALUES ('m2','s',{T},{T},'{u}')")));
            assert!(c.exec(&format!("INSERT INTO session_message VALUES ('x1','s','assistant',{T},{T},'{v2}')")));
            assert!(c.exec(&format!("INSERT INTO session_message VALUES ('x2','s','user',{T},{T},'{v2}')")));
        }
        let mut out = HashMap::new();
        assert!(read_db(&p, 0, 0, &mut out));
        assert_eq!(out.len(), 2, "only the two assistant messages");
        let m1 = out["m:m1"];
        assert_eq!(m1.c, Counts { input: 100, output: 25, cache_write: 40, cache_read: 900 });
        assert_eq!(out["s:x1"].c.output, 3);
        // an old row is not re-read once the watermark moves past it
        let mut later = HashMap::new();
        assert!(read_db(&p, T + 1, 0, &mut later));
        assert!(later.is_empty());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn a_database_without_those_tables_is_just_not_ok() {
        let p = temp_db("empty");
        unsafe {
            let c = Conn::open(&p, false).unwrap();
            assert!(c.exec("CREATE TABLE other (x INTEGER)"));
        }
        assert!(!read_db(&p, 0, 0, &mut HashMap::new()));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn days_are_summed() {
        let mut m = HashMap::new();
        let c = Counts { input: 1, output: 2, cache_write: 3, cache_read: 4 };
        m.insert("a".to_string(), Msg { day: 5, c });
        m.insert("b".to_string(), Msg { day: 5, c });
        assert_eq!(days_of(&m)[&5], Counts { input: 2, output: 4, cache_write: 6, cache_read: 8 });
    }
}
