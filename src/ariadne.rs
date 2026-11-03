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
    /// How far an Organize plan has got, while one runs.
    #[serde(default)]
    pub planning: Option<Progress>,
}

/// Files planned so far, out of all of them.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
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

static DETAIL_HIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the detail panel is off: put away from the top bar, or a page (Tidy, Settings) stands
/// in its place, as the window last drew it.
pub fn detail_hidden() -> bool {
    DETAIL_HIDDEN.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_detail_hidden(hidden: bool) {
    DETAIL_HIDDEN.store(hidden, std::sync::atomic::Ordering::Relaxed);
}

/// Start the daemon when it is down, so opening the window is all it takes. `ariadne up` names
/// its unit, so two windows racing here start it once. Found in ~/.local/bin by path, since
/// windows started over D-Bus do not have it on PATH.
pub fn ensure_running() {
    if activity().is_some() {
        return;
    }
    let cli = dirs::executable_dir()
        .map(|dir| dir.join("ariadne"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("ariadne"));
    match std::process::Command::new(cli).arg("up").output() {
        Ok(out) if !out.status.success() => {
            log::warn!("ariadne up: {}", String::from_utf8_lossy(&out.stderr));
        }
        Ok(_) => {}
        Err(err) => log::warn!("ariadne up: {err}"),
    }
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
    request_within(head, body, Duration::from_secs(15))
}

/// The daemon's address; ARIADNE_PORT points the app at a sandbox daemon beside the real one.
fn addr() -> SocketAddr {
    let port = std::env::var("ARIADNE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8199);
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn request_within(head: &str, body: &str, wait: Duration) -> Option<String> {
    let addr = addr();
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(150)).ok()?;
    s.set_read_timeout(Some(wait)).ok()?;
    write!(s, "{head} HTTP/1.0\r\nHost: {addr}\r\n").ok()?;
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

/// What an undo put back. An Organize comes back whole, so `count` can be many files.
#[derive(Clone, Debug, Deserialize)]
pub struct Undone {
    pub count: usize,
    /// The Organize it took back, when the move undone was part of one.
    #[serde(default)]
    pub batch: Option<String>,
}

/// Undo one move by where it landed, else the newest move on the thread. Either way a move
/// made by an Organize brings back its whole batch.
pub fn undo(path: Option<&Path>) -> Option<Undone> {
    let body = match path {
        Some(path) => serde_json::json!({ "path": path }),
        None => serde_json::json!({}),
    };
    serde_json::from_str(&request("POST /undo", &body.to_string())?).ok()
}

/// A file Organize leaves where it is, and why: already there, an empty file, no real guess.
#[derive(Clone, Debug, Deserialize)]
pub struct Left {
    pub path: PathBuf,
    pub why: String,
    /// Other folders the daemon would consider for it, best first, for Send to.
    #[serde(default)]
    pub alternatives: Vec<PathBuf>,
}

/// One file in an Organize plan: ticked when the daemon is sure, and the note to show beside it.
#[derive(Clone, Debug, Deserialize)]
pub struct PlanFile {
    pub path: PathBuf,
    /// The folder it sits in now.
    pub from: PathBuf,
    pub ticked: bool,
    pub share: Option<f32>,
    #[serde(default)]
    pub note: String,
    /// Other folders the daemon would consider for it, best first, for Send to.
    #[serde(default)]
    pub alternatives: Vec<PathBuf>,
    /// The daemon's own tick, kept while the user changes `ticked`: a pre-ticked row the user
    /// unticks is a "no" the daemon remembers; a row it never ticked is not.
    #[serde(skip)]
    pub pre: bool,
}

impl PlanFile {
    /// The user said no to this row: the plan ticked it and the user took the tick off.
    pub fn declined(&self) -> bool {
        self.pre && !self.ticked
    }
}

/// The files that would go to one folder. `new` when the folder does not exist yet; `proposed`
/// is the daemon's own folder, sent back unchanged when the user renames a new one.
#[derive(Clone, Debug, Deserialize)]
pub struct PlanGroup {
    pub folder: PathBuf,
    pub proposed: PathBuf,
    pub new: bool,
    #[serde(default)]
    pub why: String,
    /// What Move does for this group, in one sentence, counting the files the daemon ticked.
    pub summary: String,
    pub files: Vec<PlanFile>,
    /// The group goes along when a folder above it is renamed, as the daemon places it. False
    /// when the user said to keep it where it was planned.
    #[serde(skip, default = "along")]
    pub carry: bool,
}

fn along() -> bool {
    true
}

/// Where Organize would put the picked files, and the files it would leave.
#[derive(Clone, Debug, Deserialize)]
pub struct Plan {
    pub groups: Vec<PlanGroup>,
    #[serde(default)]
    pub left: Vec<Left>,
    /// Copies the user marked for the trash, which go in the Move's batch.
    #[serde(skip)]
    pub trash: Vec<PathBuf>,
}

/// What a plan did: the moves, which undo together as its batch, the copies it trashed, and
/// the files it left.
#[derive(Clone, Debug, Deserialize)]
pub struct Applied {
    pub batch: String,
    pub moved: Vec<Move>,
    #[serde(default)]
    pub trashed: Vec<PathBuf>,
    pub left: Vec<Left>,
}

/// Each file may wait on the embedder and on a remote model, so planning gets minutes.
const ORGANIZE_WAIT: Duration = Duration::from_secs(600);

/// Ask where the picked files would go. Nothing moves; a folder means the files loose in it.
pub fn plan(paths: &[PathBuf]) -> Option<Plan> {
    let body = serde_json::json!({ "paths": paths }).to_string();
    serde_json::from_str(&request_within("POST /organize/plan", &body, ORGANIZE_WAIT)?).ok()
}

/// Move the ticked files of a plan as one batch; unticked ones are remembered as rejected.
pub fn apply_plan(plan: &Plan) -> Option<Applied> {
    let body = apply_body(plan).to_string();
    serde_json::from_str(&request_within("POST /organize/apply", &body, ORGANIZE_WAIT)?).ok()
}

/// The apply request for a plan as the panel holds it: only what the contract asks for.
pub fn apply_body(plan: &Plan) -> serde_json::Value {
    // A group every file was sent away from has nothing to say.
    let groups: Vec<_> = plan
        .groups
        .iter()
        .filter(|g| !g.files.is_empty())
        .map(|g| {
            let files: Vec<_> = g
                .files
                .iter()
                .map(|f| {
                    let mut file = serde_json::json!({ "path": f.path, "ticked": f.ticked });
                    // Only a row the user unticked from ticked carries the flag, never false.
                    if f.declined() {
                        file["declined"] = serde_json::Value::Bool(true);
                    }
                    file
                })
                .collect();
            let mut group = serde_json::json!({
                "folder": g.folder,
                "proposed": g.proposed,
                "new": g.new,
                "files": files,
            });
            // Only a group kept out of a renamed folder above it says so.
            if !g.carry {
                group["carry"] = serde_json::Value::Bool(false);
            }
            group
        })
        .collect();
    let mut body = serde_json::json!({ "groups": groups });
    if !plan.trash.is_empty() {
        body["trash"] = serde_json::json!(plan.trash);
    }
    body
}

/// Put back every move of one Organize.
pub fn undo_batch(batch: &str) -> Option<Undone> {
    let body = serde_json::json!({ "batch": batch }).to_string();
    serde_json::from_str(&request("POST /undo", &body)?).ok()
}

/// Move a waiting file to a folder the user picked instead of the suggestion.
pub fn move_to(path: &Path, folder: &Path) -> bool {
    let body = serde_json::json!({ "path": path, "folder": folder }).to_string();
    request("POST /accept", &body).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_undo_reply_counts_the_files_back() {
        let reply = r#"{"undone": [{"from": "/a", "to": "/b"}], "count": 138, "batch": "organize-1"}"#;
        let undone: Undone = serde_json::from_str(reply).unwrap();
        assert_eq!(undone.count, 138);
        let none: Undone = serde_json::from_str(r#"{"undone": [], "count": 0, "batch": null}"#).unwrap();
        assert_eq!(none.count, 0);
    }

    #[test]
    fn activity_carries_plan_progress_while_a_plan_runs() {
        let busy: Activity = serde_json::from_str(
            r#"{"line": "planning 120 of 246", "busy": true, "planning": {"done": 120, "total": 246}}"#,
        )
        .unwrap();
        assert_eq!(busy.planning, Some(Progress { done: 120, total: 246 }));
        let idle: Activity = serde_json::from_str(r#"{"line": "up to date", "busy": false, "planning": null}"#).unwrap();
        assert_eq!(idle.planning, None);
    }
}
