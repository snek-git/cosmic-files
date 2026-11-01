// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{self, Core, Settings, Task};
use cosmic::iced::{Subscription, window};
use cosmic::{Application, Element, executor};
use cosmic_files::map::{self, Map, Node, Overlay, Travel};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const MB: u64 = 1 << 20;
const GB: u64 = 1 << 30;
const HOME: &str = "/home/snek";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run::<App>(Settings::default(), ())?;
    Ok(())
}

fn path(rel: &str) -> PathBuf {
    PathBuf::from(HOME).join(rel)
}

fn file(rel: &str, size: u64) -> Node {
    Node {
        path: path(rel),
        name: String::new(),
        size,
        kids: Vec::new(),
    }
}

fn dir(rel: &str, kids: Vec<Node>) -> Node {
    Node {
        path: path(rel),
        name: String::new(),
        size: kids.iter().map(|kid| kid.size).sum(),
        kids,
    }
}

fn sample() -> Node {
    let mut root = dir(
        "",
        vec![
            dir(
                "Documents",
                vec![
                    dir(
                        "Documents/stl",
                        vec![
                            dir(
                                "Documents/stl/terrain",
                                vec![
                                    file("Documents/stl/terrain/cliffs.stl", 5 * GB),
                                    file("Documents/stl/terrain/ruins.stl", 4 * GB),
                                    file("Documents/stl/terrain/bridge.stl", 3 * GB),
                                ],
                            ),
                            dir(
                                "Documents/stl/minis",
                                vec![
                                    file("Documents/stl/minis/paladin.stl", 3 * GB),
                                    file("Documents/stl/minis/dragon.stl", 4 * GB),
                                    file("Documents/stl/minis/goblins.3mf", 2 * GB),
                                ],
                            ),
                            file("Documents/stl/dragon_bust.stl", 11 * GB),
                            file("Documents/stl/printer_base.3mf", 6 * GB),
                        ],
                    ),
                    dir(
                        "Documents/blender",
                        vec![
                            file("Documents/blender/city.blend", 5 * GB),
                            file("Documents/blender/ship.blend", 4 * GB),
                            dir(
                                "Documents/blender/renders",
                                vec![
                                    file("Documents/blender/renders/frame_0001.png", GB),
                                    file("Documents/blender/renders/frame_0002.png", GB),
                                    file("Documents/blender/renders/frame_0003.png", GB),
                                ],
                            ),
                        ],
                    ),
                    dir(
                        "Documents/obsidian-vault",
                        vec![
                            file("Documents/obsidian-vault/attachments.zip", 180 * MB),
                            file("Documents/obsidian-vault/daily.md", 100 * MB),
                            file("Documents/obsidian-vault/index.md", 20 * MB),
                        ],
                    ),
                    dir(
                        "Documents/dnd",
                        vec![
                            file("Documents/dnd/Edric_Paladin_L2.pdf", 15 * MB),
                            file("Documents/dnd/maps.zip", 360 * MB),
                            file("Documents/dnd/notes.md", 25 * MB),
                        ],
                    ),
                    dir(
                        "Documents/invoice",
                        vec![
                            file("Documents/invoice/invoice_09.pdf", 2 * MB),
                            file("Documents/invoice/invoice_10.pdf", 2 * MB),
                            file("Documents/invoice/order_details.pdf", 2 * MB),
                        ],
                    ),
                ],
            ),
            dir(
                "Downloads",
                vec![
                    file("Downloads/steam_backup.tar", 9 * GB),
                    file("Downloads/ubuntu-24.04.iso", 6 * GB),
                    file("Downloads/talk.mp4", 5 * GB),
                    dir(
                        "Downloads/dnd-maps",
                        vec![
                            file("Downloads/dnd-maps/tavern.png", 400 * MB),
                            file("Downloads/dnd-maps/keep.png", 600 * MB),
                        ],
                    ),
                    file("Downloads/billing_october_2026.txt", MB),
                ],
            ),
            dir(
                "Pictures",
                vec![
                    file("Pictures/camera.zip", 2 * GB),
                    file("Pictures/wallpapers.zip", 1536 * MB),
                    file("Pictures/screenshots.zip", 500 * MB),
                    file("Pictures/ehavgx_4k.jpg", MB),
                ],
            ),
        ],
    );
    root.name = "snek".to_string();
    root
}

#[derive(Clone, Debug)]
pub enum Message {
    Map(map::Event),
    Tick(Instant),
}

pub struct App {
    core: Core,
    map: Map,
    first: Option<Instant>,
    sent: bool,
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "com.system76.CosmicFilesMapDemo";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        let mut map = Map::new();
        map.set_tree(sample());
        map.set_overlays(vec![
            Overlay::Arrow {
                from: path("Downloads/billing_october_2026.txt"),
                to: path("Documents/invoice"),
            },
            Overlay::Merge {
                a: path("Documents/dnd"),
                b: path("Downloads/dnd-maps"),
            },
        ]);
        (
            Self {
                core,
                map,
                first: None,
                sent: false,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Map(event) => match event {
                map::Event::Zoom(path) => self.map.zoom_to(&path),
                map::Event::Open(path) => println!("open {}", path.display()),
                map::Event::Hover(_) => {}
                map::Event::Wheel(view) => self.map.wheel(view),
            },
            Message::Tick(now) => {
                // count from the first frame so a slow start does not eat the travel
                let first = *self.first.get_or_insert(now);
                if !self.sent && now.duration_since(first) >= Duration::from_secs(1) {
                    self.sent = true;
                    self.map.travel(Travel {
                        from: path("Downloads"),
                        to: path("Documents/invoice/order_details.pdf"),
                        started: now,
                    });
                }
                self.map.tick(now);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        self.map.view(Message::Map)
    }

    fn subscription(&self) -> Subscription<Message> {
        if !self.sent || self.map.is_animating() {
            window::frames().map(|(_, now)| Message::Tick(now))
        } else {
            Subscription::none()
        }
    }
}
