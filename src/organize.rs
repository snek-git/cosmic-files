//! Organize as a plan: the state the side panel and the wide sheet share, the counts and
//! sentences both show, and the two views. Nothing here moves a file; Move hands the plan as
//! edited to the daemon.
//!
//! While the panel is open nothing reflows under the pointer: every destination is one card in
//! an order fixed when the plan arrives, its files keep their rows, and an unticked row stays
//! where it was, dimmed. Groups that end up going to one folder (a rename onto another card's
//! folder) are drawn as one card.
//!
//! The keyboard works the panel through a cursor of its own, not widget focus: the arrows walk
//! every card heading, file and staying file; Space ticks; F2 names the card's new folder.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit, Wrapping};
use cosmic::iced::advanced::widget::operation::{Outcome, Scrollable};
use cosmic::iced::advanced::widget::{Id, Operation};
use cosmic::iced::widget::container;
use cosmic::iced::widget::scrollable::AbsoluteOffset;
use cosmic::iced::{Alignment, Background, Border, Length, Rectangle, Vector};
use cosmic::{Element, theme, widget};

use crate::app::Message;
use crate::ariadne::{Left, Plan, PlanFile, PlanGroup, Progress, folder_name, short};
use crate::ariadne_ui::{Glyph, Ticked, clip_middle, flat, glyph, header, more, name_field, quiet_icon, sheet, tick};
use crate::fl;
use crate::palette::{body, ladder, secondary};
use crate::yorha::{Tone, micro};

/// What folds open: a card in the panel, a reason in the panel's Stay part, or a group of the
/// sheet (None is the files that stay).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Fold {
    Card(usize),
    Stay(String),
    Group(Option<usize>),
}

/// What waits behind "Discard changes?": closing the panel, or a new Organize of these paths.
#[derive(Clone, Debug)]
pub enum Pending {
    Close,
    Organize(Vec<PathBuf>),
}

/// Where the panel's cursor can stand: a card's heading (by its lead group), one file of a
/// card, or a file that stays (by its place in the plan's `left`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    Group(usize),
    Row(usize, usize),
    Left(usize),
}

/// The keys the panel takes while a plan is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKey {
    Up,
    Down,
    /// To the previous or next card's heading, past a long card's files.
    PrevCard,
    NextCard,
    Space,
    Rename,
    Enter,
    /// Send to for the cursor's file, or its whole card on a heading.
    Send,
    /// Mark or unmark a staying copy for the trash.
    Trash,
    /// Jump to MOVE, ASK or STAY.
    Part(usize),
    /// Take back the rename that joined this card to another folder.
    Unrename,
    /// Answer "Also move papers/cv to invoice/cv?".
    Carry(bool),
}

/// What a key asks of the app beyond the plan itself.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Done,
    /// Put the keyboard in this card's name field.
    Name(usize),
    /// Send to the chosen folder, or open the folder picker for any other.
    Send(Sending, PathBuf),
    Browse(Sending),
}

/// What Send to sends: one file, or every file of a card (by its lead group).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sending {
    File(PathBuf),
    Card(usize),
}

/// Where the panel's list was measured: its three parts, the cursor's row, and the height of
/// the list on screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measured {
    sections: [Option<f32>; 3],
    cursor: Option<(f32, f32)>,
    view: f32,
}

/// One card: the groups that go to one folder, drawn under the lead's heading and name.
#[derive(Clone, Debug, PartialEq)]
struct Card {
    lead: usize,
    part: usize,
    members: Vec<usize>,
}

/// The three parts of a plan, in the order the panel lists them.
const MOVING: usize = 0;
const ASKING: usize = 1;
const STAYING: usize = 2;

/// Names a panel card shows before its `+N more`.
const CARD_NAMES: usize = 3;
/// A name longer than this may be cut short, so it gets its whole self on hover.
const LONG_NAME: usize = 28;
/// A note this short sits beside its file's name; a longer one goes under it.
const NOTE_BESIDE: usize = 14;
/// The width of one character of the 12px text face, for fields sized to their name.
const CHAR_WIDTH: f32 = 7.2;
/// Characters of a new folder's parent shown in front of its name field, so the field and its
/// NEW tag still fit the panel.
const PARENT_CHARS: usize = 18;
/// Folders Send to offers before Browse.
const SEND_CHOICES: usize = 8;
/// The sheet's last column, room for "Send to".
const SEND_WIDTH: f32 = 56.0;
/// How the daemon starts the reason for a copy (filed elsewhere, loose elsewhere, or another
/// in this selection): such a row can go to the trash with the Move.
const COPY_WHY: &str = "a copy";

static LIST: LazyLock<Id> = LazyLock::new(|| Id::new("organize-list"));
static MOVE: LazyLock<Id> = LazyLock::new(|| Id::new("organize-move"));
static CURSOR: LazyLock<Id> = LazyLock::new(|| Id::new("organize-cursor"));
static SECTIONS: LazyLock<[Id; 3]> = LazyLock::new(|| {
    [
        Id::new("organize-moving"),
        Id::new("organize-asking"),
        Id::new("organize-staying"),
    ]
});

/// The name field of a card's new folder, for F2.
pub fn name_id(g: usize) -> Id {
    Id::new(format!("organize-name-{g}"))
}

/// The plan for one Organize, as the panel and the sheet edit it.
#[derive(Default)]
pub struct Organize {
    /// Counts every ask, so a plan that comes back for an older pick is dropped.
    pub asked: u64,
    /// The picked items, and when they were picked, while the plan is on its way.
    picked: Vec<PathBuf>,
    started: Option<Instant>,
    /// How far the daemon has got with the plan, as the status poll last said.
    progress: Option<Progress>,
    pub plan: Option<Plan>,
    /// A picked folder had folders in it, which Organize leaves alone.
    subfolders: bool,
    /// New folder names as edited, by group.
    names: HashMap<usize, String>,
    /// Groups whose ticks, name or files changed: closing the panel asks first, and their
    /// heading is redone from the daemon's.
    touched: HashSet<usize>,
    /// The wide sheet is open over the window.
    pub sheet: bool,
    open: HashSet<Fold>,
    /// Every group once, with the part it shows in, fixed when the plan arrives.
    order: Vec<(usize, usize)>,
    /// How many files the daemon ticked in each group, and how many it had: the counts its
    /// sentence and its reason say.
    said: Vec<usize>,
    sizes: Vec<usize>,
    /// Staying copies marked for the trash; they go with the Move, in its batch.
    trash: HashSet<PathBuf>,
    /// For a renamed new folder (by group): whether the folders planned inside it go along.
    /// Not there yet: not asked, and they go along, as the daemon would take them.
    carry: HashMap<usize, bool>,
    /// The panel's keyboard cursor, and whether the list should scroll to it once measured.
    cursor: Option<Stop>,
    follow: bool,
    /// The card whose name field has the keyboard, so the arrows and Escape leave it be.
    pub editing: Option<usize>,
    /// A name as it was before its last edit, for Ctrl+Z in its field.
    name_before: Option<(usize, String)>,
    /// The sheet's filter on file names, destinations and reasons.
    filter: String,
    /// Whose Send to list is open, and the choice the keyboard stands on in it.
    pub send_menu: Option<Sending>,
    send_pick: Option<usize>,
    /// What the folder picker (Browse) is choosing a destination for.
    pub sending: Option<Sending>,
    /// "Discard changes?" is up, and what happens if the answer is yes.
    pub confirm: Option<Pending>,
    /// How far the panel's list is scrolled, and where its three parts start in it.
    scroll_y: f32,
    sections: [Option<f32>; 3],
}

/// The numbers the summary line, the meter and the tabs show.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    pub total: usize,
    pub moving: usize,
    pub asking: usize,
    pub staying: usize,
    /// Staying copies marked for the trash.
    pub trashing: usize,
    /// Folders with a ticked file, and how many of them are new.
    pub folders: usize,
    pub new: usize,
}

impl Organize {
    /// A fresh ask for these picked items (none: the panel is closed).
    pub fn asking(asked: u64, picked: Vec<PathBuf>) -> Self {
        Self {
            asked,
            started: (!picked.is_empty()).then(Instant::now),
            picked,
            ..Self::default()
        }
    }

    /// A picked folder or file is gone from disk (trashed, moved away): the plan is about
    /// files that are no longer there.
    pub fn gone(&self) -> bool {
        self.plan.is_some() && self.picked.iter().any(|p| !p.exists())
    }

    /// A plan was asked for and has not come back yet.
    pub fn planning(&self) -> bool {
        self.plan.is_none() && self.started.is_some()
    }

    /// The daemon's count of files planned so far, while this plan is on its way.
    pub fn progressed(&mut self, progress: Option<Progress>) {
        if self.planning() {
            self.progress = progress;
        }
    }

    /// Take the daemon's plan. Each group's sure files go first, then its guesses by share; the
    /// groups the daemon ticked anything in come first, under Move, the rest under Ask. That
    /// order holds until the panel closes. Every group of the sheet starts open, and the cursor
    /// stands on the first card.
    pub fn set_plan(&mut self, mut plan: Plan, subfolders: bool) {
        for group in &mut plan.groups {
            group.files.sort_by(|a, b| {
                b.ticked.cmp(&a.ticked).then(
                    b.share
                        .unwrap_or(0.0)
                        .partial_cmp(&a.share.unwrap_or(0.0))
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
            });
            for file in &mut group.files {
                file.pre = file.ticked;
            }
        }
        self.said = plan.groups.iter().map(|g| g.files.iter().filter(|f| f.pre).count()).collect();
        self.sizes = plan.groups.iter().map(|g| g.files.len()).collect();
        let sure = |g: &PlanGroup| g.files.iter().any(|f| f.pre);
        self.order = (0..plan.groups.len())
            .filter(|&g| sure(&plan.groups[g]))
            .map(|g| (g, MOVING))
            .chain(
                (0..plan.groups.len())
                    .filter(|&g| !sure(&plan.groups[g]))
                    .map(|g| (g, ASKING)),
            )
            .collect();
        self.open = (0..plan.groups.len())
            .map(|g| Fold::Group(Some(g)))
            .chain([Fold::Group(None)])
            .collect();
        self.subfolders = subfolders;
        self.plan = Some(plan);
        self.cursor = self.stops().into_iter().next();
    }

    fn group(&self, g: usize) -> Option<&PlanGroup> {
        self.plan.as_ref().and_then(|p| p.groups.get(g))
    }

    pub fn tick(&mut self, g: usize, f: usize, ticked: bool) {
        if let Some(file) = self
            .plan
            .as_mut()
            .and_then(|p| p.groups.get_mut(g))
            .and_then(|group| group.files.get_mut(f))
        {
            file.ticked = ticked;
            self.touched.insert(g);
        }
    }

    /// Tick or untick every file of a card: all the groups going to its folder.
    pub fn tick_card(&mut self, lead: usize, ticked: bool) {
        for g in self.members(lead) {
            if let Some(group) = self.plan.as_mut().and_then(|p| p.groups.get_mut(g)) {
                for file in &mut group.files {
                    file.ticked = ticked;
                }
                self.touched.insert(g);
            }
        }
    }

    /// Name a card's new folder: every new group on the card takes the name.
    pub fn rename(&mut self, lead: usize, name: String) {
        let was = self.name(lead);
        if was != name {
            self.name_before = Some((lead, was));
        }
        self.editing = Some(lead);
        for g in self.members(lead) {
            if self.group(g).is_some_and(|group| group.new) {
                self.names.insert(g, name.clone());
                self.touched.insert(g);
            }
        }
    }

    /// Ctrl+Z in a name field: the name goes back to what it was before its last edit, once.
    /// Nothing to take back: nothing happens.
    pub fn undo_name(&mut self) {
        if let Some((lead, was)) = self.name_before.take() {
            self.rename(lead, was);
            self.name_before = None;
        }
    }

    /// Send one file to a folder the user picked: into the group already going there, or a new
    /// group of its own at the end of the list. It goes ticked; the user chose it. A file that
    /// was staying can be sent too. Nothing happens when it is already going there.
    pub fn send_to(&mut self, path: &Path, folder: PathBuf) -> bool {
        let Some(plan) = &self.plan else { return false };
        let at = plan.groups.iter().enumerate().find_map(|(g, group)| {
            Some((g, group.files.iter().position(|file| file.path == path)?))
        });
        let left = plan.left.iter().position(|l| l.path == path);
        if at.is_none() && left.is_none() {
            return false;
        }
        let target = (0..plan.groups.len())
            .filter(|&g| !plan.groups[g].files.is_empty())
            .find(|&g| self.shown(g) == folder);
        if at.is_some_and(|(from, _)| target == Some(from)) {
            return false;
        }
        let before = self.cursor_index();
        let plan = self.plan.as_mut().expect("checked above");
        let mut file = match (at, left) {
            (Some((from, f)), _) => {
                self.touched.insert(from);
                plan.groups[from].files.remove(f)
            }
            (None, Some(i)) => {
                let left = plan.left.remove(i);
                self.trash.remove(&left.path);
                PlanFile {
                    from: left.path.parent().map(Path::to_path_buf).unwrap_or_default(),
                    path: left.path,
                    ticked: true,
                    share: None,
                    note: String::new(),
                    alternatives: left.alternatives,
                    pre: false,
                }
            }
            (None, None) => unreachable!("checked above"),
        };
        file.ticked = true;
        file.pre = false;
        file.note = fl!("organize-sent-note");
        let to = match target {
            Some(g) => {
                plan.groups[g].files.push(file);
                g
            }
            None => {
                plan.groups.push(PlanGroup {
                    proposed: folder.clone(),
                    folder,
                    new: false,
                    why: fl!("organize-why-yours"),
                    summary: String::new(),
                    files: vec![file],
                    carry: true,
                });
                let g = plan.groups.len() - 1;
                self.order.push((g, MOVING));
                self.open.insert(Fold::Group(Some(g)));
                g
            }
        };
        self.touched.insert(to);
        self.keep_cursor(before);
        true
    }

    /// The files Send to sends: the one file, or every file of the card.
    pub fn paths(&self, sending: &Sending) -> Vec<PathBuf> {
        match sending {
            Sending::File(path) => vec![path.clone()],
            Sending::Card(lead) => self
                .card_of(*lead)
                .map(|card| self.rows(&card))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(g, f)| self.file(g, f).map(|f| f.path.clone()))
                .collect(),
        }
    }

    /// Send to, for one file or a whole card. A card's files go one by one into one group.
    pub fn send(&mut self, sending: &Sending, folder: PathBuf) -> bool {
        let mut sent = false;
        for path in self.paths(sending) {
            sent |= self.send_to(&path, folder.clone());
        }
        sent
    }

    /// The folders Send to offers, in an order that holds for every file: the plan's own
    /// folders as its cards run, then the daemon's other ideas for this file (or the card's
    /// files, the most shared first), then the home for its kind of file. Never the folder it
    /// goes to now or sits in.
    pub fn send_choices(&self, sending: &Sending) -> Vec<PathBuf> {
        let Some(plan) = &self.plan else {
            return Vec::new();
        };
        let paths = self.paths(sending);
        let here = match sending {
            Sending::File(path) => plan.groups.iter().enumerate().find_map(|(g, group)| {
                group.files.iter().any(|f| f.path == *path).then(|| self.shown(g))
            }),
            Sending::Card(lead) => Some(self.shown(*lead)),
        };
        let mut ideas: Vec<(PathBuf, usize)> = Vec::new();
        for path in &paths {
            let alternatives = plan
                .groups
                .iter()
                .flat_map(|g| g.files.iter())
                .find(|f| f.path == *path)
                .map(|f| f.alternatives.as_slice())
                .or_else(|| plan.left.iter().find(|l| l.path == *path).map(|l| l.alternatives.as_slice()))
                .unwrap_or_default();
            for folder in alternatives {
                match ideas.iter_mut().find(|(f, _)| f == folder) {
                    Some((_, n)) => *n += 1,
                    None => ideas.push((folder.clone(), 1)),
                }
            }
        }
        // Stable: the most shared first, then as the daemon ranked them.
        ideas.sort_by(|a, b| b.1.cmp(&a.1));
        let parent = paths.first().and_then(|p| p.parent()).map(Path::to_path_buf);
        let mut folders: Vec<PathBuf> = Vec::new();
        let all = self
            .cards()
            .into_iter()
            .map(|card| self.shown(card.lead))
            .chain(ideas.into_iter().map(|(folder, _)| folder))
            .chain(paths.first().and_then(|p| type_home(p)));
        for folder in all {
            if Some(&folder) != here.as_ref() && Some(&folder) != parent.as_ref() && !folders.contains(&folder) {
                folders.push(folder);
            }
            if folders.len() == SEND_CHOICES {
                break;
            }
        }
        folders
    }

    /// Open Send to for a file or a card. Opened by key, the keyboard stands on its first
    /// choice; opened by a click, on none until an arrow is pressed.
    pub fn open_send(&mut self, sending: Option<Sending>, by_key: bool) {
        self.send_menu = sending;
        self.send_pick = by_key.then_some(0);
    }

    /// Mark a staying copy for the trash, or take the mark back.
    pub fn toggle_trash(&mut self, path: &Path) {
        if !self.trash.remove(path) {
            self.trash.insert(path.to_path_buf());
        }
    }

    fn is_copy(left: &Left) -> bool {
        left.why.starts_with(COPY_WHY)
    }

    /// The plan has edits that closing the panel would lose.
    pub fn dirty(&self) -> bool {
        self.plan.is_some() && (!self.touched.is_empty() || !self.trash.is_empty())
    }

    pub fn fold(&mut self, fold: Fold) {
        if !self.open.remove(&fold) {
            self.open.insert(fold);
        }
    }

    fn is_open(&self, fold: &Fold) -> bool {
        self.open.contains(fold)
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// The filter is in a file's name, where it goes, or why.
    fn matches(&self, path: &Path, dest: &str, why: &str) -> bool {
        let filter = self.filter.trim().to_lowercase();
        filter.is_empty()
            || [folder_name(path), dest.to_string(), why.to_string()]
                .iter()
                .any(|s| s.to_lowercase().contains(&filter))
    }

    pub fn scrolled(&mut self, y: f32) {
        self.scroll_y = y;
    }

    /// Take a measure of the list. After the cursor moved, say where to scroll so it shows.
    pub fn measured(&mut self, m: Measured) -> Option<AbsoluteOffset<Option<f32>>> {
        self.sections = m.sections;
        if !std::mem::take(&mut self.follow) {
            return None;
        }
        let (y, h) = m.cursor?;
        let heading = matches!(self.cursor, Some(Stop::Group(_)));
        // A card's heading out of view comes to the top, so its files show under it; a file
        // comes just into view.
        let to = if y < self.scroll_y || (heading && y + h > self.scroll_y + m.view) {
            (y - 8.0).max(0.0)
        } else if y + h > self.scroll_y + m.view {
            y + h - m.view + 8.0
        } else {
            return None;
        };
        self.scroll_y = to;
        Some(AbsoluteOffset { x: None, y: Some(to) })
    }

    /// Where a tab scrolls the panel's list to, once the list has been measured.
    pub fn jump(&self, part: usize) -> Option<AbsoluteOffset<Option<f32>>> {
        let y = self.sections.get(part).copied().flatten()?;
        Some(AbsoluteOffset {
            x: None,
            y: Some(y),
        })
    }

    pub fn list_id() -> Id {
        LIST.clone()
    }

    /// The part of the list at its top: the tab drawn in the one dark.
    fn current(&self) -> usize {
        let mut current = MOVING;
        for (part, start) in self.sections.iter().enumerate() {
            if start.is_some_and(|y| y <= self.scroll_y + 4.0) {
                current = part;
            }
        }
        current
    }

    /// The last part of a new group's folder: as edited, else the daemon's own.
    pub fn name(&self, g: usize) -> String {
        self.names
            .get(&g)
            .cloned()
            .unwrap_or_else(|| self.group(g).map(|group| folder_name(&group.folder)).unwrap_or_default())
    }

    /// Why a new folder's name can't be used, if it can't: empty, a slash, a dot or two, a
    /// leading dot (a hidden folder), or what no filesystem takes (NUL, over 255 bytes).
    pub fn name_error(&self, g: usize) -> Option<String> {
        if !self.group(g)?.new {
            return None;
        }
        let name = self.name(g);
        let name = name.trim();
        if name.is_empty() {
            Some(fl!("organize-name-empty"))
        } else if name.contains('/') {
            Some(fl!("organize-name-slash"))
        } else if name == "." || name == ".." {
            Some(fl!("organize-name-dots"))
        } else if name.starts_with('.') {
            Some(fl!("organize-name-hidden"))
        } else if name.contains('\0') || name.len() > 255 {
            Some(fl!("organize-name-bad"))
        } else {
            None
        }
    }

    /// A folder Move would make has a name it can't take.
    pub fn blocked(&self) -> bool {
        let Some(plan) = &self.plan else { return false };
        plan.groups.iter().enumerate().any(|(g, group)| {
            group.new && group.files.iter().any(|f| f.ticked) && self.name_error(g).is_some()
        })
    }

    /// Move can go: a plan, something ticked or marked for the trash, and every new name
    /// usable.
    pub fn can_move(&self) -> bool {
        let c = self.counts();
        self.plan.is_some() && c.moving + c.trashing > 0 && !self.blocked()
    }

    /// The folder Move sends for a group: a renamed new folder is its parent plus the new name.
    /// A name that can't be used keeps the daemon's folder (and blocks Move).
    pub fn folder(&self, g: usize) -> PathBuf {
        let Some(group) = self.group(g) else {
            return PathBuf::new();
        };
        match group.folder.parent() {
            Some(parent) if group.new && self.name_error(g).is_none() => {
                parent.join(self.name(g).trim())
            }
            _ => group.folder.clone(),
        }
    }

    /// Where a group's files land, as the daemon will place them: a group the user did not
    /// rename sits under any renamed folder above it (papers/cv follows papers to records/cv).
    pub fn shown(&self, g: usize) -> PathBuf {
        let Some(group) = self.group(g) else {
            return PathBuf::new();
        };
        let own = self.folder(g);
        if own != group.proposed {
            return own;
        }
        match self.carried_by(g) {
            Some((proposed, folder)) => match own.strip_prefix(proposed) {
                Ok(rest) if rest.as_os_str().is_empty() => folder,
                Ok(rest) => folder.join(rest),
                Err(_) => own,
            },
            None => own,
        }
    }

    /// The renamed folder a group sits in, as the daemon's named_as picks it (the longest), when
    /// the user did not rename the group itself: its proposed path and its new path.
    fn renamed_above(&self, g: usize) -> Option<(usize, &PathBuf, PathBuf)> {
        let plan = self.plan.as_ref()?;
        let group = plan.groups.get(g)?;
        if self.folder(g) != group.proposed {
            return None;
        }
        (0..plan.groups.len())
            .filter(|&h| h != g)
            .map(|h| (h, &plan.groups[h].proposed, self.folder(h)))
            .filter(|(_, proposed, folder)| *proposed != folder && group.proposed.starts_with(proposed))
            .max_by_key(|(_, proposed, _)| proposed.components().count())
    }

    /// The renamed folder that takes a group along with it, unless the user said to keep it.
    fn carried_by(&self, g: usize) -> Option<(&PathBuf, PathBuf)> {
        let (h, proposed, folder) = self.renamed_above(g)?;
        (self.carry.get(&h) != Some(&false)).then_some((proposed, folder))
    }

    /// "Also move papers/cv to invoice/cv?" for a renamed card: the first folder inside it the
    /// rename would take along, where it would go, and how many more, until it is answered.
    fn carry_question(&self, lead: usize) -> Option<(PathBuf, PathBuf, usize)> {
        let plan = self.plan.as_ref()?;
        let members = self.members(lead);
        let renamed = members.iter().copied().find(|&h| {
            !self.carry.contains_key(&h) && plan.groups[h].new && self.folder(h) != plan.groups[h].proposed
        })?;
        let mut inside: Vec<usize> = (0..plan.groups.len())
            .filter(|&g| !plan.groups[g].files.is_empty())
            .filter(|&g| self.renamed_above(g).is_some_and(|(h, _, _)| h == renamed))
            .collect();
        // The nearest folder is the one named: papers/cv before papers/cv/others.
        inside.sort_by_key(|&g| plan.groups[g].proposed.components().count());
        let &first = inside.first()?;
        Some((plan.groups[first].proposed.clone(), self.shown(first), inside.len() - 1))
    }

    /// The answer to "Also move …?" on a card: the renamed folder takes them along, or not.
    pub fn answer_carry(&mut self, lead: usize, carry: bool) {
        let Some(plan) = &self.plan else { return };
        let renamed: Vec<usize> = self
            .members(lead)
            .into_iter()
            .filter(|&h| plan.groups[h].new && self.folder(h) != plan.groups[h].proposed)
            .collect();
        for h in renamed {
            self.carry.insert(h, carry);
            self.touched.insert(h);
        }
    }

    /// Take back the renames on a card: each new folder gets the daemon's name again, which
    /// also splits a card that a rename had joined to another folder.
    pub fn unrename(&mut self, lead: usize) {
        self.editing = None;
        for g in self.members(lead) {
            if self.names.remove(&g).is_some() {
                self.carry.remove(&g);
            }
        }
        self.cursor = Some(Stop::Group(lead));
    }

    /// A card a rename joined to another folder, which Undo rename can split again.
    fn merged_by_rename(&self, card: &Card) -> bool {
        card.members.len() > 1 && card.members.iter().any(|g| self.names.contains_key(g))
    }

    /// A group's reason, its count of files brought up to date ("4 files named boxlab" after
    /// one was sent away says 3).
    fn why(&self, g: usize) -> String {
        let Some(group) = self.group(g) else {
            return String::new();
        };
        let was = self.sizes.get(g).copied().unwrap_or(group.files.len());
        recount(&group.why, was, group.files.len()).unwrap_or_else(|| group.why.clone())
    }

    /// The card's folder is not on disk yet.
    fn is_new(&self, lead: usize) -> bool {
        self.group(lead).is_some_and(|group| group.new) && !self.shown(lead).is_dir()
    }

    /// The plan as the views hold it, ready for the daemon: each group's folder as named, a
    /// group kept out of a renamed folder above it marked so, and the copies to trash.
    pub fn edited(&self) -> Option<Plan> {
        let mut plan = self.plan.clone()?;
        for (g, group) in plan.groups.iter_mut().enumerate() {
            group.folder = self.folder(g);
            group.carry = self
                .renamed_above(g)
                .is_none_or(|(h, _, _)| self.carry.get(&h) != Some(&false));
        }
        plan.trash = plan.left.iter().filter(|l| self.trash.contains(&l.path)).map(|l| l.path.clone()).collect();
        Some(plan)
    }

    /// The cards in their fixed order: groups with files that go to one folder share a card.
    /// The lead is the group whose name is being typed, else the first new one, else the first.
    fn cards(&self) -> Vec<Card> {
        let Some(plan) = &self.plan else {
            return Vec::new();
        };
        let mut cards: Vec<(PathBuf, Card)> = Vec::new();
        for &(g, part) in &self.order {
            if plan.groups[g].files.is_empty() {
                continue;
            }
            let shown = self.shown(g);
            match cards.iter_mut().find(|(folder, _)| *folder == shown) {
                Some((_, card)) => card.members.push(g),
                None => cards.push((shown, Card { lead: g, part, members: vec![g] })),
            }
        }
        cards
            .into_iter()
            .map(|(_, mut card)| {
                let lead = card
                    .members
                    .iter()
                    .copied()
                    .find(|&g| self.editing == Some(g))
                    .or_else(|| card.members.iter().copied().find(|&g| plan.groups[g].new))
                    .unwrap_or(card.lead);
                if lead != card.lead {
                    card.part = self.order.iter().find(|(g, _)| *g == lead).map_or(card.part, |(_, p)| *p);
                    card.lead = lead;
                }
                card
            })
            .collect()
    }

    /// The card a group is drawn on, by its lead.
    fn card_of(&self, g: usize) -> Option<Card> {
        self.cards().into_iter().find(|card| card.members.contains(&g))
    }

    /// The groups drawn on a group's card.
    fn members(&self, g: usize) -> Vec<usize> {
        self.card_of(g).map_or_else(Vec::new, |card| card.members)
    }

    /// A card's files, the lead's first, as (group, file).
    fn rows(&self, card: &Card) -> Vec<(usize, usize)> {
        let mut members = card.members.clone();
        members.sort_by_key(|&g| g != card.lead);
        members
            .into_iter()
            .flat_map(|g| (0..self.group(g).map_or(0, |group| group.files.len())).map(move |f| (g, f)))
            .collect()
    }

    fn file(&self, g: usize, f: usize) -> Option<&PlanFile> {
        self.group(g).and_then(|group| group.files.get(f))
    }

    /// A card's tick: all its files ticked, none, or some.
    fn card_state(&self, card: &Card) -> Ticked {
        let rows = self.rows(card);
        let ticked = rows.iter().filter(|(g, f)| self.file(*g, *f).is_some_and(|f| f.ticked)).count();
        match ticked {
            0 => Ticked::Off,
            n if n == rows.len() => Ticked::On,
            _ => Ticked::Some,
        }
    }

    pub fn counts(&self) -> Counts {
        let Some(plan) = &self.plan else {
            return Counts::default();
        };
        let trashing = plan.left.iter().filter(|l| self.trash.contains(&l.path)).count();
        let mut c = Counts {
            staying: plan.left.len() - trashing,
            trashing,
            ..Counts::default()
        };
        for card in self.cards() {
            let rows = self.rows(&card);
            let ticked = rows.iter().filter(|(g, f)| self.file(*g, *f).is_some_and(|f| f.ticked)).count();
            c.moving += ticked;
            c.asking += rows.len() - ticked;
            if ticked > 0 {
                c.folders += 1;
                c.new += usize::from(self.is_new(card.lead));
            }
        }
        c.total = c.moving + c.asking + c.staying + c.trashing;
        c
    }

    /// The one folder every file sits in now, when there is one.
    fn source(&self) -> Option<PathBuf> {
        let plan = self.plan.as_ref()?;
        let mut froms = plan
            .groups
            .iter()
            .flat_map(|g| g.files.iter().map(|f| f.from.clone()))
            .chain(plan.left.iter().filter_map(|l| l.path.parent().map(Path::to_path_buf)));
        let first = froms.next()?;
        froms.all(|f| f == first).then_some(first)
    }

    /// The line over the meter: how many files move, how many to ask about, and how many stay.
    fn summary(&self) -> String {
        let c = self.counts();
        let said = if c.moving == 0 {
            fl!(
                "organize-summary-none",
                total = c.total,
                asking = c.asking,
                staying = c.staying
            )
        } else {
            fl!(
                "organize-summary",
                total = c.total,
                moving = c.moving,
                folders = c.folders,
                new = c.new,
                asking = c.asking,
                staying = c.staying
            )
        };
        if c.trashing > 0 {
            fl!("organize-summary-trash", said = said, n = c.trashing)
        } else {
            said
        }
    }

    /// Where the files come from, and that folders inside the picked ones were left alone.
    fn origin(&self) -> Option<String> {
        match (self.source(), self.subfolders) {
            (Some(source), false) => Some(fl!("organize-all-from", folder = short(&source))),
            (Some(source), true) => Some(fl!("organize-all-from-flat", folder = short(&source))),
            (None, true) => Some(fl!("organize-no-subfolders")),
            (None, false) => None,
        }
    }

    /// What Move does with a card: the daemon's sentence while it fits, with the count, the
    /// folder and the unticked tail brought up to date; else one built from the ticks now.
    pub fn sentence(&self, lead: usize) -> String {
        let Some(card) = self.card_of(lead) else {
            return String::new();
        };
        if card.members.len() == 1 {
            // As it came, unless a folder above it was renamed: then its folder is redone.
            if !self.touched.contains(&lead)
                && let Some(group) = self.group(lead)
                && !group.summary.is_empty()
                && self.shown(lead) == group.folder
            {
                return group.summary.clone();
            }
            if let Some(said) = self.adapted(lead) {
                return said;
            }
        }
        let rows = self.rows(&card);
        let ticked = rows.iter().filter(|(g, f)| self.file(*g, *f).is_some_and(|f| f.ticked)).count();
        let unticked = rows.len() - ticked;
        let photos = card
            .members
            .iter()
            .all(|&g| self.group(g).is_some_and(|group| group.why.starts_with("Photos")));
        let noun = |n: usize| {
            if photos {
                fl!("organize-photos", n = n)
            } else {
                fl!("organize-files", n = n)
            }
        };
        let folder = home_relative(&self.shown(lead));
        let new = self.is_new(lead);
        // A new folder renamed onto one that is already there joins it.
        let joins = !new && card.members.iter().any(|&g| self.group(g).is_some_and(|group| group.new));
        if ticked == 0 {
            let n = rows.len();
            return if new {
                fl!("organize-say-maybe-make", folder = folder, n = n, noun = noun(n))
            } else if joins {
                fl!("organize-say-maybe-join", folder = folder, n = n, noun = noun(n))
            } else {
                fl!("organize-say-maybe-move", folder = folder, n = n, noun = noun(n))
            };
        }
        let said = if new {
            fl!("organize-say-make", folder = folder, n = ticked, noun = noun(ticked))
        } else if joins {
            fl!("organize-say-join", folder = folder, n = ticked, noun = noun(ticked))
        } else {
            fl!("organize-say-move", folder = folder, n = ticked, noun = noun(ticked))
        };
        if unticked > 0 {
            fl!("organize-say-unticked", said = said, n = unticked)
        } else {
            said
        }
    }

    /// The daemon's sentence for an edited group, kept where the edit did not reach it: the
    /// count of ticked files (after ticks or sends), the folder after a rename, and the unticked
    /// tail are redone.
    /// None when the sentence can't be brought up to date that way.
    fn adapted(&self, g: usize) -> Option<String> {
        let group = self.group(g)?;
        let pre = *self.said.get(g)?;
        let ticked = group.files.iter().filter(|f| f.ticked).count();
        let unticked = group.files.len() - ticked;
        // Files sent in from elsewhere make the daemon's words wrong (its dates, its "named
        // varpet"): only a count can be redone, so the sentence is built anew.
        let sent = fl!("organize-sent-note");
        if pre == 0
            || ticked == 0
            || group.summary.is_empty()
            || group.new != self.is_new(g)
            || group.files.iter().any(|f| f.note == sent)
        {
            return None;
        }
        // The daemon's own tail ("; 22 more unticked") is redone below.
        let base = match group.summary.rsplit_once("; ") {
            Some((head, tail)) if tail.ends_with(" more unticked") => head,
            _ => group.summary.as_str(),
        };
        let mut said = recount(base, pre, ticked)?;
        let (was, now) = (home_relative(&group.folder), home_relative(&self.shown(g)));
        if was != now {
            if !said.contains(&was) {
                return None;
            }
            said = said.replace(&was, &now);
        }
        Some(if unticked > 0 {
            fl!("organize-say-unticked", said = said, n = unticked)
        } else {
            said
        })
    }

    /// The files that stay, by reason, the commonest reason first, as places in `left`.
    fn staying(&self) -> Vec<(String, Vec<usize>)> {
        let Some(plan) = &self.plan else {
            return Vec::new();
        };
        let mut by: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, left) in plan.left.iter().enumerate() {
            match by.iter_mut().find(|(why, _)| *why == left.why) {
                Some((_, files)) => files.push(i),
                None => by.push((left.why.clone(), vec![i])),
            }
        }
        by.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        by
    }

    /// The cards of one part, in their fixed order.
    fn part(&self, part: usize) -> Vec<Card> {
        self.cards().into_iter().filter(|card| card.part == part).collect()
    }

    /// Every place the cursor can stand, top to bottom: each card's heading and all its files
    /// (shown or behind `+N more`), then every file that stays.
    pub fn stops(&self) -> Vec<Stop> {
        let mut stops = Vec::new();
        for part in [MOVING, ASKING] {
            for card in self.part(part) {
                stops.push(Stop::Group(card.lead));
                stops.extend(self.rows(&card).into_iter().map(|(g, f)| Stop::Row(g, f)));
            }
        }
        for (_, files) in self.staying() {
            stops.extend(files.into_iter().map(Stop::Left));
        }
        stops
    }

    fn cursor_index(&self) -> Option<usize> {
        let cursor = self.cursor.as_ref()?;
        self.stops().iter().position(|s| s == cursor)
    }

    /// After rows came or went, keep the cursor where it was, or on what took that place.
    fn keep_cursor(&mut self, before: Option<usize>) {
        let stops = self.stops();
        if self.cursor.as_ref().is_some_and(|c| stops.contains(c)) {
            return;
        }
        self.cursor = before
            .map(|i| i.min(stops.len().saturating_sub(1)))
            .and_then(|i| stops.get(i).cloned())
            .or_else(|| stops.first().cloned());
    }

    /// Put the cursor on a place (a click on a tick does).
    pub fn point(&mut self, stop: Stop) {
        self.editing = None;
        self.cursor = Some(stop);
    }

    pub fn point_card(&mut self, g: usize) {
        if let Some(card) = self.card_of(g) {
            self.point(Stop::Group(card.lead));
        }
    }

    /// The card the cursor is on, by its lead.
    fn cursor_card(&self) -> Option<usize> {
        match self.cursor.clone()? {
            Stop::Group(g) | Stop::Row(g, _) => Some(self.card_of(g)?.lead),
            Stop::Left(_) => None,
        }
    }

    /// The keys worth a reminder where the cursor stands, the most useful first; Move's last.
    pub fn hint(&self) -> Vec<&'static str> {
        let mut keys = Vec::with_capacity(5);
        if self.send_menu.is_some() {
            keys.extend(["up down pick", "enter send", "esc close"]);
            return keys;
        }
        match self.cursor.clone() {
            Some(Stop::Left(i)) => {
                if self.plan.as_ref().and_then(|p| p.left.get(i)).is_some_and(Self::is_copy) {
                    keys.push("delete trash");
                }
                keys.push("s send");
            }
            Some(Stop::Group(g) | Stop::Row(g, _)) => {
                keys.push("space tick");
                let card = self.card_of(g);
                if card.as_ref().is_some_and(|c| self.carry_question(c.lead).is_some()) {
                    keys.push("y n answer");
                }
                if card.as_ref().is_some_and(|c| self.merged_by_rename(c)) {
                    keys.push("u undo rename");
                }
                if card.as_ref().is_some_and(|c| self.group(c.lead).is_some_and(|l| l.new)) {
                    keys.push("f2 name");
                }
                keys.push("s send");
            }
            None => {}
        }
        keys.extend(["1 2 3 parts", "ctrl+t table"]);
        keys
    }

    /// One key of the panel. With Send to open, the arrows walk its choices and Enter takes
    /// one; Browse is the last.
    pub fn key(&mut self, key: PanelKey) -> Answer {
        if let Some(sending) = self.send_menu.clone() {
            let choices = self.send_choices(&sending);
            match key {
                PanelKey::Up => self.send_pick = Some(self.send_pick.map_or(0, |i| i.saturating_sub(1))),
                PanelKey::Down => {
                    self.send_pick = Some(self.send_pick.map_or(0, |i| (i + 1).min(choices.len())));
                }
                PanelKey::Enter | PanelKey::Space => {
                    // Nothing picked yet: Enter takes the first choice.
                    self.send_menu = None;
                    return match choices.get(self.send_pick.unwrap_or(0)) {
                        Some(folder) => Answer::Send(sending, folder.clone()),
                        None => Answer::Browse(sending),
                    };
                }
                PanelKey::Send => self.send_menu = None,
                _ => {}
            }
            return Answer::Done;
        }
        let stops = self.stops();
        if stops.is_empty() {
            return Answer::Done;
        }
        match key {
            PanelKey::Send => {
                let sending = match self.cursor.clone() {
                    Some(Stop::Group(lead)) => Some(Sending::Card(lead)),
                    Some(Stop::Row(g, f)) => self.file(g, f).map(|f| Sending::File(f.path.clone())),
                    Some(Stop::Left(i)) => self.plan.as_ref().and_then(|p| p.left.get(i)).map(|l| Sending::File(l.path.clone())),
                    None => None,
                };
                self.open_send(sending, true);
            }
            PanelKey::Trash => {
                if let Some(Stop::Left(i)) = self.cursor
                    && let Some(left) = self.plan.as_ref().and_then(|p| p.left.get(i))
                    && Self::is_copy(left)
                {
                    let path = left.path.clone();
                    self.toggle_trash(&path);
                }
            }
            PanelKey::Part(part) => {
                // The first place of that part, at the top of the list.
                let first = stops.iter().find(|s| match s {
                    Stop::Group(g) => self.card_of(*g).is_some_and(|c| c.part == part),
                    Stop::Row(..) => false,
                    Stop::Left(_) => part == STAYING,
                });
                // The app scrolls the part's start to the top, as its tab does.
                if let Some(stop) = first.cloned() {
                    self.cursor = Some(stop);
                    self.unfold_to_cursor();
                }
            }
            PanelKey::Unrename => {
                if let Some(lead) = self.cursor_card() {
                    self.unrename(lead);
                }
            }
            PanelKey::Carry(carry) => {
                if let Some(lead) = self.cursor_card()
                    && self.carry_question(lead).is_some()
                {
                    self.answer_carry(lead, carry);
                }
            }
            _ => return self.walk(key, &stops),
        }
        Answer::Done
    }

    /// The cursor's own keys: the arrows, the page keys, Space, Enter and F2.
    fn walk(&mut self, key: PanelKey, stops: &[Stop]) -> Answer {
        match key {
            PanelKey::Up | PanelKey::Down => {
                let next = match self.cursor_index() {
                    None => 0,
                    Some(i) if key == PanelKey::Down => (i + 1).min(stops.len() - 1),
                    Some(i) => i.saturating_sub(1),
                };
                self.cursor = Some(stops[next].clone());
                self.unfold_to_cursor();
                self.follow = true;
            }
            PanelKey::PrevCard | PanelKey::NextCard => {
                let at = self.cursor_index().unwrap_or(0);
                let heading = |s: &Stop| matches!(s, Stop::Group(_));
                let next = if key == PanelKey::NextCard {
                    (at + 1..stops.len()).find(|&i| heading(&stops[i]))
                } else {
                    (0..at).rev().find(|&i| heading(&stops[i]))
                };
                if let Some(i) = next {
                    self.cursor = Some(stops[i].clone());
                    self.follow = true;
                }
            }
            PanelKey::Space => {
                match self.cursor.clone() {
                    Some(Stop::Group(lead)) => {
                        if let Some(card) = self.card_of(lead) {
                            let on = self.card_state(&card) == Ticked::On;
                            self.tick_card(lead, !on);
                        }
                    }
                    Some(Stop::Row(g, f)) => {
                        if let Some(ticked) = self.file(g, f).map(|f| f.ticked) {
                            self.tick(g, f, !ticked);
                        }
                    }
                    Some(Stop::Left(_)) | None => {}
                }
            }
            PanelKey::Enter => {
                if let Some(Stop::Group(lead)) = self.cursor {
                    self.fold(Fold::Card(lead));
                }
            }
            PanelKey::Rename => {
                if let Some(lead) = self.cursor_card()
                    && self.group(lead).is_some_and(|g| g.new)
                {
                    self.editing = Some(lead);
                    return Answer::Name(lead);
                }
            }
            _ => {}
        }
        Answer::Done
    }

    /// A row the cursor reached behind `+N more` opens its card.
    fn unfold_to_cursor(&mut self) {
        match self.cursor.clone() {
            Some(Stop::Row(g, f)) => {
                if let Some(card) = self.card_of(g)
                    && self.rows(&card).iter().position(|r| *r == (g, f)).is_some_and(|p| p >= CARD_NAMES)
                {
                    self.open.insert(Fold::Card(card.lead));
                }
            }
            Some(Stop::Left(i)) => {
                if let Some((why, files)) = self.staying().into_iter().find(|(_, files)| files.contains(&i))
                    && files.iter().position(|&l| l == i).is_some_and(|p| p >= CARD_NAMES)
                {
                    self.open.insert(Fold::Stay(why));
                }
            }
            _ => {}
        }
    }

    /// The panel beside the items: the header slab with the count, the bar, the summary with
    /// its meter and tabs, then the plan in three parts, scrolling.
    pub fn panel(&self, width: f32) -> Element<'_, Message> {
        let (count, inner): (String, Element<'_, Message>) = match &self.plan {
            // Until the daemon has counted the files, the slab has no number to show.
            None => (
                self.progress
                    .filter(|p| p.total > 0)
                    .map_or_else(|| "--".to_string(), |p| format!("{:02}", p.total)),
                widget::column::with_children([self.bar(false), self.waiting()])
                    .spacing(8)
                    .into(),
            ),
            Some(_) => (
                format!("{:02}", self.counts().total),
                widget::column::with_children([
                    self.bar(false),
                    self.top(false),
                    self.tabs(),
                    widget::scrollable(widget::container(self.list()).padding([0, 10, 0, 0]))
                        .id(LIST.clone())
                        .on_scroll(|v| Message::OrganizeScrolled(v.absolute_offset().y))
                        .class(cosmic::style::iced::Scrollable::Minimal)
                        .height(Length::Fill)
                        .into(),
                ])
                .spacing(8)
                .into(),
            ),
        };
        frame(count, inner, Length::Fixed(width))
    }

    /// While the plan is on its way: "Planning…" until the daemon knows how many files there
    /// are, then how many it has placed, with the seconds so far.
    fn waiting(&self) -> Element<'_, Message> {
        let seconds = self.started.map_or(0, |at| at.elapsed().as_secs());
        let (head, line) = match self.progress {
            Some(p) if p.total > 0 => (
                fl!("organize-planning", n = p.total),
                Some(fl!("organize-progress", done = p.done, total = p.total, s = seconds)),
            ),
            // A daemon that says nothing about planning still shows it is alive after a moment.
            _ => (
                fl!("organize-planning-start"),
                (seconds >= 3).then(|| fl!("organize-waited", s = seconds)),
            ),
        };
        widget::column::with_capacity(2)
            .push(crate::ariadne_ui::section(&head))
            .push_maybe(line.map(|line| body(line).class(secondary())))
            .spacing(8)
            .into()
    }

    /// The panel's content without its frame or its own scroll, for COSMIC's drawer.
    pub fn drawer(&self) -> Element<'_, Message> {
        match &self.plan {
            None => self.waiting(),
            Some(_) => widget::column::with_children([self.top(false), self.list()])
                .spacing(8)
                .into(),
        }
    }

    /// The wide sheet: the same bar and summary, a filter, then every file in a table by
    /// destination.
    pub fn sheet(&self) -> Element<'_, Message> {
        let filter = widget::row::with_children([
            micro(&fl!("organize-filter"), Tone::Secondary),
            widget::container(name_field(self.filter.clone(), Message::OrganizeFilter))
                .width(Length::Fixed(240.0))
                .into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center);
        let inner = widget::column::with_children([
            self.bar(true),
            self.top(true),
            filter.into(),
            widget::scrollable(widget::container(self.table()).padding([0, 10, 0, 0]))
                .class(cosmic::style::iced::Scrollable::Minimal)
                .height(Length::Fill)
                .into(),
        ])
        .spacing(8);
        let sheet = frame(format!("{:02}", self.counts().total), inner.into(), Length::Fill);
        // A mouse area takes every press over it, so presses on the sheet stay on the sheet and
        // only the backdrop around it closes it.
        let sheet = widget::mouse_area(widget::container(sheet).max_width(960.0).height(Length::Fill));
        backdrop(sheet.into(), Message::OrganizeSheet(false))
    }

    /// "Discard changes?" over the window, small, on the same scrim as the sheet.
    pub fn confirm(&self) -> Element<'_, Message> {
        let card = widget::column::with_children([
            header(&fl!("organize-discard-title"), String::new()),
            widget::container(
                widget::column::with_children([
                    body(fl!("organize-discard-body"))
                        .wrapping(Wrapping::WordOrGlyph)
                        .into(),
                    widget::row::with_children([
                        widget::space::horizontal().into(),
                        crate::palette::quiet(fl!("organize-keep-editing"))
                            .on_press(Message::OrganizeDiscard(false))
                            .into(),
                        crate::palette::act(fl!("organize-discard"))
                            .on_press(Message::OrganizeDiscard(true))
                            .into(),
                    ])
                    .spacing(8)
                    .into(),
                ])
                .spacing(16),
            )
            .padding(12)
            .into(),
        ]);
        let card = widget::container(card)
            .width(Length::Fixed(360.0))
            .class(crate::palette::panel());
        let card = widget::container(widget::mouse_area(card)).center(Length::Fill);
        backdrop(card.into(), Message::OrganizeDiscard(false))
    }

    /// Move N: dimmed while nothing is ticked or a new name can't be used.
    pub fn move_button(&self) -> Element<'_, Message> {
        let n = self.counts().moving;
        crate::palette::act(fl!("organize-move", n = n))
            .id(MOVE.clone())
            .on_press_maybe(self.can_move().then_some(Message::OrganizeMove))
            .into()
    }

    /// Move N, then the way to the other view, then Close.
    fn bar(&self, sheet: bool) -> Element<'_, Message> {
        let mut bar = Vec::with_capacity(4);
        if self.plan.is_some() {
            bar.push(self.move_button());
        }
        bar.push(widget::space::horizontal().into());
        if self.plan.is_some() && !sheet {
            bar.push(
                crate::palette::quiet(fl!("organize-every-file"))
                    .on_press(Message::OrganizeSheet(true))
                    .into(),
            );
        }
        let close = if sheet {
            Message::OrganizeSheet(false)
        } else {
            Message::OrganizeClose
        };
        bar.push(crate::palette::quiet(fl!("tidy-close")).on_press(close).into());
        widget::row::with_children(bar)
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
    }

    /// The summary line, where the files come from, and the meter (with its legend on the sheet).
    fn top(&self, legend: bool) -> Element<'_, Message> {
        let c = self.counts();
        let mut top = widget::column::with_capacity(4)
            .spacing(4)
            .push(body(self.summary()).wrapping(Wrapping::WordOrGlyph));
        if let Some(origin) = self.origin() {
            top = top.push(body(origin).class(secondary()).wrapping(Wrapping::WordOrGlyph));
        }
        top = top.push(widget::container(meter(c)).padding([4, 0, 0, 0]));
        if legend {
            let key = |tone: fn(&crate::palette::Tones) -> cosmic::iced::Color,
                       framed: bool,
                       label: String| {
                widget::row::with_children([
                    swatch(tone, framed, 8.0),
                    micro(&label, Tone::Secondary),
                ])
                .spacing(4)
                .align_y(Alignment::Center)
                .into()
            };
            top = top.push(
                widget::row::with_children([
                    key(|t| t.solid(), false, fl!("organize-legend-move", n = c.moving)),
                    key(|t| t.secondary(), false, fl!("organize-legend-ask", n = c.asking)),
                    key(|t| t.shadow(), true, fl!("organize-legend-stay", n = c.staying)),
                ])
                .spacing(16)
                .padding([4, 0, 0, 0]),
            );
        }
        top.into()
    }

    /// MOVE, ASK and STAY: each scrolls the list to its part; the part at the top is dark.
    fn tabs(&self) -> Element<'_, Message> {
        let c = self.counts();
        let current = self.current();
        let tab = |part: usize, label: String, n: usize| -> Element<'_, Message> {
            let on = part == current;
            // A zero count is the disabled tone, as the law writes empty values.
            let count = body(n.to_string());
            let count = if n == 0 && !on { count.class(dim()) } else { count };
            widget::button::custom(
                widget::row::with_children([
                    micro(&label, if on { Tone::Light } else { Tone::Ink }),
                    widget::space::horizontal().into(),
                    count.into(),
                ])
                .align_y(Alignment::Center),
            )
            .padding([0, 8])
            .height(Length::Fixed(24.0))
            .width(Length::FillPortion(1))
            .on_press(Message::OrganizeTab(part))
            .class(tab_style(on))
            .into()
        };
        widget::row::with_children([
            tab(MOVING, fl!("organize-tab-move"), c.moving),
            tab(ASKING, fl!("organize-tab-ask"), c.asking),
            tab(STAYING, fl!("organize-tab-stay"), c.staying),
        ])
        .spacing(4)
        .into()
    }

    /// The panel's list: the cards the daemon was sure about, the ones it asks about, and what
    /// stays, each part under a thin rule with its name.
    fn list(&self) -> Element<'_, Message> {
        let Some(plan) = &self.plan else {
            return widget::space::vertical().into();
        };
        let mut list = widget::column::with_capacity(plan.groups.len() + 8).spacing(8);
        for (part, label) in [(MOVING, fl!("organize-tab-move")), (ASKING, fl!("organize-tab-ask"))] {
            let cards = self.part(part);
            if cards.is_empty() {
                continue;
            }
            list = list.push(divider(part, &label, &fl!("organize-folders", n = cards.len())));
            for card in cards {
                list = list.push(self.card(card));
            }
        }
        if !plan.left.is_empty() {
            list = list.push(divider(
                STAYING,
                &fl!("organize-tab-stay"),
                // The noun alone says "files": the count goes in front, as the folders' does.
                &format!("{} {}", plan.left.len(), fl!("organize-files", n = plan.left.len())),
            ));
            for (why, files) in self.staying() {
                list = list.push(self.stay_card(why, files));
            }
        }
        list.into()
    }

    /// One destination in the panel: the tick for the whole card and what Move does, then where
    /// to and from, then its files, the unticked ones dimmed in place, three until `+N more`.
    fn card(&self, card: Card) -> Element<'_, Message> {
        let lead = card.lead;
        let rows = self.rows(&card);
        let fold = Fold::Card(lead);
        let n = rows.len();
        let shown = if self.is_open(&fold) { n } else { n.min(CARD_NAMES) };
        let state = self.card_state(&card);
        let here = self.cursor == Some(Stop::Group(lead));
        let heading = widget::row::with_children([
            tick(state, Message::OrganizeTickGroup(lead, state != Ticked::On), here),
            body(self.sentence(lead)).wrapping(Wrapping::WordOrGlyph).width(Length::Fill).into(),
            self.send_to_button(Sending::Card(lead)),
        ])
        .spacing(6);
        let from = common(card.members.iter().flat_map(|&g| {
            self.group(g).into_iter().flat_map(|group| group.files.iter().map(|f| f.from.as_path()))
        }));
        // A rename that joined this card to another folder can be taken back.
        let to = widget::row::with_capacity(2)
            .push(self.destination(lead, PARENT_CHARS))
            .push_maybe(self.merged_by_rename(&card).then(|| {
                flat(
                    body(fl!("organize-unrename")).class(secondary()).wrapping(Wrapping::None),
                    Message::OrganizeUnrename(lead),
                )
            }))
            .spacing(8)
            .align_y(Alignment::Center);
        let mut lines = widget::column::with_capacity(shown + 5)
            .spacing(4)
            .width(Length::Fill)
            .push(kv(&fl!("organize-to"), to.into()))
            .push_maybe(self.name_error(lead).map(name_problem))
            .push_maybe(self.carry_question(lead).map(|(from, to, more)| carry_line(lead, &from, &to, more)))
            .push(kv(&fl!("organize-from"), fit(short(&from)).class(secondary()).into()));
        for &(g, f) in &rows[..shown] {
            lines = lines.push(self.card_row(g, f));
        }
        if let Some(latch) = latch(n, shown, fold) {
            lines = lines.push(latch);
        }
        plate(
            widget::column::with_children([
                mark(here, heading.into()),
                widget::container(lines).padding([0, 0, 0, 28]).into(),
            ])
            .spacing(4)
            .into(),
        )
    }

    /// One file in a card: its tick, its name (which shows it in the view), why it goes there,
    /// and Send to. An unticked file is dimmed and keeps its row.
    fn card_row(&self, g: usize, f: usize) -> Element<'_, Message> {
        let file = &self.plan.as_ref().expect("a card has a plan").groups[g].files[f];
        let here = self.cursor == Some(Stop::Row(g, f));
        let note = file.note.clone();
        let beside = note.chars().count() <= NOTE_BESIDE;
        let quiet = if file.ticked { secondary() } else { dim() };
        let line = widget::row::with_capacity(4)
            .push(tick(
                if file.ticked { Ticked::On } else { Ticked::Off },
                Message::OrganizeTick(g, f, !file.ticked),
                here,
            ))
            .push(self.file_name(&file.path, file.ticked))
            .push_maybe((beside && !note.is_empty()).then(|| body(note.clone()).class(quiet)))
            .push(self.send_to_button(Sending::File(file.path.clone())))
            .spacing(6)
            .align_y(Alignment::Center);
        let under = (!beside).then(|| {
            widget::container(body(note).class(quiet).wrapping(Wrapping::WordOrGlyph)).padding([0, 0, 0, 28])
        });
        mark(here, widget::column::with_capacity(2).push(line).push_maybe(under).into())
    }

    /// A file's name, cut in the middle to fit, whole on hover when it may have been cut; a
    /// press shows the file in the view.
    fn file_name(&self, path: &Path, ticked: bool) -> Element<'_, Message> {
        let name = folder_name(path);
        let text = fit(name.clone());
        let text = if ticked { text } else { text.class(dim()) };
        let button = flat(text, Message::OrganizeReveal(path.to_path_buf())).width(Length::Fill);
        if name.chars().count() > LONG_NAME {
            widget::tooltip(button, body(name), widget::tooltip::Position::Bottom).into()
        } else {
            button.into()
        }
    }

    /// "Trash this copy" on a copy that stays, "Keep this copy" once it is marked: the trash
    /// waits for Move and goes in its batch.
    fn trash_toggle(&self, path: &Path) -> Element<'_, Message> {
        let label = if self.trash.contains(path) {
            fl!("organize-keep-copy")
        } else {
            fl!("organize-trash-copy")
        };
        flat(
            body(label).class(secondary()).wrapping(Wrapping::None),
            Message::OrganizeTrash(path.to_path_buf()),
        )
        .into()
    }

    /// "Send to", and while it is open, the folders it offers under it. The popup keeps inside
    /// the window.
    fn send_to_button(&self, sending: Sending) -> Element<'_, Message> {
        let label = flat(
            body(fl!("organize-send-to")).class(secondary()).wrapping(Wrapping::None),
            Message::OrganizeSendMenu(Some(sending.clone())),
        );
        if self.send_menu.as_ref() != Some(&sending) {
            return label.into();
        }
        // The popup is pushed back inside the window; the margin keeps it off the window's own
        // edge too.
        let popup = widget::container(self.send_sheet(sending)).padding([0, 8, 8, 0]);
        widget::popover(label)
            .popup(popup)
            .on_close(Message::OrganizeSendMenu(None))
            .position(widget::popover::Position::Bottom)
            .into()
    }

    /// The folders Send to offers, the keyboard's choice framed, then Browse for any other.
    fn send_sheet(&self, sending: Sending) -> Element<'_, Message> {
        let choices = self.send_choices(&sending);
        let browse = choices.len();
        let mut rows: Vec<Element<'_, Message>> = choices
            .into_iter()
            .enumerate()
            .map(|(i, folder)| {
                let choice = widget::button::custom(fit(home_relative(&folder)))
                    .padding([4, 8])
                    .width(Length::Fill)
                    .on_press(Message::OrganizeSendTo(sending.clone(), folder))
                    .class(crate::palette::plate_button());
                // The choice has a background of its own: the ring sits 1px around it.
                ring(self.send_pick == Some(i), choice.into()).padding(1).into()
            })
            .collect();
        rows.push(
            ring(
                self.send_pick == Some(browse),
                crate::palette::quiet(fl!("organize-send-browse"))
                    .on_press(Message::OrganizeBrowse(sending))
                    .into(),
            )
            .padding(1)
            .into(),
        );
        sheet(&fl!("organize-send-title"), String::new(), vec![
            widget::column::with_children(rows).spacing(4).into(),
        ])
    }

    /// The files that stay for one reason, in the panel: a sentence, where they sit, and each
    /// file with Send to (and for a copy filed elsewhere, Trash this copy).
    fn stay_card(&self, why: String, files: Vec<usize>) -> Element<'_, Message> {
        let plan = self.plan.as_ref().expect("a stay card has a plan");
        let fold = Fold::Stay(why.clone());
        let n = files.len();
        let shown = if self.is_open(&fold) { n } else { n.min(CARD_NAMES) };
        let mut names = widget::column::with_capacity(shown).spacing(4);
        for &i in &files[..shown] {
            let left = &plan.left[i];
            let here = self.cursor == Some(Stop::Left(i));
            let marked = self.trash.contains(&left.path);
            let row = widget::row::with_capacity(3)
                .push(self.file_name(&left.path, !marked))
                .push(self.send_to_button(Sending::File(left.path.clone())))
                .push_maybe(Self::is_copy(left).then(|| self.trash_toggle(&left.path)))
                .spacing(6)
                .align_y(Alignment::Center);
            let under = marked.then(|| body(fl!("organize-will-trash")).class(secondary()));
            names = names.push(mark(here, widget::column::with_capacity(2).push(row).push_maybe(under).into()));
        }
        let mut lines = widget::column::with_capacity(5)
            .spacing(4)
            .width(Length::Fill)
            .push(body(fl!("organize-leave", n = n, why = why.clone())).wrapping(Wrapping::WordOrGlyph))
            .push(kv(
                &fl!("organize-in"),
                fit(short(&common(files.iter().filter_map(|&i| plan.left[i].path.parent())))).class(secondary()).into(),
            ))
            .push(names);
        if let Some(latch) = latch(n, shown, fold) {
            lines = lines.push(latch);
        }
        plate(
            widget::row::with_children([
                widget::space::horizontal().width(22).into(),
                lines.into(),
            ])
            .spacing(6)
            .into(),
        )
    }

    /// A card's folder: the path cut in the middle to fit, or for a new folder its parent and a
    /// field for its last part, then the NEW tag while it is not on disk. The field fits its
    /// name, so the tag follows it.
    fn destination(&self, lead: usize, parent_chars: usize) -> Element<'_, Message> {
        let Some(group) = self.group(lead) else {
            return widget::space::horizontal().into();
        };
        let shown = self.shown(lead);
        if !group.new {
            return fit(short(&shown)).into();
        }
        let width = (self.name(lead).chars().count().max(4) + 1) as f32 * CHAR_WIDTH + 8.0;
        let parent = shown.parent().map(short).unwrap_or_default();
        let field = name_field(self.name(lead), move |name| Message::OrganizeName(lead, name))
            .id(name_id(lead))
            .on_submit(|_| Message::OrganizeNamed);
        widget::row::with_capacity(3)
            .push(body(clip_middle(&format!("{parent}/"), parent_chars)))
            .push(widget::container(field).width(Length::Fixed(width)))
            .push_maybe(self.is_new(lead).then(new_tag))
            .spacing(4)
            .align_y(Alignment::Center)
            .into()
    }

    /// The sheet's table: a header row, then each destination with all its rows, then the files
    /// that stay. The filter keeps the rows whose name, destination or reason has it, and the
    /// destinations that have any.
    fn table(&self) -> Element<'_, Message> {
        let Some(plan) = &self.plan else {
            return widget::space::vertical().into();
        };
        let filtering = !self.filter.trim().is_empty();
        let mut table = widget::column::with_capacity(plan.groups.len() * 8 + 8).spacing(4);
        let mut any = false;
        table = table.push(grid_row(
            [
                widget::space::horizontal().width(16).into(),
                widget::space::horizontal().width(22).into(),
                part_cell(micro(&fl!("organize-col-file"), Tone::Secondary), 7),
                widget::space::horizontal().width(16).into(),
                part_cell(micro(&fl!("organize-col-to"), Tone::Secondary), 7),
                part_cell(micro(&fl!("organize-col-why"), Tone::Secondary), 8),
                widget::space::horizontal().width(SEND_WIDTH).into(),
            ],
            false,
        ));
        for card in self.cards() {
            let lead = card.lead;
            let dest = short(&self.shown(lead));
            let all = self.rows(&card);
            let rows: Vec<(usize, usize)> = all
                .iter()
                .copied()
                .filter(|&(g, f)| {
                    let file = &plan.groups[g].files[f];
                    self.matches(&file.path, &dest, &file.note)
                })
                .collect();
            if filtering && rows.is_empty() {
                continue;
            }
            any = true;
            let ticked = all.iter().filter(|&&(g, f)| plan.groups[g].files[f].ticked).count();
            let state = self.card_state(&card);
            let asking = all.len() - ticked;
            let count = [
                (ticked > 0).then(|| fl!("organize-n-move", n = ticked)),
                (asking > 0).then(|| fl!("organize-n-ask", n = asking)),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
            let fold = Fold::Group(Some(lead));
            let open = filtering || self.is_open(&fold);
            table = table.push(self.group_row(
                fold,
                Some(tick(state, Message::OrganizeTickGroup(lead, state != Ticked::On), false)),
                self.destination(lead, 40),
                self.why(lead),
                count,
            ));
            if let Some(problem) = self.name_error(lead) {
                table = table.push(widget::container(name_problem(problem)).padding([0, 8, 0, 60]));
            }
            if !open {
                continue;
            }
            for (g, f) in rows {
                let file = &plan.groups[g].files[f];
                let quiet = if file.ticked { secondary() } else { dim() };
                // An unticked file goes nowhere: it says so, instead of the folder it would go to.
                let (arrow, to): (Element<'_, Message>, String) = if file.ticked {
                    (quiet_icon(Glyph::Arrow, 16), dest.clone())
                } else {
                    (widget::space::horizontal().width(16).into(), fl!("organize-stays"))
                };
                table = table.push(grid_row(
                    [
                        widget::space::horizontal().width(16).into(),
                        tick(
                            if file.ticked { Ticked::On } else { Ticked::Off },
                            Message::OrganizeTick(g, f, !file.ticked),
                            false,
                        ),
                        part_cell(self.file_name(&file.path, file.ticked), 7),
                        arrow,
                        part_cell(fit(to).class(quiet), 7),
                        part_cell(fit(file.note.clone()).class(quiet), 8),
                        widget::container(self.send_to_button(Sending::File(file.path.clone())))
                            .width(Length::Fixed(SEND_WIDTH))
                            .into(),
                    ],
                    false,
                ));
            }
        }
        let staying: Vec<&Left> = plan
            .left
            .iter()
            .filter(|l| self.matches(&l.path, &fl!("organize-stays"), &l.why))
            .collect();
        if !staying.is_empty() {
            any = true;
            let fold = Fold::Group(None);
            let open = filtering || self.is_open(&fold);
            let label = match self.source() {
                Some(source) => fl!("organize-staying-in", folder = short(&source)),
                None => fl!("organize-staying"),
            };
            table = table.push(self.group_row(
                fold,
                None,
                fit(label).into(),
                String::new(),
                fl!("organize-n-stay", n = plan.left.len()),
            ));
            if open {
                for left in staying {
                    // A reason is a sentence with a folder in it: two lines, never cut there.
                    // A copy filed elsewhere can go to the trash with the Move, said under its
                    // reason.
                    let marked = self.trash.contains(&left.path);
                    let why = widget::column::with_capacity(2)
                        .push(
                            body(left.why.clone())
                                .class(secondary())
                                .wrapping(Wrapping::WordOrGlyph)
                                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(2))),
                        )
                        .push_maybe(Self::is_copy(left).then(|| self.trash_toggle(&left.path)));
                    let fate = if marked { fl!("organize-will-trash") } else { fl!("organize-stays") };
                    table = table.push(tall_row([
                        widget::space::horizontal().width(16).into(),
                        widget::space::horizontal().width(22).into(),
                        part_cell(self.file_name(&left.path, !marked), 7),
                        widget::space::horizontal().width(16).into(),
                        part_cell(widget::text(fate).size(12.0).class(dim()), 7),
                        part_cell(why, 8),
                        widget::container(self.send_to_button(Sending::File(left.path.clone())))
                            .width(Length::Fixed(SEND_WIDTH))
                            .into(),
                    ]));
                }
            }
        }
        if !any {
            table = table.push(widget::container(body(fl!("organize-no-match")).class(secondary())).padding([8, 8]));
        }
        table.into()
    }

    /// A destination's header row on the sheet: the chevron, its tick, the folder, then why and
    /// the counts, which fold the group like the chevron does. Its fixed widths add up to a file
    /// row's, so the columns line up at any width.
    fn group_row<'a>(
        &'a self,
        fold: Fold,
        tick: Option<Element<'a, Message>>,
        folder: Element<'a, Message>,
        why: String,
        count: String,
    ) -> Element<'a, Message> {
        let chevron = if self.is_open(&fold) { Glyph::Open } else { Glyph::Shut };
        let toggle = Message::OrganizeFold(fold);
        grid_row(
            [
                flat(
                    widget::container(widget::icon(glyph(chevron)).size(8)).center(Length::Fixed(16.0)),
                    toggle.clone(),
                )
                .into(),
                tick.unwrap_or_else(|| widget::space::horizontal().width(22).into()),
                part_cell(folder, 14),
                widget::space::horizontal().width(24).into(),
                flat(
                    widget::row::with_children([
                        fit(why).class(secondary()).into(),
                        body(count).into(),
                    ])
                    .spacing(8)
                    .height(Length::Fixed(24.0))
                    .align_y(Alignment::Center),
                    toggle,
                )
                .width(Length::FillPortion(8))
                .into(),
                widget::space::horizontal().width(SEND_WIDTH).into(),
            ],
            true,
        )
    }
}

/// The daemon's words with a count in them, the count changed from `was` to `now` and its noun
/// made one or many to match ("4 files named boxlab" to "3 files", "71 photos" to "1 photo").
/// None when the count is not there as a word.
fn recount(text: &str, was: usize, now: usize) -> Option<String> {
    let mut words: Vec<String> = text.split(' ').map(str::to_string).collect();
    let at = words.iter().position(|w| *w == was.to_string())?;
    words[at] = now.to_string();
    if let Some(noun) = words.get_mut(at + 1) {
        if now == 1 && was != 1 {
            if let Some(one) = noun.strip_suffix('s') {
                *noun = one.to_string();
            }
        } else if now != 1 && was == 1 {
            noun.push('s');
        }
    }
    Some(words.join(" "))
}

/// The home for a kind of file, for Send to: pictures, videos, music, else documents.
fn type_home(path: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let kind = mime_guess::from_path(path).first().map(|m| m.type_().as_str().to_string());
    Some(home.join(match kind.as_deref() {
        Some("image") => "Pictures",
        Some("video") => "Videos",
        Some("audio") => "Music",
        _ => "Documents",
    }))
}

/// The cursor's frame: a 1px ink edge around the row it stands on, and the id the list is
/// measured by to keep it in view. Drawn on every row, so the frame never moves anything.
fn mark<'a>(on: bool, content: Element<'a, Message>) -> Element<'a, Message> {
    let frame = ring(on, content);
    if on { frame.id(CURSOR.clone()).into() } else { frame.into() }
}

/// A 1px ink edge around what the keyboard stands on, or room for it.
fn ring<'a>(on: bool, content: Element<'a, Message>) -> widget::Container<'a, Message, cosmic::Theme, cosmic::Renderer> {
    widget::container(content)
        .width(Length::Fill)
        .class(theme::Container::custom(move |_| container::Style {
            border: Border {
                color: ladder().map_or(cosmic::iced::Color::TRANSPARENT, |t| t.text()),
                width: if on { 1.0 } else { 0.0 },
                radius: 0.0.into(),
            },
            ..Default::default()
        }))
}

/// "Also move papers/cv to invoice/cv?" under a renamed card, with its two answers (y and n).
fn carry_line<'a>(lead: usize, from: &Path, to: &Path, more: usize) -> Element<'a, Message> {
    let (from, to) = (home_relative(from), home_relative(to));
    let ask = if more == 0 {
        fl!("organize-carry", from = from, to = to)
    } else {
        fl!("organize-carry-more", from = from, to = to, n = more)
    };
    widget::column::with_children([
        body(ask).wrapping(Wrapping::WordOrGlyph).into(),
        widget::row::with_children([
            crate::palette::quiet(fl!("organize-carry-yes"))
                .on_press(Message::OrganizeCarry(lead, true))
                .into(),
            crate::palette::quiet(fl!("organize-carry-no"))
                .on_press(Message::OrganizeCarry(lead, false))
                .into(),
        ])
        .spacing(8)
        .into(),
    ])
    .spacing(4)
    .into()
}

/// One line of text that fills its room and gives up its middle when it can't fit.
fn fit<'a>(text: String) -> widget::Text<'a, cosmic::Theme, cosmic::Renderer> {
    body(text)
        .width(Length::Fill)
        .wrapping(Wrapping::None)
        .ellipsize(Ellipsize::Middle(EllipsizeHeightLimit::Lines(1)))
}

/// A new folder name that can't be used, said under its field.
fn name_problem<'a>(problem: String) -> Element<'a, Message> {
    widget::row::with_children([
        micro(&fl!("organize-name-label"), Tone::Ink),
        body(problem).wrapping(Wrapping::WordOrGlyph).into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

/// A part of the panel's list: its name, a thin rule, and how much is in it. Thin, so the list
/// scrolling under the tabs never cuts a heavy slab in half.
fn divider<'a>(part: usize, label: &str, value: &str) -> Element<'a, Message> {
    let rule = widget::container(widget::space::horizontal().height(1))
        .width(Length::Fill)
        .class(theme::Container::custom(|_| container::Style {
            background: ladder().map(|t| Background::Color(t.secondary())),
            ..Default::default()
        }));
    widget::container(
        widget::row::with_children([
            micro(label, Tone::Ink),
            rule.into(),
            micro(value, Tone::Secondary),
        ])
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .id(SECTIONS[part].clone())
    .padding([4, 0])
    .into()
}

/// The scrim behind the sheet and the confirm; a press on it does `close`.
fn backdrop<'a>(content: Element<'a, Message>, close: Message) -> Element<'a, Message> {
    let backdrop = widget::container(content)
        .padding(24)
        .center_x(Length::Fill)
        .height(Length::Fill)
        .class(theme::Container::custom(|_| container::Style {
            background: ladder().map(|t| Background::Color(scrim(&t))),
            ..Default::default()
        }));
    widget::mouse_area(backdrop).on_press(close).into()
}

/// A folder written as the daemon writes it in sentences: relative to home, without `~/`.
pub fn home_relative(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rel) => rel.display().to_string(),
        None => path.display().to_string(),
    }
}

/// The folder every path shares, or the longest common start of them.
fn common<'a>(mut paths: impl Iterator<Item = &'a Path>) -> PathBuf {
    let Some(first) = paths.next() else {
        return PathBuf::new();
    };
    let mut common = first.to_path_buf();
    for path in paths {
        while !path.starts_with(&common) {
            if !common.pop() {
                return PathBuf::new();
            }
        }
    }
    common
}

/// The header slab, then the body padded as the Tidy panel pads it, on the panel tone.
fn frame<'a>(count: String, inner: Element<'a, Message>, width: Length) -> Element<'a, Message> {
    widget::container(widget::column::with_children([
        header(&fl!("organize"), count),
        widget::container(inner)
            .padding([8, 8, 8, 8])
            .height(Length::Fill)
            .into(),
    ]))
    .class(crate::palette::panel())
    .width(width)
    .height(Length::Fill)
    .into()
}

/// A card on the panel's list: the plate tone, 8px in.
fn plate(content: Element<'_, Message>) -> Element<'_, Message> {
    widget::container(content)
        .padding(8)
        .width(Length::Fill)
        .class(crate::palette::row_plate())
        .into()
}

/// A label in micro caps in a 48px column, room for FROM in tracked caps and a gap, then its
/// value.
fn kv<'a>(label: &str, value: Element<'a, Message>) -> Element<'a, Message> {
    widget::row::with_children([
        widget::container(micro(label, Tone::Secondary))
            .width(Length::Fixed(48.0))
            .into(),
        value,
    ])
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

/// `+N more` while a card shows only some, `Fewer` once it shows all of a long one.
fn latch<'a>(total: usize, shown: usize, fold: Fold) -> Option<Element<'a, Message>> {
    if total <= CARD_NAMES {
        return None;
    }
    let hidden: usize = total - shown;
    let label = if hidden > 0 {
        fl!("organize-more", n = hidden)
    } else {
        fl!("organize-fewer")
    };
    Some(widget::container(more(label, Message::OrganizeFold(fold))).padding([4, 0, 2, 0]).into())
}

/// A sheet row 24px tall with the table's gutters; a header row sits on the plate tone.
fn grid_row<'a, const N: usize>(cells: [Element<'a, Message>; N], plate: bool) -> Element<'a, Message> {
    let row = widget::container(
        widget::row::with_children(cells)
            .spacing(8)
            .height(Length::Fixed(24.0))
            .align_y(Alignment::Center),
    )
    .padding([0, 8])
    .width(Length::Fill);
    if plate {
        row.class(crate::palette::row_plate()).into()
    } else {
        row.into()
    }
}

/// A sheet row as tall as its tallest cell, for a reason that takes two lines.
fn tall_row<'a, const N: usize>(cells: [Element<'a, Message>; N]) -> Element<'a, Message> {
    widget::container(
        widget::row::with_children(cells)
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// A sheet column's share of the room left after the fixed ones: FILE 7, GOES TO 7, WHY 8.
fn part_cell<'a>(content: impl Into<Element<'a, Message>>, portion: u16) -> Element<'a, Message> {
    widget::container(content)
        .width(Length::FillPortion(portion))
        .into()
}

/// The NEW tag on a folder that does not exist yet: light micro caps on the one dark, with the
/// drop shadow tags carry.
fn new_tag<'a>() -> Element<'a, Message> {
    widget::container(micro(&fl!("organize-new"), Tone::Light))
        .padding([3, 4])
        .class(theme::Container::custom(|_| match ladder() {
            Some(t) => container::Style {
                background: Some(Background::Color(t.accent())),
                shadow: cosmic::iced::Shadow {
                    color: t.faint(),
                    offset: Vector::new(2.0, 2.0),
                    blur_radius: 0.0,
                },
                ..Default::default()
            },
            None => container::Style::default(),
        }))
        .into()
}

/// The meter under the summary: solid for what moves, secondary for what to ask about, the
/// plate tone for what stays.
fn meter<'a>(c: Counts) -> Element<'a, Message> {
    let part = |tone: fn(&crate::palette::Tones) -> cosmic::iced::Color, n: usize| {
        (n > 0).then(|| {
            widget::container(widget::space::horizontal())
                .width(Length::FillPortion(n.min(u16::MAX as usize) as u16))
                .height(8)
                .class(theme::Container::custom(move |_| container::Style {
                    background: ladder().map(|t| Background::Color(tone(&t))),
                    ..Default::default()
                }))
        })
    };
    widget::row::with_capacity(3)
        .push_maybe(part(|t| t.solid(), c.moving))
        .push_maybe(part(|t| t.secondary(), c.asking))
        .push_maybe(part(|t| t.shadow(), c.staying))
        .height(8)
        .into()
}

/// A square of one tone for the legend; the plate tone gets a secondary frame to show on paper.
fn swatch<'a>(
    tone: fn(&crate::palette::Tones) -> cosmic::iced::Color,
    framed: bool,
    size: f32,
) -> Element<'a, Message> {
    widget::container(widget::space::horizontal().width(size).height(size))
        .class(theme::Container::custom(move |_| match ladder() {
            Some(t) => container::Style {
                background: Some(Background::Color(tone(&t))),
                border: Border {
                    color: t.secondary(),
                    width: if framed { 1.0 } else { 0.0 },
                    radius: 0.0.into(),
                },
                ..Default::default()
            },
            None => container::Style::default(),
        }))
        .into()
}

/// A tab plate: the one dark while its part is at the top, the plate tone otherwise.
fn tab_style(on: bool) -> theme::Button {
    let look = move |theme: &theme::Theme| {
        let mut s = widget::button::Style::new();
        s.border_radius = 0.0.into();
        match ladder() {
            Some(t) if on => {
                s.background = Some(Background::Color(t.accent()));
                s.text_color = Some(t.light());
            }
            Some(t) => {
                s.background = Some(Background::Color(t.shadow()));
                s.text_color = Some(t.text());
            }
            None => {
                let c = theme.cosmic();
                s.background = Some(Background::Color(if on {
                    c.accent_color().into()
                } else {
                    c.button_bg_color().into()
                }));
            }
        }
        s
    };
    theme::Button::Custom {
        active: Box::new(move |_, theme| look(theme)),
        disabled: Box::new(move |theme| look(theme)),
        hovered: Box::new(move |_, theme| look(theme)),
        pressed: Box::new(move |_, theme| look(theme)),
    }
}

/// The scrim behind a modal sheet: the law drops everything under it to about 0.6 of its
/// luminance. Ink laid over the field at this opacity does that in the palette's own family.
fn scrim(t: &crate::palette::Tones) -> cosmic::iced::Color {
    cosmic::iced::Color {
        a: 0.42,
        ..t.text()
    }
}

/// The disabled tone: unticked rows, the quiet `stays here`, an empty count.
fn dim() -> theme::Text {
    match ladder() {
        Some(t) => theme::Text::Color(t.dim()),
        None => theme::Text::Default,
    }
}

/// Finds where the panel list's three parts and the cursor's row sit, relative to the top of
/// its content, and how tall the list is on screen.
#[derive(Default)]
struct Measure {
    top: Option<f32>,
    view: f32,
    found: [Option<f32>; 3],
    cursor: Option<Rectangle>,
}

impl Operation<Measured> for Measure {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Measured>)) {
        operate(self);
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        let Some(id) = id else { return };
        if let Some(part) = SECTIONS.iter().position(|s| s == id) {
            self.found[part] = Some(bounds.y);
        } else if id == &*CURSOR {
            self.cursor = Some(bounds);
        }
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        if id == Some(&*LIST) {
            self.top = Some(content_bounds.y);
            self.view = bounds.height;
        }
    }

    fn finish(&self) -> Outcome<Measured> {
        match self.top {
            Some(top) => Outcome::Some(Measured {
                sections: self.found.map(|y| y.map(|y| (y - top).max(0.0))),
                cursor: self.cursor.map(|b| ((b.y - top).max(0.0), b.height)),
                view: self.view,
            }),
            None => Outcome::None,
        }
    }
}

/// Measure the panel's list, so its tabs know where to scroll and which one is current, and
/// the cursor stays in view.
pub fn measure() -> cosmic::app::Task<Message> {
    cosmic::iced::advanced::widget::operate(Measure::default())
        .map(|measured| cosmic::Action::App(Message::OrganizeSections(measured)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, ticked: bool) -> PlanFile {
        PlanFile {
            path: PathBuf::from(path),
            from: PathBuf::from("/home/u/Downloads"),
            ticked,
            share: if ticked { Some(0.9) } else { Some(0.4) },
            note: "note".to_string(),
            alternatives: Vec::new(),
            pre: false,
        }
    }

    fn group(folder: &str, new: bool, summary: &str, files: Vec<PlanFile>) -> PlanGroup {
        PlanGroup {
            folder: PathBuf::from(folder),
            proposed: PathBuf::from(folder),
            new,
            why: "why".to_string(),
            summary: summary.to_string(),
            files,
            carry: true,
        }
    }

    fn plan() -> Plan {
        Plan {
            groups: vec![
                PlanGroup {
                    why: "3 files named varpet".to_string(),
                    ..group(
                        "/home/u/Documents/varpet",
                        true,
                        "Make Documents/varpet and move 2 files named varpet into it",
                        vec![
                            file("/home/u/Downloads/varpet-logo.png", true),
                            file("/home/u/Downloads/varpet-brief.pdf", true),
                            file("/home/u/Downloads/varpet-old.png", false),
                        ],
                    )
                },
                PlanGroup {
                    why: "Like the files already there".to_string(),
                    ..group(
                        "/home/u/Downloads/docs",
                        false,
                        "Nothing ticked: 2 PDFs could go into Downloads/docs",
                        vec![file("/home/u/Downloads/a.pdf", false), file("/home/u/Downloads/b.pdf", false)],
                    )
                },
            ],
            left: vec![
                Left {
                    path: PathBuf::from("/home/u/Downloads/1.stl"),
                    why: "its name says nothing about it".to_string(),
                    alternatives: Vec::new(),
                },
                Left {
                    path: PathBuf::from("/home/u/Downloads/2.stl"),
                    why: "a copy is already in Documents/stl as lid.stl".to_string(),
                    alternatives: Vec::new(),
                },
            ],
            trash: Vec::new(),
        }
    }

    fn picked() -> Vec<PathBuf> {
        vec![PathBuf::from("/home/u/Downloads")]
    }

    fn planned() -> Organize {
        let mut organize = Organize::asking(1, picked());
        organize.set_plan(plan(), false);
        organize
    }

    #[test]
    fn apply_request_declines_only_what_the_user_unticked() {
        let mut organize = planned();
        // Untick a sure file, tick a guess, tick one of two unsure files, rename the new folder.
        organize.tick(0, 1, false);
        organize.tick(0, 2, true);
        organize.tick(1, 0, true);
        organize.rename(0, "varpet-brand".to_string());

        let body = crate::ariadne::apply_body(&organize.edited().unwrap());
        let want = serde_json::json!({"groups": [
            {
                "folder": "/home/u/Documents/varpet-brand",
                "proposed": "/home/u/Documents/varpet",
                "new": true,
                "files": [
                    {"path": "/home/u/Downloads/varpet-logo.png", "ticked": true},
                    {"path": "/home/u/Downloads/varpet-brief.pdf", "ticked": false, "declined": true},
                    {"path": "/home/u/Downloads/varpet-old.png", "ticked": true},
                ],
            },
            {
                "folder": "/home/u/Downloads/docs",
                "proposed": "/home/u/Downloads/docs",
                "new": false,
                // b.pdf was never ticked: no flag, so the daemon records no "no" for it.
                "files": [
                    {"path": "/home/u/Downloads/a.pdf", "ticked": true},
                    {"path": "/home/u/Downloads/b.pdf", "ticked": false},
                ],
            },
        ]});
        assert_eq!(body, want);
    }

    #[test]
    fn a_bad_name_blocks_move_and_keeps_the_proposed_folder() {
        let mut organize = planned();
        assert!(organize.can_move());
        for bad in ["  ", "a/b", "..", ".", ".hidden"] {
            organize.rename(0, bad.to_string());
            assert!(organize.name_error(0).is_some(), "{bad:?} should be refused");
            assert!(organize.blocked());
            assert!(!organize.can_move());
            assert_eq!(organize.folder(0), PathBuf::from("/home/u/Documents/varpet"));
        }
        organize.rename(0, " varpet-brand ".to_string());
        assert!(organize.name_error(0).is_none());
        // Ctrl+Z in the field takes back the last edit only, then does nothing.
        organize.rename(0, " varpet-brandx".to_string());
        organize.undo_name();
        assert_eq!(organize.name(0), " varpet-brand ");
        organize.undo_name();
        assert_eq!(organize.name(0), " varpet-brand ");
        assert_eq!(organize.folder(0), PathBuf::from("/home/u/Documents/varpet-brand"));
        // A bad name on a new folder nothing goes to doesn't hold Move back.
        organize.rename(0, "".to_string());
        organize.tick_card(0, false);
        organize.tick(1, 0, true);
        assert!(!organize.blocked());
        // An existing folder never takes a name.
        organize.rename(1, "other".to_string());
        assert_eq!(organize.folder(1), PathBuf::from("/home/u/Downloads/docs"));
    }

    #[test]
    fn counts_follow_the_ticks() {
        let mut organize = planned();
        let c = organize.counts();
        assert_eq!((c.total, c.moving, c.asking, c.staying), (7, 2, 3, 2));
        assert_eq!((c.folders, c.new), (1, 1));
        organize.tick_card(1, true);
        let c = organize.counts();
        assert_eq!((c.moving, c.asking, c.folders, c.new), (4, 1, 2, 1));
    }

    #[test]
    fn the_order_holds_while_ticks_change() {
        let mut organize = planned();
        assert_eq!(organize.order, vec![(0, MOVING), (1, ASKING)]);
        organize.tick_card(0, false);
        organize.tick_card(1, true);
        let leads = |o: &Organize, part| o.part(part).into_iter().map(|c| c.lead).collect::<Vec<_>>();
        assert_eq!(leads(&organize, MOVING), vec![0]);
        assert_eq!(leads(&organize, ASKING), vec![1]);
        // Rows keep their places: the unticked file is still the second row.
        assert_eq!(
            organize.plan.as_ref().unwrap().groups[0].files[1].path,
            PathBuf::from("/home/u/Downloads/varpet-brief.pdf")
        );
    }

    #[test]
    fn the_card_tick_counts_every_file() {
        let mut organize = planned();
        let state = |o: &Organize| o.card_state(&o.card_of(0).unwrap());
        assert_eq!(state(&organize), Ticked::Some);
        organize.tick(0, 2, true);
        assert_eq!(state(&organize), Ticked::On);
        organize.tick_card(0, false);
        assert_eq!(state(&organize), Ticked::Off);
    }

    #[test]
    fn send_to_moves_a_file_into_the_group_going_there_or_a_new_one() {
        let mut organize = planned();
        // Into a folder the plan already goes to: it joins that group, ticked, with no "no".
        assert!(organize.send_to(Path::new("/home/u/Downloads/b.pdf"), PathBuf::from("/home/u/Documents/varpet")));
        let plan = organize.plan.as_ref().unwrap();
        assert_eq!(plan.groups[0].files.len(), 4);
        assert_eq!(plan.groups[1].files.len(), 1);
        let sent = plan.groups[0].files.last().unwrap();
        assert!(sent.ticked && !sent.declined());
        // Into a folder of the user's own: a new group at the end, under Move.
        assert!(organize.send_to(Path::new("/home/u/Downloads/varpet-brief.pdf"), PathBuf::from("/home/u/Work")));
        assert_eq!(organize.order.last(), Some(&(2, MOVING)));
        let body = crate::ariadne::apply_body(&organize.edited().unwrap());
        assert_eq!(
            body["groups"][2],
            serde_json::json!({
                "folder": "/home/u/Work",
                "proposed": "/home/u/Work",
                "new": false,
                "files": [{"path": "/home/u/Downloads/varpet-brief.pdf", "ticked": true}],
            })
        );
        assert!(organize.dirty());
        // A group a file was sent into can't keep the daemon's words ("named varpet" is not
        // true of b.pdf): the sentence is built from the ticks.
        let plain: String = organize.sentence(0).chars().filter(|c| !matches!(c, '\u{2068}' | '\u{2069}')).collect();
        assert_eq!(plain, "Make a new folder /home/u/Documents/varpet and move 2 files; 1 more unticked");
        // A group every file left is not sent at all.
        assert!(organize.send_to(Path::new("/home/u/Downloads/a.pdf"), PathBuf::from("/home/u/Work")));
        let body = crate::ariadne::apply_body(&organize.edited().unwrap());
        assert_eq!(body["groups"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_staying_file_can_be_sent_and_a_copy_marked_goes_to_the_trash_with_the_move() {
        let mut organize = planned();
        assert!(organize.send_to(Path::new("/home/u/Downloads/1.stl"), PathBuf::from("/home/u/Documents/stl")));
        let plan = organize.plan.as_ref().unwrap();
        assert_eq!(plan.left.len(), 1);
        let sent = &plan.groups[2].files[0];
        assert!(sent.ticked && !sent.declined());
        assert_eq!(sent.from, PathBuf::from("/home/u/Downloads"));

        // The cursor on the copy: Delete marks it, the plan keeps the row, Move carries it.
        let mut organize = planned();
        organize.tick_card(0, false);
        assert!(!organize.can_move());
        let copy = organize.stops().into_iter().find(|s| *s == Stop::Left(1)).unwrap();
        organize.point(copy);
        organize.key(PanelKey::Trash);
        let c = organize.counts();
        assert_eq!((c.staying, c.trashing, c.total), (1, 1, 7));
        assert!(organize.dirty() && organize.can_move());
        let body = crate::ariadne::apply_body(&organize.edited().unwrap());
        assert_eq!(body["trash"], serde_json::json!(["/home/u/Downloads/2.stl"]));
        // A file that is no copy takes no mark; a second Delete takes the mark back.
        organize.point(Stop::Left(0));
        organize.key(PanelKey::Trash);
        assert_eq!(organize.counts().trashing, 1);
        organize.point(Stop::Left(1));
        organize.key(PanelKey::Trash);
        assert_eq!(organize.counts().trashing, 0);
        assert!(crate::ariadne::apply_body(&organize.edited().unwrap()).get("trash").is_none());
    }

    #[test]
    fn the_cursor_walks_every_row_and_space_ticks_where_it_stands() {
        let mut organize = planned();
        assert_eq!(organize.cursor, Some(Stop::Group(0)));
        organize.key(PanelKey::Down);
        assert_eq!(organize.cursor, Some(Stop::Row(0, 0)));
        organize.key(PanelKey::Space);
        assert!(!organize.plan.as_ref().unwrap().groups[0].files[0].ticked);
        // Down through the card, the next card, then the file that stays; it stops at the end.
        for _ in 0..10 {
            organize.key(PanelKey::Down);
        }
        assert_eq!(organize.cursor, Some(Stop::Left(0)));
        organize.key(PanelKey::Up);
        organize.key(PanelKey::Up);
        assert_eq!(organize.cursor, Some(Stop::Row(1, 1)));
        // Space on a heading ticks the whole card; F2 there names only a new folder.
        organize.point_card(1);
        organize.key(PanelKey::Space);
        assert_eq!(organize.card_state(&organize.card_of(1).unwrap()), Ticked::On);
        assert_eq!(organize.key(PanelKey::Rename), Answer::Done);
        organize.point(Stop::Row(0, 2));
        assert_eq!(organize.key(PanelKey::Rename), Answer::Name(0));
        // 1, 2, 3 go to the parts.
        organize.key(PanelKey::Part(STAYING));
        assert!(matches!(organize.cursor, Some(Stop::Left(_))));
        organize.key(PanelKey::Part(ASKING));
        assert_eq!(organize.cursor, Some(Stop::Group(1)));
        organize.key(PanelKey::Part(MOVING));
        assert_eq!(organize.cursor, Some(Stop::Group(0)));
        // Page keys go heading to heading.
        organize.key(PanelKey::NextCard);
        assert_eq!(organize.cursor, Some(Stop::Group(1)));
        organize.key(PanelKey::PrevCard);
        assert_eq!(organize.cursor, Some(Stop::Group(0)));
    }

    #[test]
    fn a_row_behind_more_opens_its_card_when_the_cursor_gets_there() {
        let mut p = plan();
        p.groups[0].files.push(file("/home/u/Downloads/varpet-4.png", true));
        let mut organize = Organize::asking(1, picked());
        organize.set_plan(p, false);
        for _ in 0..4 {
            organize.key(PanelKey::Down);
        }
        assert_eq!(organize.cursor, Some(Stop::Row(0, 3)));
        assert!(organize.is_open(&Fold::Card(0)));
    }

    #[test]
    fn a_folder_inside_a_renamed_one_is_drawn_under_the_new_name_but_sent_as_proposed() {
        let mut p = plan();
        p.groups.push(group(
            "/home/u/Documents/varpet/cv",
            true,
            "Make Documents/varpet/cv and move 1 file into it",
            vec![file("/home/u/Downloads/cv.pdf", true)],
        ));
        let mut organize = Organize::asking(1, picked());
        organize.set_plan(p, false);
        organize.rename(0, "records".to_string());
        assert_eq!(organize.shown(2), PathBuf::from("/home/u/Documents/records/cv"));
        // Its heading follows, though the group itself was not edited.
        assert!(organize.sentence(2).contains("records/cv"), "{}", organize.sentence(2));
        let body = crate::ariadne::apply_body(&organize.edited().unwrap());
        assert_eq!(body["groups"][2]["folder"], "/home/u/Documents/varpet/cv");
        assert_eq!(body["groups"][2]["proposed"], "/home/u/Documents/varpet/cv");
        assert!(body["groups"][2].get("carry").is_none());
        // The renamed card asks before it takes cv along; no keeps cv where it was planned.
        assert_eq!(
            organize.carry_question(0),
            Some((
                PathBuf::from("/home/u/Documents/varpet/cv"),
                PathBuf::from("/home/u/Documents/records/cv"),
                0
            ))
        );
        organize.key(PanelKey::Carry(false));
        assert_eq!(organize.carry_question(0), None);
        assert_eq!(organize.shown(2), PathBuf::from("/home/u/Documents/varpet/cv"));
        let body = crate::ariadne::apply_body(&organize.edited().unwrap());
        assert_eq!(body["groups"][2]["carry"], false);
        assert_eq!(body["groups"][2]["folder"], "/home/u/Documents/varpet/cv");
    }

    #[test]
    fn a_card_is_sent_whole_and_send_to_offers_the_plan_folders_first() {
        let mut organize = planned();
        let card = Sending::Card(0);
        let choices = organize.send_choices(&card);
        // Its own folder is not offered; the other card's is first, the same for every file.
        assert_eq!(choices.first(), Some(&PathBuf::from("/home/u/Downloads/docs")));
        assert!(!choices.contains(&PathBuf::from("/home/u/Documents/varpet")));
        assert_eq!(
            organize.send_choices(&Sending::File(PathBuf::from("/home/u/Downloads/varpet-old.png"))).first(),
            choices.first()
        );
        // The keyboard: s opens it, Down and Enter take the second choice.
        organize.key(PanelKey::Send);
        assert_eq!(organize.send_menu, Some(card.clone()));
        organize.key(PanelKey::Down);
        assert_eq!(organize.key(PanelKey::Enter), Answer::Send(card.clone(), choices[1].clone()));
        assert_eq!(organize.send_menu, None);
        // Every file of the card goes, ticked, into the one group going there.
        assert!(organize.send(&card, PathBuf::from("/home/u/Downloads/docs")));
        let plan = organize.plan.as_ref().unwrap();
        assert!(plan.groups[0].files.is_empty());
        assert_eq!(plan.groups[1].files.len(), 5);
        assert_eq!(organize.cards().len(), 1);
        // The reason's count follows the files that left.
        assert_eq!(organize.why(1), "Like the files already there");
        assert_eq!(recount("4 files named boxlab", 4, 1).as_deref(), Some("1 file named boxlab"));
    }

    #[test]
    fn a_plan_whose_folder_is_gone_says_so() {
        let disk = std::env::temp_dir().join(format!("organize-gone-{}", std::process::id()));
        std::fs::create_dir_all(&disk).unwrap();
        let mut organize = Organize::asking(1, vec![disk.clone()]);
        assert!(!organize.gone());
        organize.set_plan(plan(), false);
        assert!(!organize.gone());
        std::fs::remove_dir_all(&disk).unwrap();
        assert!(organize.gone());
    }

    #[test]
    fn renaming_onto_a_folder_on_disk_joins_it_on_one_card() {
        let disk = std::env::temp_dir().join(format!("organize-test-{}", std::process::id()));
        std::fs::create_dir_all(disk.join("invoice")).unwrap();
        let at = |name: &str| disk.join(name).display().to_string();
        let mut organize = Organize::asking(1, picked());
        organize.set_plan(
            Plan {
                groups: vec![
                    group(&at("bills"), true, "", vec![file("/home/u/Downloads/b1.pdf", true)]),
                    group(&at("invoice"), false, "", vec![file("/home/u/Downloads/i1.pdf", true)]),
                    group(&at("receipts"), true, "", vec![file("/home/u/Downloads/r1.pdf", true)]),
                ],
                left: Vec::new(),
                trash: Vec::new(),
            },
            false,
        );
        assert_eq!(organize.cards().len(), 3);
        organize.rename(0, "invoice".to_string());
        organize.rename(2, "invoice".to_string());
        let cards = organize.cards();
        assert_eq!(cards.len(), 1);
        // The card being named leads, so its field stays where the typing is.
        assert_eq!(cards[0].lead, 2);
        assert!(!organize.is_new(2));
        assert!(organize.sentence(2).contains("existing folder"));
        assert_eq!((organize.counts().folders, organize.counts().new), (1, 0));
        // Undo rename splits the card again, each folder under its own name.
        assert!(organize.merged_by_rename(&cards[0]));
        organize.key(PanelKey::Unrename);
        assert_eq!(organize.cards().len(), 3);
        assert_eq!(organize.folder(0), disk.join("bills"));
        std::fs::remove_dir_all(&disk).unwrap();
    }

    #[test]
    fn the_filter_finds_a_file_by_where_it_goes_and_why() {
        let mut organize = planned();
        organize.set_filter("docs".to_string());
        assert!(organize.matches(Path::new("/home/u/Downloads/a.pdf"), "~/Downloads/docs", "note"));
        organize.set_filter("SAYS NOTHING".to_string());
        assert!(organize.matches(Path::new("/home/u/Downloads/1.stl"), "stays here", "its name says nothing about it"));
        organize.set_filter("zzz".to_string());
        assert!(!organize.matches(Path::new("/home/u/Downloads/a.pdf"), "~/Downloads/docs", "note"));
    }

    #[test]
    fn the_daemon_sentence_holds_until_the_group_is_edited_then_keeps_its_wording() {
        let mut organize = planned();
        assert!(!organize.dirty());
        assert_eq!(
            organize.sentence(0),
            "Make Documents/varpet and move 2 files named varpet into it"
        );
        organize.tick(0, 1, false);
        // Fluent wraps what it puts in in isolation marks; the words are what count here.
        let plain: String = organize.sentence(0).chars().filter(|c| !matches!(c, '\u{2068}' | '\u{2069}')).collect();
        assert_eq!(
            plain,
            "Make Documents/varpet and move 1 file named varpet into it; 2 more unticked"
        );
        assert!(organize.dirty());
    }
}
