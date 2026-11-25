//! Watching builds: the runs of GitHub Actions in repositories the user names, and whatever a script
//! tells with `pixel-pet --tell pass`. GitHub is only asked when there is a repository to watch.

use crate::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32};

pub(crate) const MOST: usize = 4;
/// Sent by `--tell` to the pet that runs: wParam = which of `TELLS`.
pub(crate) const WM_TELL: u32 = WM_APP + 6;
pub(crate) const TELLS: [&str; 3] = ["pass", "fail", "start"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum How {
    Waiting, // nothing known yet
    Running,
    Passed,
    Failed,
    Other, // cancelled, skipped
    Unknown, // cannot be asked
}

/// One run of a workflow.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Run {
    id: u64,
    name: String,
    branch: String,
    how: How,
    url: String,
}

// ---- rules (pure, so they can be tested) ----

/// "owner/name", from a link or typed like that.
pub(crate) fn repo_of(t: &str) -> Option<String> {
    let t = t.trim().trim_end_matches('/');
    let rest = t.strip_prefix("https://github.com/").or_else(|| t.strip_prefix("git@github.com:")).unwrap_or(t);
    let mut parts = rest.split(['/', '?', '#']);
    let (owner, name) = (parts.next()?, parts.next()?);
    let name = name.strip_suffix(".git").unwrap_or(name);
    if rest == t && rest.split('/').count() != 2 {
        return None;
    }
    let good = |s: &str| (1..=100).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) && !s.starts_with('.');
    (good(owner) && good(name) && !owner.contains('.')).then(|| format!("{owner}/{name}"))
}

/// A GitHub token, going by its looks.
pub(crate) fn is_token(t: &str) -> bool {
    let t = t.trim();
    (t.starts_with("ghp_") || t.starts_with("github_pat_") || t.starts_with("gho_")) && (20..=300).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The runs in GitHub's answer, newest first.
pub(crate) fn runs(answer: &str) -> Option<Vec<Run>> {
    let j = Json::parse(answer)?;
    let list = j.get("workflow_runs")?.list();
    Some(
        list.iter()
            .filter_map(|r| {
                let how = match (r.text("status"), r.text("conclusion")) {
                    ("completed", "success") => How::Passed,
                    ("completed", "failure" | "timed_out" | "startup_failure") => How::Failed,
                    ("completed", _) => How::Other,
                    _ => How::Running,
                };
                let url = r.text("html_url");
                Some(Run { id: r.get("id")?.num()? as u64, name: r.text("name").to_string(), branch: r.text("head_branch").to_string(), how, url: if url.starts_with("https://github.com/") { url.to_string() } else { String::new() } })
            })
            .collect(),
    )
}

/// What became of the runs since the last look. At the first look nothing is news.
pub(crate) fn news(seen: &mut Option<HashMap<u64, How>>, now: &[Run]) -> Vec<Run> {
    let mut out = Vec::new();
    let first = seen.is_none();
    let s = seen.get_or_insert_with(HashMap::new);
    for r in now.iter().rev() {
        let was = s.insert(r.id, r.how);
        let done = matches!(r.how, How::Passed | How::Failed);
        // one that came and went between two looks is news as well
        if done && !first && was != Some(r.how) {
            out.push(r.clone());
        }
    }
    if s.len() > 500 {
        s.retain(|id, _| now.iter().any(|r| r.id == *id));
    }
    out
}

/// How the repository is: what its newest run says.
pub(crate) fn overall(now: &[Run]) -> How {
    if now.iter().any(|r| r.how == How::Running) {
        return How::Running;
    }
    now.iter().map(|r| r.how).find(|h| *h != How::Other).unwrap_or(How::Waiting)
}

/// Seconds between two looks. Without a token GitHub answers sixty times an hour.
pub(crate) fn pace(repos: usize, token: bool) -> u64 {
    if token { 45 } else { (repos as u64 * 75).max(90) }
}

pub(crate) fn words(repo: &str, r: &Run) -> String {
    let short = repo.rsplit('/').next().unwrap_or(repo);
    let on = if r.branch.is_empty() { String::new() } else { format!(" on {}", r.branch.chars().take(20).collect::<String>()) };
    let name: String = r.name.chars().take(20).collect();
    if r.how == How::Passed { format!("{short}: {name} passed{on}!") } else { format!("{short}: {name} failed{on}.") }
}

pub(crate) fn label(h: How) -> &'static str {
    match h {
        How::Waiting => "-",
        How::Running => "RUNNING",
        How::Passed => "PASSED",
        How::Failed => "FAILED",
        How::Other => "STOPPED",
        How::Unknown => "CANNOT READ",
    }
}

// ---- keeping up ----

static REPOS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// How each repository is, and the page of its newest run.
static STATE: Mutex<Vec<(String, How, String)>> = Mutex::new(Vec::new());
/// What is to be said: passed?, the words.
static NEWS: Mutex<Vec<(bool, String)>> = Mutex::new(Vec::new());
static RUNNING: AtomicBool = AtomicBool::new(false);
static TURN: AtomicU32 = AtomicU32::new(0);

/// The repositories, as the settings file has them.
pub(crate) fn to_text() -> String {
    REPOS.lock().unwrap().join(",")
}

pub(crate) fn from_text(t: &str) {
    if friend::is_friend() {
        return;
    }
    *REPOS.lock().unwrap() = t.split(',').filter_map(repo_of).take(MOST).collect();
    wake();
}

fn wake() {
    TURN.fetch_add(1, Relaxed);
    if !REPOS.lock().unwrap().is_empty() && !RUNNING.swap(true, Relaxed) {
        std::thread::spawn(keep_up);
    }
}

pub(crate) fn add(repo: String) -> bool {
    let mut l = REPOS.lock().unwrap();
    if l.len() >= MOST || l.iter().any(|r| r.eq_ignore_ascii_case(&repo)) {
        return false;
    }
    l.push(repo);
    drop(l);
    wake();
    true
}

pub(crate) fn remove(i: usize) {
    let mut l = REPOS.lock().unwrap();
    if i < l.len() {
        l.remove(i);
    }
    drop(l);
    TURN.fetch_add(1, Relaxed);
}

pub(crate) fn set_token(t: &str) -> bool {
    let ok = secret::set("github", t.trim());
    HAS.store(2, Relaxed);
    TURN.fetch_add(1, Relaxed);
    ok
}

/// Is there a token? Asked once, as it may mean starting a program.
static HAS: AtomicU32 = AtomicU32::new(2);

pub(crate) fn has_token() -> bool {
    if HAS.load(Relaxed) == 2 {
        HAS.store(secret::get("github").is_some() as u32, Relaxed);
    }
    HAS.load(Relaxed) == 1
}

/// Name, how it is, page of the newest run.
pub(crate) fn list() -> Vec<(String, How, String)> {
    let state = STATE.lock().unwrap().clone();
    REPOS.lock().unwrap().iter().map(|r| state.iter().find(|s| s.0 == *r).cloned().unwrap_or((r.clone(), How::Waiting, String::new()))).collect()
}

/// For the tile: the worst of them.
pub(crate) fn summary() -> String {
    let l = list();
    let n = |h: How| l.iter().filter(|r| r.1 == h).count();
    match (l.len(), n(How::Running), n(How::Failed), n(How::Passed)) {
        (0, ..) => "NONE WATCHED".into(),
        (_, r, _, _) if r > 0 => format!("{r} RUNNING"),
        (_, _, f, _) if f > 0 => format!("{f} FAILED"),
        (all, _, _, p) if p == all => "ALL PASSED".into(),
        (all, ..) => format!("{all} WATCHED"),
    }
}

fn keep_up() {
    let mut seen: HashMap<String, Option<HashMap<u64, How>>> = HashMap::new();
    loop {
        let turn = TURN.load(Relaxed);
        let repos = REPOS.lock().unwrap().clone();
        let token = secret::get("github");
        seen.retain(|r, _| repos.contains(r));
        STATE.lock().unwrap().retain(|s| repos.contains(&s.0));
        let mut slow = false;
        for repo in &repos {
            let mut headers = vec![("Accept".to_string(), "application/vnd.github+json".to_string())];
            if let Some(t) = &token {
                headers.push(("Authorization".into(), format!("Bearer {t}")));
            }
            let url = format!("{}/repos/{repo}/actions/runs?per_page=12", net::base("PIXEL_PET_GITHUB", "https://api.github.com"));
            let got = net::ask(&net::Ask { url, headers, most: 2 << 20, secs: 25, ..Default::default() });
            let code = got.as_ref().map_or(0, |a| a.code);
            slow |= code == 403 || code == 429;
            let now = got.ok().filter(|a| a.code == 200).and_then(|a| runs(&a.text()));
            let mut st = STATE.lock().unwrap();
            let i = st.iter().position(|s| s.0 == *repo).unwrap_or_else(|| {
                st.push((repo.clone(), How::Waiting, String::new()));
                st.len() - 1
            });
            match now {
                Some(now) => {
                    st[i].1 = overall(&now);
                    st[i].2 = now.first().map_or(String::new(), |r| r.url.clone());
                    for r in news(seen.entry(repo.clone()).or_default(), &now) {
                        NEWS.lock().unwrap().push((r.how == How::Passed, words(repo, &r)));
                    }
                }
                None if code == 404 || code == 401 => st[i].1 = How::Unknown,
                None => {} // later, perhaps
            }
        }
        let pause = if slow { 900 } else { pace(repos.len(), token.is_some()) };
        for _ in 0..pause {
            std::thread::sleep(Duration::from_secs(1));
            if TURN.load(Relaxed) != turn {
                break;
            }
        }
    }
}

/// Something to say about a build.
pub(crate) fn due() -> Option<(bool, String)> {
    let mut n = NEWS.lock().unwrap();
    // of many at once only the last few matter
    let len = n.len();
    if len > 3 {
        n.drain(..len - 3);
    }
    (!n.is_empty()).then(|| n.remove(0))
}

// ---- being told ----

fn told_path() -> Option<std::path::PathBuf> {
    data_path("pixel-pet-told.txt")
}

/// `pixel-pet --tell pass my tests`: which of `TELLS`, and the words.
pub(crate) fn told_by(args: &[String]) -> Option<(usize, String)> {
    let i = args.iter().position(|a| a == "--tell")?;
    let kind = TELLS.iter().position(|t| args.get(i + 1).is_some_and(|a| a == t))?;
    Some((kind, args[i + 2..].join(" ")))
}

pub(crate) fn clean(words: &str) -> String {
    words.chars().filter(|c| !c.is_control()).take(40).collect::<String>().trim().to_string()
}

/// Leave the words where the pet finds them.
pub(crate) fn leave(words: &str) {
    if let Some(p) = told_path() {
        let _ = std::fs::write(p, clean(words));
    }
}

/// The pet was told.
pub(crate) fn heard(kind: usize) {
    let what = told_path().and_then(|p| {
        let t = std::fs::read_to_string(&p).ok();
        let _ = std::fs::remove_file(p);
        t
    });
    let what = clean(what.as_deref().unwrap_or(""));
    let what = if what.is_empty() { "The build".to_string() } else { what };
    match kind {
        0 => NEWS.lock().unwrap().push((true, format!("{what} passed!"))),
        1 => NEWS.lock().unwrap().push((false, format!("{what} failed."))),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: u64, how: How) -> Run {
        Run { id, name: "build".into(), branch: "main".into(), how, url: String::new() }
    }

    #[test]
    fn repositories_are_found_in_links() {
        for t in ["https://github.com/Own-er/na.me", "https://github.com/Own-er/na.me.git", "https://github.com/Own-er/na.me/actions/runs/1?x=2#y", "Own-er/na.me", " git@github.com:Own-er/na.me.git "] {
            assert_eq!(repo_of(t).as_deref(), Some("Own-er/na.me"), "{t}");
        }
        for t in ["", "a", "https://gitlab.com/a/b", "https://github.com/a", "a/b/c", "a/../b", "../a", "a/b c", "https://evil.com/a/b", "a/.git"] {
            assert_eq!(repo_of(t), None, "{t}");
        }
        assert!(is_token("ghp_abcdefghijklmnopqrstuvwxyz0123456789") && !is_token("ghp_x") && !is_token("hello ghp_abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn githubs_answer_is_understood() {
        let a = r#"{"total_count": 3, "workflow_runs": [
            {"id": 3, "name": "CI", "head_branch": "main", "status": "in_progress", "conclusion": null, "html_url": "https://github.com/a/b/actions/runs/3"},
            {"id": 2, "name": "CI", "head_branch": "x", "status": "completed", "conclusion": "failure", "html_url": "javascript:alert(1)"},
            {"id": 1, "name": "CI", "head_branch": "y", "status": "completed", "conclusion": "success", "html_url": ""}]}"#;
        let r = runs(a).unwrap();
        assert_eq!(r.iter().map(|r| r.how).collect::<Vec<_>>(), [How::Running, How::Failed, How::Passed]);
        assert_eq!((r[0].url.as_str(), r[1].url.as_str()), ("https://github.com/a/b/actions/runs/3", ""));
        assert!(runs("{}").is_none() && runs("<html>").is_none());
        assert_eq!(words("a/b", &r[1]), "b: CI failed on x.");
    }

    #[test]
    fn only_what_is_new_is_told_and_only_once() {
        let mut seen = None;
        assert!(news(&mut seen, &[run(2, How::Running), run(1, How::Failed)]).is_empty(), "old news");
        assert!(news(&mut seen, &[run(2, How::Running), run(1, How::Failed)]).is_empty());
        let n = news(&mut seen, &[run(2, How::Passed), run(1, How::Failed)]);
        assert_eq!(n, [run(2, How::Passed)]);
        assert!(news(&mut seen, &[run(2, How::Passed)]).is_empty());
        // came and went between two looks; a second try of one that failed
        assert_eq!(news(&mut seen, &[run(3, How::Failed)]).len(), 1);
        assert!(news(&mut seen, &[run(3, How::Running)]).is_empty());
        assert_eq!(news(&mut seen, &[run(3, How::Passed)]).len(), 1);
        assert!(news(&mut seen, &[run(4, How::Other)]).is_empty());
    }

    #[test]
    fn a_repository_is_as_its_newest_run() {
        assert_eq!(overall(&[]), How::Waiting);
        assert_eq!(overall(&[run(3, How::Other), run(2, How::Passed), run(1, How::Failed)]), How::Passed);
        assert_eq!(overall(&[run(3, How::Failed), run(2, How::Running)]), How::Running);
        // never more than sixty an hour without a token
        for n in 1..=MOST {
            assert!(3600 / pace(n, false) * n as u64 <= 60, "{n}");
        }
    }

    #[test]
    fn scripts_can_tell() {
        let a = |t: &str| t.split(' ').map(str::to_string).collect::<Vec<_>>();
        assert_eq!(told_by(&a("pixel-pet --tell pass my tests")), Some((0, "my tests".into())));
        assert_eq!(told_by(&a("pixel-pet --tell fail")), Some((1, String::new())));
        assert_eq!(told_by(&a("pixel-pet --tell maybe")), None);
        assert_eq!(told_by(&a("pixel-pet --tell")), None);
        assert_eq!(told_by(&a("pixel-pet")), None);
        assert_eq!(clean(&format!("a\u{7}b\n{}", "x".repeat(99))).len(), 40);
    }
}
