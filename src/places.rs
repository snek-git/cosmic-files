//! The places panel in sections, as Finder has them: ariadne's own places, yours, the drives,
//! the network, then Trash. The app keeps one nav model that every message and menu works on;
//! each section draws from a copy of its part of it and maps clicks back.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use cosmic::widget::dnd_destination::DragId;
use cosmic::widget::segmented_button::{self, Entity, Selectable, SingleSelectModel};

/// One drop target id per section, the same across rebuilds.
static DRAG_IDS: LazyLock<[DragId; 5]> = LazyLock::new(|| std::array::from_fn(|_| DragId::new()));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Section {
    Ariadne,
    Places,
    Drives,
    Network,
    Trash,
}

impl Section {
    const ORDER: [Self; 5] = [
        Self::Ariadne,
        Self::Places,
        Self::Drives,
        Self::Network,
        Self::Trash,
    ];

    pub fn drag_id(self) -> DragId {
        let i = Self::ORDER.iter().position(|s| *s == self).unwrap_or(0);
        DRAG_IDS[i]
    }

    /// The header slab's label; Trash stands alone at the bottom without one.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Ariadne => Some("ariadne"),
            Self::Places => Some("places"),
            Self::Drives => Some("drives"),
            Self::Network => Some("network"),
            Self::Trash => None,
        }
    }
}

/// One section as drawn: its own model, and the app's entity for each of its items.
pub struct Group {
    pub section: Section,
    pub model: SingleSelectModel,
    master: Arc<HashMap<Entity, Entity>>,
}

impl Group {
    /// Turns this section's entity into the app's, for every message the widget sends.
    pub fn to_master(&self) -> impl Fn(Entity) -> Entity + Clone + 'static {
        let master = self.master.clone();
        move |entity| master.get(&entity).copied().unwrap_or(entity)
    }

    /// Each row's share taken and whether it is lit, for the tracks under the Drives rows.
    pub fn usage(&self, nav: &SingleSelectModel) -> Vec<(Option<f32>, bool)> {
        let active = nav.active();
        self.masters()
            .into_iter()
            .map(|m| (nav.data::<Usage>(m).map(|u| u.0), m == active))
            .collect()
    }

    /// The app's entities, in the order this section draws them.
    pub fn masters(&self) -> Vec<Entity> {
        self.model.iter().map(self.to_master()).collect()
    }
}

/// Split the app's nav model by the `Section` each item carries, in Finder's order.
pub fn split(nav: &SingleSelectModel) -> Vec<Group> {
    Section::ORDER
        .into_iter()
        .filter_map(|section| {
            let items: Vec<Entity> = nav
                .iter()
                .filter(|&e| nav.data::<Section>(e).copied().unwrap_or(Section::Places) == section)
                .collect();
            if items.is_empty() {
                return None;
            }
            let mut builder = segmented_button::ModelBuilder::default();
            for &e in &items {
                builder = builder.insert(|mut b| {
                    b = b.text(nav.text(e).unwrap_or_default().to_string());
                    if let Some(icon) = nav.icon(e) {
                        b = b.icon(icon.clone());
                    }
                    if nav.is_closable(e) {
                        b = b.closable();
                    }
                    b
                });
            }
            let model = builder.build();
            let master = model.iter().zip(items).collect();
            Some(Group {
                section,
                model,
                master: Arc::new(master),
            })
        })
        .collect()
}

/// Light the item that stands for `active` in its section, and nothing in the others.
pub fn sync(groups: &mut [Group], active: Option<Entity>) {
    for group in groups {
        let found = group
            .model
            .iter()
            .find(|e| Some(group.master.get(e).copied().unwrap_or(*e)) == active);
        match found {
            Some(e) => group.model.activate(e),
            None => {
                let current = group.model.active();
                Selectable::deactivate(&mut group.model, current);
            }
        }
    }
}

/// A disk mounted in the file system, as the Drives section lists it.
pub struct Drive {
    pub name: String,
    pub path: PathBuf,
    pub used: Option<Usage>,
}

/// How full a drive is, 0 to 1: the track under its row.
#[derive(Clone, Copy, Debug)]
pub struct Usage(pub f32);

/// Height of a row in the Drives section. The row centers its name, so the extra height is what
/// keeps the track under it clear of the name.
pub const DRIVE_ROW: u16 = 46;

/// The root and every block device mounted under /mnt, /media or /run/media, one entry per
/// device. Read from the mount table, since gvfs hides disks that fstab mounts.
pub fn drives() -> Vec<Drive> {
    let Ok(table) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return Vec::new();
    };
    let mut seen = Vec::new();
    let mut drives = Vec::new();
    for line in table.lines() {
        let Some((head, tail)) = line.split_once(" - ") else {
            continue;
        };
        let head: Vec<&str> = head.split(' ').collect();
        let tail: Vec<&str> = tail.split(' ').collect();
        let (Some(mount), Some(source)) = (head.get(4), tail.get(1)) else {
            continue;
        };
        let mount = mount.replace("\\040", " ");
        let wanted = mount == "/"
            || ["/mnt/", "/media/", "/run/media/"]
                .iter()
                .any(|p| mount.starts_with(p));
        if !wanted || !source.starts_with("/dev/") || seen.contains(source) {
            continue;
        }
        seen.push(*source);
        let path = PathBuf::from(&mount);
        let name = if mount == "/" {
            "System".to_string()
        } else {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| mount.clone())
        };
        drives.push(Drive {
            name,
            used: usage(&path),
            path,
        });
    }
    drives
}

/// The share of the disk taken, counting the space ext4 keeps back for root.
fn usage(path: &std::path::Path) -> Option<Usage> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 || stat.f_blocks == 0 {
        return None;
    }
    Some(Usage(1.0 - stat.f_bavail as f32 / stat.f_blocks as f32))
}
