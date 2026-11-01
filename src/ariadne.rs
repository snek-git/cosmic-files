//! The local ariadne daemon: semantic search and the Inbox. Every call returns None when the
//! daemon is not running, so search falls back to plain filename matching and the Inbox is empty.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ADDR: &str = "127.0.0.1:8199";
const RESULTS: usize = 40;

#[derive(Deserialize)]
struct Hit {
    path: PathBuf,
}

#[derive(Deserialize)]
struct Hits {
    hits: Vec<Hit>,
}

/// Another folder the vote considered, with its share.
#[derive(Clone, Debug, Deserialize)]
pub struct Alternative {
    pub folder: PathBuf,
    pub share: f32,
}

/// A file the Inbox shows: a suggestion waiting for Move or Keep, a file being read, or a file
/// moved on its own.
#[derive(Clone, Debug, Deserialize)]
pub struct InboxEntry {
    pub state: String,
    pub path: PathBuf,
    #[serde(default)]
    pub folder: PathBuf,
    pub share: Option<f32>,
    pub by: Option<String>,
    pub from: Option<PathBuf>,
    #[serde(default)]
    pub at: f64,
    /// Where it came from: a website's host or the drop zone's name.
    pub source: Option<String>,
    /// A copy of this file that is already filed.
    pub dup_of: Option<PathBuf>,
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
}

/// One knot on the undo thread.
#[derive(Clone, Debug, Deserialize)]
pub struct Move {
    pub id: u64,
    pub at: f64,
    pub from: PathBuf,
    pub to: PathBuf,
    /// "ariadne" or "you"
    pub by: String,
    pub share: Option<f32>,
    #[serde(default)]
    pub learned: bool,
}

impl Move {
    pub fn by_ariadne(&self) -> bool {
        self.by != "you"
    }

    pub fn age(&self) -> Duration {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        Duration::from_secs_f64((now - self.at).max(0.0))
    }

    /// Recent enough to leave a ghost where it left and a `new` tag where it landed.
    pub fn fresh(&self) -> bool {
        self.age() < Duration::from_secs(24 * 3600)
    }
}

/// What the status line says.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Activity {
    pub line: String,
    #[serde(default)]
    pub busy: bool,
}

/// Everything the window shows from the daemon, refetched when its version moves.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub inbox: Vec<InboxEntry>,
    pub moves: Vec<Move>,
    pub proposals: Vec<Proposal>,
}

/// A change ariadne would make to tidy up: a misfiled file, a duplicate, two folders that hold
/// the same thing, or files that want a folder of their own.
#[derive(Clone, Debug, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub kind: String,
    pub confidence: f32,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    pub from: Option<PathBuf>,
    pub to: Option<PathBuf>,
    pub name: Option<String>,
    /// Why Opus said yes, in one sentence.
    #[serde(default)]
    pub reason: Option<String>,
}

/// A path as the window writes it: under home it starts with `~`.
pub fn short(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rel) => format!("~/{}", rel.display()),
        None => path.display().to_string(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

impl Proposal {
    fn first(&self) -> PathBuf {
        self.paths.first().cloned().unwrap_or_default()
    }

    fn last(&self) -> PathBuf {
        self.paths.last().cloned().unwrap_or_default()
    }

    /// The folder a misfiled file or the extra copy sits in.
    fn parent_of(path: &Path) -> PathBuf {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    /// What a misfiled idea moves: the file's name, or `3 files` for a group.
    fn moved(&self) -> String {
        match self.paths.len() {
            1 => file_name(&self.first()),
            n => format!("{n} files"),
        }
    }

    /// What the change is about, without its verb: the file, `3 files`, or the two folders.
    pub fn subject(&self) -> String {
        match self.kind.as_str() {
            "misfiled" => self.moved(),
            "duplicate" => file_name(&self.last()),
            "merge" => format!("{} + {}", folder_name(&self.first()), folder_name(&self.last())),
            _ => format!("{} files", self.paths.len()),
        }
    }

    /// Where things go, under the title.
    pub fn route(&self) -> String {
        match self.kind.as_str() {
            "misfiled" => format!(
                "{} → {}",
                short(&Self::parent_of(&self.first())),
                short(&self.to.clone().unwrap_or_default())
            ),
            "duplicate" => format!(
                "from {} · original stays in {}",
                short(&Self::parent_of(&self.last())),
                short(&Self::parent_of(&self.first()))
            ),
            "merge" => {
                let to = self.to.clone().unwrap_or_else(|| self.last());
                let from = self.paths.iter().find(|p| **p != to).cloned().unwrap_or_default();
                format!("{} → {}", short(&from), short(&to))
            }
            _ => format!(
                "for {} files in {}",
                self.paths.len(),
                short(&self.from.clone().unwrap_or_default())
            ),
        }
    }

    /// The card's sentence when ariadne notices this inside a folder.
    pub fn noticed(&self) -> String {
        let sure = format!("{:.0}%", self.confidence * 100.0);
        match self.kind.as_str() {
            "misfiled" => format!(
                "{} {} in {} · {sure}",
                self.moved(),
                if self.paths.len() == 1 { "belongs" } else { "belong" },
                folder_name(&self.to.clone().unwrap_or_default())
            ),
            "duplicate" => format!(
                "{} is also in {} · {sure}",
                file_name(&self.last()),
                short(&Self::parent_of(&self.first()))
            ),
            "merge" => format!(
                "{} and {} hold the same thing · {sure}",
                short(&self.first()),
                short(&self.last())
            ),
            _ => format!("{} files are about one thing · new folder?", self.paths.len()),
        }
    }

    pub fn action(&self) -> &'static str {
        match self.kind.as_str() {
            "misfiled" => "Move",
            "duplicate" => "Trash",
            "merge" => "Merge",
            _ => "Create",
        }
    }

    pub fn involves(&self, path: &Path) -> bool {
        self.paths.iter().any(|p| p == path)
    }
}

#[derive(Deserialize)]
struct Proposals {
    #[serde(default)]
    proposals: Vec<Proposal>,
}

/// Tidy ideas, optionally only those touching one folder; empty when the daemon is down.
pub fn proposals(folder: Option<&Path>) -> Vec<Proposal> {
    let query = match folder {
        Some(folder) => format!("/proposals?folder={}", encode(&folder.to_string_lossy())),
        None => "/proposals".to_string(),
    };
    get(&query)
        .and_then(|body| serde_json::from_str::<Proposals>(&body).ok())
        .map(|p| p.proposals)
        .unwrap_or_default()
}

/// Make the change; `name` names the new folder for a group.
pub fn apply(id: &str, name: Option<&str>) -> bool {
    let body = serde_json::json!({ "id": id, "name": name }).to_string();
    request("POST /proposals/apply", &body).is_some()
}

/// Not now: quiet for 30 days.
pub fn skip(id: &str) -> bool {
    let body = serde_json::json!({ "id": id }).to_string();
    request("POST /proposals/skip", &body).is_some()
}

/// Never: no more cards for this folder.
pub fn mute(folder: &Path) -> bool {
    let body = serde_json::json!({ "folder": folder }).to_string();
    request("POST /mute", &body).is_some()
}

/// What ariadne has to say about one item in a folder view.
#[derive(Clone, Debug)]
pub enum Mark {
    /// being decided right now
    Reading,
    /// waiting for Move or Keep
    Waiting(InboxEntry),
    /// a download that is already filed elsewhere
    Copy(InboxEntry),
    /// moved here by ariadne recently
    New(Move),
}

static SNAPSHOT: Mutex<Option<Arc<Snapshot>>> = Mutex::new(None);

pub fn snapshot() -> Arc<Snapshot> {
    SNAPSHOT.lock().unwrap().clone().unwrap_or_default()
}

pub fn set_snapshot(snapshot: Arc<Snapshot>) {
    *SNAPSHOT.lock().unwrap() = Some(snapshot);
}

static TIDY_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the Tidy list is open, as the window last drew it.
pub fn tidy_open() -> bool {
    TIDY_OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_tidy_open(open: bool) {
    TIDY_OPEN.store(open, std::sync::atomic::Ordering::Relaxed);
}

static SIDE_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a page (Tidy, Settings) stands beside the items, as the window last drew it.
pub fn side_open() -> bool {
    SIDE_OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_side_open(open: bool) {
    SIDE_OPEN.store(open, std::sync::atomic::Ordering::Relaxed);
}

/// The inbox and the thread, or None when the daemon is down.
pub fn fetch() -> Option<Snapshot> {
    let inbox: Inbox = serde_json::from_str(&get("/inbox")?).ok()?;
    let moves: Moves = get("/moves?limit=40")
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or(Moves { moves: Vec::new() });
    *INBOX.lock().unwrap() = Some(inbox.items.iter().map(|e| (e.path.clone(), e.clone())).collect());
    Some(Snapshot {
        inbox: inbox.items,
        moves: moves.moves,
        proposals: proposals(None),
    })
}

pub fn activity() -> Option<Activity> {
    serde_json::from_str(&get("/activity")?).ok()
}

#[derive(Deserialize)]
struct Moves {
    #[serde(default)]
    moves: Vec<Move>,
}

impl Snapshot {
    pub fn mark(&self, path: &Path) -> Option<Mark> {
        if let Some(e) = self.inbox.iter().find(|e| e.path == path) {
            return match e.state.as_str() {
                "reading" => Some(Mark::Reading),
                "suggested" if e.dup_of.is_some() => Some(Mark::Copy(e.clone())),
                "suggested" => Some(Mark::Waiting(e.clone())),
                _ => None,
            };
        }
        self.moves
            .iter()
            .rev()
            .find(|m| m.to == path && m.by_ariadne() && m.fresh())
            .map(|m| Mark::New(m.clone()))
    }

    /// Files ariadne moved out of `dir` recently that still exist where they went.
    /// One ghost per file still where ariadne put it, newest move first.
    pub fn ghosts(&self, dir: &Path) -> Vec<Move> {
        let mut seen = std::collections::HashSet::new();
        self.moves
            .iter()
            .rev()
            .filter(|m| m.by_ariadne() && m.fresh() && m.from.parent() == Some(dir) && m.to.exists())
            .filter(|m| seen.insert(m.to.clone()))
            .cloned()
            .collect()
    }

    pub fn waiting(&self) -> usize {
        self.inbox.iter().filter(|e| e.suggested()).count()
    }

    /// Files waiting anywhere under `dir`.
    pub fn waiting_in(&self, dir: &Path) -> usize {
        self.inbox
            .iter()
            .filter(|e| e.suggested() && e.path != dir && e.path.starts_with(dir))
            .count()
    }

    /// Tidy ideas that touch a file anywhere under `dir`.
    pub fn ideas_in(&self, dir: &Path) -> usize {
        self.proposals
            .iter()
            .filter(|p| p.paths.iter().any(|x| x != dir && x.starts_with(dir)))
            .count()
    }
}

/// A folder as the window names it: its last component.
pub fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

impl InboxEntry {
    pub fn suggested(&self) -> bool {
        self.state == "suggested"
    }

    /// The line under the file name: where it goes, or where it went and from where.
    pub fn caption(&self) -> String {
        let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string()).unwrap_or_default();
        let short = |p: &Path| p.to_string_lossy().replacen(&home, "~", 1);
        let sure = match (self.share, &self.by) {
            (Some(s), Some(by)) => format!("  ·  {:.0}% sure, by {by}", s * 100.0),
            _ => String::new(),
        };
        if self.suggested() {
            format!("belongs in {}{sure}", short(&self.folder))
        } else {
            let from = self.from.as_deref().and_then(Path::parent).map(short).unwrap_or_default();
            format!("moved here from {from}{sure}")
        }
    }
}

#[derive(Deserialize)]
struct Inbox {
    version: u64,
    #[serde(default)]
    items: Vec<InboxEntry>,
}

/// The entries of the last Inbox scan, so rows can show their caption and buttons.
static INBOX: Mutex<Option<HashMap<PathBuf, InboxEntry>>> = Mutex::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InboxVerb {
    Move,
    Keep,
    Undo,
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

fn request(head: &str, body: &str) -> Option<String> {
    let addr: SocketAddr = ADDR.parse().ok()?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(150)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok()?;
    write!(s, "{head} HTTP/1.0\r\nHost: {ADDR}\r\n").ok()?;
    if !body.is_empty() {
        write!(s, "Content-Type: application/json\r\nContent-Length: {}\r\n", body.len()).ok()?;
    }
    write!(s, "\r\n{body}").ok()?;
    let mut buf = String::new();
    s.read_to_string(&mut buf).ok()?;
    let (head, body) = buf.split_once("\r\n\r\n")?;
    if !head.split_whitespace().nth(1).is_some_and(|code| code == "200") {
        log::warn!("ariadne: {head} {body}");
        return None;
    }
    Some(body.to_string())
}

fn get(path_and_query: &str) -> Option<String> {
    request(&format!("GET {path_and_query}"), "")
}

/// `like:<path>` asks for files like that one; anything else is a search by meaning below `under`.
pub fn is_like(term: &str) -> bool {
    term.starts_with("like:")
}

pub fn search(term: &str, under: &Path) -> Option<Vec<PathBuf>> {
    let query = if let Some(path) = term.strip_prefix("like:") {
        format!("/similar?n={RESULTS}&path={}", encode(path.trim()))
    } else {
        format!(
            "/search?n={RESULTS}&q={}&under={}",
            encode(term),
            encode(&under.to_string_lossy())
        )
    };
    let hits: Hits = serde_json::from_str(&get(&query)?).ok()?;
    Some(hits.hits.into_iter().map(|h| h.path).collect())
}

/// Changes whenever the Inbox does; cheap enough to poll.
pub fn inbox_version() -> Option<u64> {
    let v: Inbox = serde_json::from_str(&get("/inbox?version_only=1")?).ok()?;
    Some(v.version)
}

/// The Inbox, newest first. Also remembered for `inbox_entry`.
pub fn inbox() -> Vec<InboxEntry> {
    let items = get("/inbox")
        .and_then(|body| serde_json::from_str::<Inbox>(&body).ok())
        .map(|i| i.items)
        .unwrap_or_default();
    *INBOX.lock().unwrap() = Some(items.iter().map(|e| (e.path.clone(), e.clone())).collect());
    items
}

pub fn inbox_entry(path: &Path) -> Option<InboxEntry> {
    INBOX.lock().unwrap().as_ref()?.get(path).cloned()
}

/// Move a suggestion to its folder, keep it where it is, or undo a move made on its own.
pub fn act(verb: InboxVerb, path: &Path) -> bool {
    let endpoint = match verb {
        InboxVerb::Move => "/accept",
        InboxVerb::Keep => "/keep",
        InboxVerb::Undo => "/undo",
    };
    let body = serde_json::json!({ "path": path }).to_string();
    request(&format!("POST {endpoint}"), &body).is_some()
}

#[derive(Deserialize)]
struct Peek {
    excerpt: Option<String>,
    facts: Option<String>,
}

/// One line about what a file is: its facts when ariadne knows them, else the start of its text.
pub fn peek(path: &Path) -> Option<String> {
    let peek: Peek =
        serde_json::from_str(&get(&format!("/peek?path={}", encode(&path.to_string_lossy())))?)
            .ok()?;
    peek.facts
        .or(peek.excerpt)
        .filter(|line| !line.trim().is_empty())
}

#[derive(Deserialize)]
struct Summary {
    text: Option<String>,
}

/// The folder's one-line description, the text Jev routes with.
pub fn summary(folder: &Path) -> Option<String> {
    let s: Summary =
        serde_json::from_str(&get(&format!("/summary?path={}", encode(&folder.to_string_lossy())))?)
            .ok()?;
    s.text
}

pub fn set_summary(folder: &Path, text: &str) -> bool {
    let body = serde_json::json!({ "path": folder, "text": text }).to_string();
    request("POST /summary", &body).is_some()
}

/// Disk use under a folder, three levels deep; None while the daemon is still measuring it.
fn sizes(under: &Path) -> Option<crate::map::Node> {
    let query = format!("/sizes?under={}&depth=3", encode(&under.to_string_lossy()));
    serde_json::from_str(&get(&query)?).ok()
}

/// The map's world: the places files live and arrive in, under one Home node. All of home holds
/// caches and games that take minutes to measure and say nothing about where files belong.
pub fn map_tree() -> Option<crate::map::Node> {
    let home = dirs::home_dir()?;
    let kids: Vec<_> = [
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::picture_dir(),
        dirs::desktop_dir(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|dir| sizes(&dir))
    .collect();
    (!kids.is_empty()).then(|| crate::map::Node {
        path: home,
        name: "Home".to_string(),
        size: kids.iter().map(|k| k.size).sum(),
        kids,
    })
}

/// Meaning on the map: arrows for misfiled files, joined borders for folders to merge.
pub fn overlays(proposals: &[Proposal]) -> Vec<crate::map::Overlay> {
    proposals
        .iter()
        .filter_map(|p| match p.kind.as_str() {
            "misfiled" => Some(crate::map::Overlay::Arrow {
                from: p.paths.first()?.clone(),
                to: p.to.clone()?,
            }),
            "merge" => Some(crate::map::Overlay::Merge {
                a: p.paths.first()?.clone(),
                b: p.paths.last()?.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// Undo the newest move on the thread.
pub fn undo_last() -> bool {
    request("POST /undo", "{}").is_some()
}

/// Move a waiting file to a folder the user picked instead of the suggestion.
pub fn move_to(path: &Path, folder: &Path) -> bool {
    let body = serde_json::json!({ "path": path, "folder": folder }).to_string();
    request("POST /accept", &body).is_some()
}
