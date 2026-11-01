// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "desktop")]
use cosmic::desktop::fde::GenericEntry;
use mime_guess::Mime;
use rustc_hash::FxHashMap;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;
use std::{fs, process};

#[derive(Clone, Debug)]
pub struct Thumbnailer {
    pub exec: String,
}

impl Thumbnailer {
    pub fn command(
        &self,
        input: &Path,
        output: &Path,
        thumbnail_size: u32,
    ) -> Option<process::Command> {
        let args_vec: Vec<String> = shlex::split(&self.exec)?;
        let mut args = args_vec.iter();
        let mut command = process::Command::new(args.next()?);
        for arg in args {
            command.arg(self.expand(arg, input, output, thumbnail_size)?);
        }
        Some(command)
    }

    /// Fill in the Exec codes of one argument. Codes may sit inside an argument, as in f3d's
    /// `--output=%o --resolution=%s,%s`.
    fn expand(&self, arg: &str, input: &Path, output: &Path, thumbnail_size: u32) -> Option<OsString> {
        let mut expanded = OsString::new();
        let mut literal = String::new();
        let mut chars = arg.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                literal.push(c);
                continue;
            }
            let code = chars.next();
            if code == Some('%') {
                literal.push('%');
                continue;
            }
            expanded.push(&literal);
            literal.clear();
            match code {
                Some('i' | 'u') => expanded.push(input),
                Some('o') => expanded.push(output),
                Some('s') => expanded.push(thumbnail_size.to_string()),
                _ => {
                    log::warn!(
                        "unsupported thumbnailer Exec code {:?} in {:?}",
                        arg,
                        self.exec
                    );
                    return None;
                }
            }
        }
        expanded.push(literal);
        Some(expanded)
    }
}

pub struct ThumbnailerCache {
    cache: FxHashMap<Mime, Vec<Thumbnailer>>,
}

impl ThumbnailerCache {
    pub fn new() -> Self {
        let mut thumbnailer_cache = Self {
            cache: FxHashMap::default(),
        };
        thumbnailer_cache.reload();
        thumbnailer_cache
    }

    #[cfg(not(feature = "desktop"))]
    pub fn reload(&mut self) {}

    #[cfg(feature = "desktop")]
    pub fn reload(&mut self) {
        let start = Instant::now();

        self.cache.clear();

        let mut search_dirs = Vec::new();
        let xdg_dirs = xdg::BaseDirectories::new();

        if let Some(mut data_home) = xdg_dirs.get_data_home() {
            data_home.push("thumbnailers");
            search_dirs.push(data_home);
        }
        search_dirs.extend(xdg_dirs.get_data_dirs().into_iter().map(|mut data_dir| {
            data_dir.push("thumbnailers");
            data_dir
        }));

        let mut thumbnailer_paths = Vec::new();
        for dir in search_dirs {
            log::trace!("looking for thumbnailers in {}", dir.display());
            match fs::read_dir(&dir) {
                Ok(entries) => {
                    thumbnailer_paths.extend(entries.filter_map(|entry_res| {
                        entry_res
                            .inspect_err(|err| {
                                log::warn!(
                                    "failed to read entry in directory {}: {}",
                                    dir.display(),
                                    err
                                )
                            })
                            .ok()
                            .map(|entry| entry.path())
                    }));
                }
                Err(err) => {
                    log::warn!("failed to read directory {}: {}", dir.display(), err);
                }
            }
        }

        //TODO: handle directory specific behavior
        for path in thumbnailer_paths {
            let entry = match GenericEntry::from_path(&path) {
                Ok(ok) => ok,
                Err(err) => {
                    log::warn!("failed to parse {}: {}", path.display(), err);
                    continue;
                }
            };

            //TODO: use TryExec?
            let Some(section) = entry.group("Thumbnailer Entry") else {
                log::warn!(
                    "missing Thumbnailer Entry section for thumbnailer {}",
                    path.display()
                );
                continue;
            };
            let Some(exec) = section.entry("Exec") else {
                log::warn!("missing Exec attribute for thumbnailer {}", path.display());
                continue;
            };
            let Some(mime_types) = section.entry("MimeType") else {
                log::warn!(
                    "missing MimeType attribute for thumbnailer {}",
                    path.display()
                );
                continue;
            };

            for mime_type in mime_types.split_terminator(';') {
                if let Ok(mime) = mime_type.parse::<Mime>() {
                    log::trace!("thumbnailer {}={}", mime, path.display());
                    let apps = self
                        .cache
                        .entry(mime)
                        .or_insert_with(|| Vec::with_capacity(1));
                    apps.push(Thumbnailer {
                        exec: exec.to_string(),
                    });
                }
            }
        }

        let elapsed = start.elapsed();
        log::info!("loaded thumbnailer cache in {elapsed:?}");
    }

    pub fn get(&self, key: &Mime) -> Vec<Thumbnailer> {
        self.cache.get(key).map_or_else(Vec::new, Vec::clone)
    }
}

/// Where slicers leave their own render of the model inside a 3MF: the spec's package
/// thumbnail, then Bambu and Orca's first plate, then MakerWorld's.
const EMBEDDED_3MF: [&str; 3] = [
    "Metadata/thumbnail.png",
    "Metadata/plate_1.png",
    "Auxiliaries/.thumbnails/thumbnail_3mf.png",
];
const EMBEDDED_3MF_MAX_BYTES: u64 = 8 * 1000 * 1000;

/// The picture a slicer saved inside a 3MF. Slicer 3MFs keep the mesh in part files that
/// assimp (and so f3d) cannot read, but they carry this render; None when there is none.
pub fn embedded_3mf(path: &Path) -> Option<image::DynamicImage> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(fs::File::open(path).ok()?)
        .inspect_err(|err| log::warn!("failed to open 3MF {}: {}", path.display(), err))
        .ok()?;
    EMBEDDED_3MF.iter().find_map(|name| {
        let entry = archive.by_name(name).ok()?;
        if entry.size() > EMBEDDED_3MF_MAX_BYTES {
            return None;
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.take(EMBEDDED_3MF_MAX_BYTES).read_to_end(&mut data).ok()?;
        image::load_from_memory(&data)
            .inspect_err(|err| log::warn!("failed to decode {name} in {}: {}", path.display(), err))
            .ok()
    })
}

/// Whether a document's first page is a cover rather than a page of text: covers, posters and
/// decks ink most of the page, text pages a third of it at most.
pub fn is_cover(page: &image::RgbaImage) -> bool {
    let pixels = u64::from(page.width()) * u64::from(page.height());
    if pixels == 0 {
        return false;
    }
    let inked = page
        .pixels()
        .filter(|p| p[3] > 128 && p[0].min(p[1]).min(p[2]) < 225)
        .count() as u64;
    inked * 10 >= pixels * 4
}

static THUMBNAILER_CACHE: LazyLock<Mutex<ThumbnailerCache>> =
    LazyLock::new(|| Mutex::new(ThumbnailerCache::new()));

pub fn thumbnailer(mime: &Mime) -> Vec<Thumbnailer> {
    let thumbnailer_cache = THUMBNAILER_CACHE.lock().unwrap();
    thumbnailer_cache.get(mime)
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::Thumbnailer;
    use std::path::Path;

    #[test]
    fn command_expands_thumbnailer_arguments() {
        let thumbnailer = Thumbnailer {
            exec: "cosmic-files-thumbnailer %o --size %s %i".to_string(),
        };

        let command = thumbnailer
            .command(Path::new("input.exe"), Path::new("output.png"), 128)
            .expect("valid thumbnailer command");
        assert_eq!(command.get_program(), "cosmic-files-thumbnailer");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["output.png", "--size", "128", "input.exe"]
        );
    }

    #[test]
    fn command_expands_codes_inside_arguments() {
        let thumbnailer = Thumbnailer {
            exec: "f3d --output=%o --resolution=%s,%s 100%% %i".to_string(),
        };

        let command = thumbnailer
            .command(Path::new("model.stl"), Path::new("out.png"), 320)
            .expect("valid thumbnailer command");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--output=out.png", "--resolution=320,320", "100%", "model.stl"]
        );
    }
}
