//! Home preferences, filesystem snapshots and first-artboard thumbnail cache.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    #[default]
    Modified,
    Name,
    Created,
}
impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Modified => tl!("Last modified"),
            Self::Name => tl!("Name"),
            Self::Created => tl!("Date created"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub projects: Vec<PathBuf>,
    pub tips_hidden: bool,
    pub sort: Sort,
}

#[derive(Clone, Debug)]
pub(super) struct File {
    pub path: PathBuf,
    pub name: String,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub artboards: Option<usize>,
    pub thumbnail: Option<PathBuf>,
}
impl File {
    pub fn read(path: PathBuf) -> Self {
        let meta = std::fs::metadata(&path).ok();
        let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
        let cached = cache_root().and_then(|root| cached_in(&root, &path));
        Self {
            modified: meta.as_ref().and_then(|m| m.modified().ok()),
            created: meta.as_ref().and_then(|m| m.created().ok()),
            artboards: cached.as_ref().map(|(_, n)| *n),
            thumbnail: cached.map(|(p, _)| p),
            path,
            name,
        }
    }
    pub fn matches(&self, query: &str) -> bool {
        self.path.to_string_lossy().to_lowercase().contains(&query.to_lowercase())
    }
}

pub(super) fn sort(files: &mut [File], order: Sort) {
    files.sort_by(|a, b| {
        let name = || a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.path.cmp(&b.path));
        match order {
            Sort::Name => name(),
            Sort::Modified => b.modified.cmp(&a.modified).then_with(name),
            Sort::Created => b.created.cmp(&a.created).then_with(name),
        }
    });
}

pub(super) fn project_files(path: &Path) -> Vec<File> {
    if vectorcraft_engine::file_access::check_read(&path.to_string_lossy()).is_err() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(path) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("varos")) && p.is_file())
        .take(10_000)
        .map(File::read)
        .collect()
}

pub(super) fn greeting(hour: u32, name: &str) -> String {
    let source = match hour {
        5..=11 => tl!("Good morning, {name}"),
        12..=17 => tl!("Good afternoon, {name}"),
        _ => tl!("Good evening, {name}"),
    };
    crate::i18n::fmt(source, &[("name", name)])
}

pub(super) fn user_name() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        #[cfg(target_os = "macos")]
        if let Ok(out) = std::process::Command::new("/usr/bin/id").arg("-F").output()
            && out.status.success()
            && let Some(first) = String::from_utf8_lossy(&out.stdout).split_whitespace().next()
        {
            return first.to_owned();
        }
        std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| tl!("friend").into())
    })
    .clone()
}

pub(super) fn local_hour() -> u32 {
    // Ask the platform for its local clock; no hand-written timezone/calendar conversion.
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(out) = std::process::Command::new("date").arg("+%H").output()
        && out.status.success()
        && let Ok(hour) = String::from_utf8_lossy(&out.stdout).trim().parse::<u32>()
    {
        return hour;
    }
    vectorcraft_doc::metadata::now_unix().map_or(12, |t| ((t.rem_euclid(86400)) / 3600) as u32)
}

pub(super) fn edited(modified: Option<SystemTime>, now: SystemTime) -> String {
    let Some(at) = modified else { return tl!("Modification date unavailable").into() };
    let age = now.duration_since(at).unwrap_or_default().as_secs();
    let (label, n) = if age < 60 {
        return tl!("Edited just now").into();
    } else if age < 3600 {
        (if age < 120 { tl!("Edited {n} minute ago") } else { tl!("Edited {n} minutes ago") }, age / 60)
    } else if age < 86400 {
        (if age < 7200 { tl!("Edited {n} hour ago") } else { tl!("Edited {n} hours ago") }, age / 3600)
    } else {
        (if age < 172800 { tl!("Edited {n} day ago") } else { tl!("Edited {n} days ago") }, age / 86400)
    };
    crate::i18n::fmt(label, &[("n", &n.to_string())])
}

pub(super) fn cache_root() -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var_os("HOME").filter(|h| !h.is_empty()).map(|h| PathBuf::from(h).join("Library/Application Support/Varos/thumbs"))
    }
}

fn stamp(path: &Path) -> Option<(u128, u64)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos(), m.len()))
}

pub(super) fn cache_path(root: &Path, path: &Path) -> PathBuf {
    // Stable FNV-1a over the canonical path (not DefaultHasher's unspecified algorithm).
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    let hash = path.as_os_str().as_encoded_bytes().iter().fold(0xcbf29ce484222325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3));
    root.join(format!("{hash:016x}.png"))
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    source: PathBuf,
    stamp: Option<(u128, u64)>,
    artboards: usize,
}
fn cached_in(root: &Path, source: &Path) -> Option<(PathBuf, usize)> {
    let png = cache_path(root, source);
    let m: Manifest = serde_json::from_slice(&std::fs::read(png.with_extension("json")).ok()?).ok()?;
    (m.version == 1 && m.source == source && m.stamp.is_some() && m.stamp == stamp(source) && png.is_file()).then_some((png, m.artboards))
}

pub(super) fn write_thumbnail(root: &Path, path: &Path, doc: &vectorcraft_doc::Document) -> Result<PathBuf, String> {
    vectorcraft_engine::file_access::check_write(&root.to_string_lossy())?;
    let original_stamp = stamp(path);
    let region = doc.artboards.first().map(|a| a.rect).or_else(|| super::board_bounds(doc)).ok_or("No artboard to preview")?;
    if !region.width().is_finite() || !region.height().is_finite() || region.width() <= 0.0 || region.height() <= 0.0 {
        return Err("Invalid thumbnail bounds".into());
    }
    let scale = (480.0 / region.width()).min(360.0 / region.height());
    let mut renderer = vectorcraft_render::Renderer::new();
    let rendered = renderer.render_region(doc, region, scale, true);
    if stamp(path) != original_stamp {
        return Err("File changed during thumbnail rendering".into());
    }
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let png = cache_path(root, path);
    let temp = png.with_extension("png.tmp");
    image::save_buffer_with_format(&temp, &rendered.to_straight(), rendered.width, rendered.height, image::ColorType::Rgba8, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    std::fs::rename(temp, &png).map_err(|e| e.to_string())?;
    let manifest = Manifest { version: 1, source: path.to_owned(), stamp: original_stamp, artboards: doc.artboards.len() };
    let bytes = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(png.with_extension("json.tmp"), bytes).map_err(|e| e.to_string())?;
    std::fs::rename(png.with_extension("json.tmp"), png.with_extension("json")).map_err(|e| e.to_string())?;
    Ok(png)
}

/// Open/save callers pass the saved snapshot, never a subsequently edited active tab.
/// Cache errors are non-fatal and leave a placeholder; document I/O has already succeeded.
pub(crate) fn cache_document(path: &str, doc: &vectorcraft_doc::Document) {
    let Some(root) = cache_root() else { return };
    if !Path::new(path).is_file() || vectorcraft_engine::file_access::check_write(&root.to_string_lossy()).is_err() {
        return;
    }
    #[cfg(test)]
    if let Err(e) = write_thumbnail(&root, Path::new(path), doc) {
        log::warn!("Home thumbnail: {e}");
    }
    #[cfg(not(test))]
    {
        use std::sync::{Mutex, OnceLock, mpsc};
        type Job = (PathBuf, PathBuf, vectorcraft_doc::Document, Option<(u128, u64)>, vectorcraft_engine::file_access::ScopeHandle);
        static WORKER: OnceLock<Option<Mutex<mpsc::SyncSender<Job>>>> = OnceLock::new();
        let worker = WORKER.get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<Job>(16);
            std::thread::Builder::new()
                .name("varos-home-thumbnails".into())
                .spawn(move || {
                    while let Ok((root, path, doc, expected, scope)) = rx.recv() {
                        if stamp(&path) != expected {
                            continue;
                        } // A newer save superseded this job.
                        let result = scope.run(|| vectorcraft_engine::guard::catch_panic(|| write_thumbnail(&root, &path, &doc)));
                        if let Err(e) = result.and_then(|r| r) {
                            log::warn!("Home thumbnail: {e}");
                        }
                    }
                })
                .ok()
                .map(|_| Mutex::new(tx))
        });
        if let Some(worker) = worker
            && let Ok(tx) = worker.lock()
            && let Err(e) =
                tx.try_send((root, PathBuf::from(path), doc.clone(), stamp(Path::new(path)), vectorcraft_engine::file_access::ScopeHandle::capture()))
        {
            log::warn!("Home thumbnail queue: {e}");
        }
    }
}
