use crate::config::LiftConfig;
use crate::mode::LiftMode;
use crate::model::{LiftResult, LiftResultKind};
use halley_ui::assets::ImageData;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct IconCache {
    entries: HashMap<String, IconSlot>,
    target_size: u32,
    theme: String,
    decode_tx: Option<Sender<(String, String)>>,
    decode_rx: Option<Receiver<(String, Option<IconRaster>)>>,
    pending_decodes: usize,
    wake: Option<calloop::channel::Sender<()>>,
    /// Lazily rasterized search-bar glyphs, keyed by the size they were rendered at.
    search_icon: Option<(u32, IconRaster)>,
    app_search_icon: Option<(u32, IconRaster)>,
    cluster_search_icon: Option<(u32, IconRaster)>,
    action_search_icon: Option<(u32, IconRaster)>,
    term_icon: Option<(u32, IconRaster)>,
    config_search_icon: Option<(u32, IconRaster)>,
    selection_icon: Option<(u32, IconRaster)>,
}

/// Search-bar glyphs. Authored as square SVGs and rendered to alpha masks that are
/// tinted at draw time.
const SEARCH_ICON_SVG: &[u8] = include_bytes!("../assets/loupe.svg");
const APP_SEARCH_ICON_SVG: &[u8] = include_bytes!("../assets/apps.svg");
const ACTION_ICON_SVG: &[u8] = include_bytes!("../assets/spark.svg");
const TERM_ICON_SVG: &[u8] = include_bytes!("../assets/term.svg");
const CONFIG_ICON_SVG: &[u8] = include_bytes!("../assets/settings.svg");
const CLUSTER_SEARCH_ICON_SVG: &[u8] = include_bytes!("../assets/clusters.svg");
const SELECTION_ICON_SVG: &[u8] = include_bytes!("../assets/selected.svg");

/// State of a single icon in the in-memory cache. Decoding happens on a worker thread,
/// so a freshly requested icon is `Pending` until its raster arrives.
enum IconSlot {
    Pending,
    Ready(IconRaster),
    Missing { retry_at: Instant },
}

enum IconLookup<'a> {
    Ready(&'a IconRaster),
    Pending,
    Missing,
}

type IconRaster = Arc<ImageData>;

// Re-resolve paths selected by the old lookup, which could retain undersized bitmaps.
const ICON_PATH_CACHE_MAGIC: &str = "halley-lift-icon-paths-v2";
const MISSING_ICON_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Successful theme lookups are persisted by resolved theme and target size. Only positive
/// paths are stored: a missing icon may appear later after an application or icon-theme update.
struct IconPathCache {
    file: Option<PathBuf>,
    paths: HashMap<String, PathBuf>,
}

impl IconPathCache {
    fn load(theme: &str, target_size: u32) -> Self {
        Self::load_file(icon_path_cache_file(theme, target_size))
    }

    #[cfg(test)]
    fn load_from_file(file: PathBuf) -> Self {
        Self::load_file(Some(file))
    }

    fn load_file(file: Option<PathBuf>) -> Self {
        let mut paths = HashMap::new();
        if let Some(file) = file.as_ref()
            && let Ok(contents) = fs::read_to_string(file)
        {
            let mut lines = contents.lines();
            if lines.next() == Some(ICON_PATH_CACHE_MAGIC) {
                for line in lines {
                    let Some((encoded_name, encoded_path)) = line.split_once('\t') else {
                        continue;
                    };
                    let Some(name) =
                        decode_hex(encoded_name).and_then(|bytes| String::from_utf8(bytes).ok())
                    else {
                        continue;
                    };
                    let Some(path) = decode_hex(encoded_path)
                        .map(OsString::from_vec)
                        .map(PathBuf::from)
                        .filter(|path| path.is_file())
                    else {
                        continue;
                    };
                    paths.insert(name, path);
                }
            }
        }
        Self { file, paths }
    }

    fn get(&mut self, name: &str) -> Option<PathBuf> {
        let path = self.paths.get(name)?.clone();
        if path.is_file() {
            Some(path)
        } else {
            self.paths.remove(name);
            self.persist();
            None
        }
    }

    fn insert(&mut self, name: &str, path: &Path) {
        if self.paths.get(name).is_some_and(|cached| cached == path) {
            return;
        }
        self.paths.insert(name.to_string(), path.to_path_buf());
        self.persist();
    }

    fn remove(&mut self, name: &str) {
        if self.paths.remove(name).is_some() {
            self.persist();
        }
    }

    fn persist(&self) {
        let Some(file) = self.file.as_ref() else {
            return;
        };
        let Some(parent) = file.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }

        let mut entries = self.paths.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(name, _)| *name);
        let mut contents = String::from(ICON_PATH_CACHE_MAGIC);
        contents.push('\n');
        for (name, path) in entries {
            contents.push_str(&encode_hex(name.as_bytes()));
            contents.push('\t');
            contents.push_str(&encode_hex(path.as_os_str().as_bytes()));
            contents.push('\n');
        }

        let temporary = file.with_extension(format!("tmp-{}", std::process::id()));
        if fs::write(&temporary, contents).is_ok() {
            let _ = fs::rename(&temporary, file);
        }
    }
}

fn icon_path_cache_file(theme: &str, target_size: u32) -> Option<PathBuf> {
    let cache_home = std::env::var_os("XDG_CACHE_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|path| !path.is_empty())
                .map(|home| PathBuf::from(home).join(".cache"))
        })?;
    Some(cache_home.join("halley").join("lift-icons").join(format!(
        "{}-{target_size}.paths",
        encode_hex(theme.as_bytes())
    )))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_hex(encoded: &str) -> Option<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(digits, 16).ok()
        })
        .collect()
}

impl IconCache {
    pub fn new(config: &LiftConfig) -> Self {
        let target_size = config.icon_size.max(1);
        Self {
            entries: HashMap::new(),
            target_size,
            theme: config.icon_theme.clone(),
            decode_tx: None,
            decode_rx: None,
            pending_decodes: 0,
            wake: None,
            search_icon: None,
            app_search_icon: None,
            cluster_search_icon: None,
            action_search_icon: None,
            term_icon: None,
            config_search_icon: None,
            selection_icon: None,
        }
    }

    /// Returns the search-bar glyph rasterized at `size`, rendering (and caching) it on
    /// first use or whenever the requested size changes.
    pub(super) fn search_glyph(&mut self, size: u32, mode: LiftMode) -> Option<&IconRaster> {
        let size = size.max(1);
        let (slot, svg) = match mode {
            LiftMode::Apps => (&mut self.app_search_icon, APP_SEARCH_ICON_SVG),
            LiftMode::Clusters => (&mut self.cluster_search_icon, CLUSTER_SEARCH_ICON_SVG),
            LiftMode::Actions => (&mut self.action_search_icon, ACTION_ICON_SVG),
            LiftMode::Term => (&mut self.term_icon, TERM_ICON_SVG),
            LiftMode::Config => (&mut self.config_search_icon, CONFIG_ICON_SVG),
            _ => (&mut self.search_icon, SEARCH_ICON_SVG),
        };
        if slot.as_ref().map(|(s, _)| *s) != Some(size) {
            let raster = render_svg_data(svg, None, size)?;
            *slot = Some((size, raster));
        }
        slot.as_ref().map(|(_, raster)| raster)
    }

    pub(super) fn selection_glyph(&mut self, size: u32) -> Option<&IconRaster> {
        let size = size.max(1);
        if self.selection_icon.as_ref().map(|(s, _)| *s) != Some(size) {
            let raster = render_svg_data(SELECTION_ICON_SVG, None, size)?;
            self.selection_icon = Some((size, raster));
        }
        self.selection_icon.as_ref().map(|(_, raster)| raster)
    }

    pub fn set_waker(&mut self, wake: calloop::channel::Sender<()>) {
        self.wake = Some(wake);
    }

    /// Drain any icons the worker thread finished decoding into the cache. Returns true
    /// if a redraw is warranted because a previously pending icon became available.
    pub fn poll_decodes(&mut self) -> bool {
        let Some(rx) = self.decode_rx.as_ref() else {
            return false;
        };
        let mut changed = false;
        while let Ok((key, raster)) = rx.try_recv() {
            let slot = raster.map_or_else(
                || IconSlot::Missing {
                    retry_at: Instant::now() + MISSING_ICON_RETRY_DELAY,
                },
                IconSlot::Ready,
            );
            self.entries.insert(key, slot);
            self.pending_decodes = self.pending_decodes.saturating_sub(1);
            changed = true;
        }
        changed
    }

    fn load(&mut self, name: &str) -> IconLookup<'_> {
        let key = name.trim().to_string();
        if key.is_empty() {
            return IconLookup::Missing;
        }
        let should_retry = matches!(
            self.entries.get(&key),
            Some(IconSlot::Missing { retry_at }) if Instant::now() >= *retry_at
        );
        if should_retry {
            self.entries.remove(&key);
        }
        if !self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), IconSlot::Pending);
            self.ensure_decode_worker();
            let queued = self
                .decode_tx
                .as_ref()
                .is_some_and(|tx| tx.send((key.clone(), key.clone())).is_ok());
            if queued {
                self.pending_decodes += 1;
            } else {
                self.entries.insert(
                    key.clone(),
                    IconSlot::Missing {
                        retry_at: Instant::now() + MISSING_ICON_RETRY_DELAY,
                    },
                );
            }
        }
        match self.entries.get(&key) {
            Some(IconSlot::Ready(raster)) => IconLookup::Ready(raster),
            Some(IconSlot::Missing { .. }) => IconLookup::Missing,
            Some(IconSlot::Pending) | None => IconLookup::Pending,
        }
    }

    /// Spawn the decode worker on first use. It outlives individual requests and exits
    /// when the cache (and thus the job sender) is dropped.
    fn ensure_decode_worker(&mut self) {
        if self.decode_tx.is_some() {
            return;
        }
        let (job_tx, job_rx) = mpsc::channel::<(String, String)>();
        let (res_tx, res_rx) = mpsc::channel::<(String, Option<IconRaster>)>();
        let target_size = self.target_size;
        let theme = self.theme.clone();
        let wake = self.wake.clone();
        let jobs = Arc::new(Mutex::new(job_rx));
        let path_cache = Arc::new(Mutex::new(None::<IconPathCache>));
        for worker in 0..2 {
            let jobs = jobs.clone();
            let results = res_tx.clone();
            let theme = theme.clone();
            let wake = wake.clone();
            let path_cache = path_cache.clone();
            thread::Builder::new()
                .name(format!("halley-lift-icon-{worker}"))
                .spawn(move || {
                    let theme = if theme.trim().is_empty() || theme.eq_ignore_ascii_case("auto") {
                        freedesktop_icons::default_theme_gtk().unwrap_or_else(|| "hicolor".into())
                    } else {
                        theme
                    };
                    loop {
                        let job = jobs.lock().ok().and_then(|receiver| receiver.recv().ok());
                        let Some((key, name)) = job else { break };
                        let raster = load_requested_icon(&name, &theme, target_size, &path_cache);
                        if results.send((key, raster)).is_err() {
                            break;
                        }
                        if let Some(wake) = wake.as_ref() {
                            let _ = wake.send(());
                        }
                    }
                })
                .ok();
        }
        self.decode_tx = Some(job_tx);
        self.decode_rx = Some(res_rx);
    }
}

fn load_requested_icon(
    name: &str,
    theme: &str,
    target_size: u32,
    shared_cache: &Mutex<Option<IconPathCache>>,
) -> Option<IconRaster> {
    let cached_path = shared_cache.lock().ok().and_then(|mut shared| {
        shared
            .get_or_insert_with(|| IconPathCache::load(theme, target_size))
            .get(name)
    });
    if let Some(path) = cached_path {
        if let Some(raster) = load_icon(path.as_path(), target_size) {
            return Some(raster);
        }
        if let Ok(mut shared) = shared_cache.lock()
            && let Some(cache) = shared.as_mut()
        {
            cache.remove(name);
        }
    }

    let path = resolve_icon_path_direct(name, theme, target_size)?;
    let raster = load_icon(path.as_path(), target_size)?;
    if let Ok(mut shared) = shared_cache.lock() {
        shared
            .get_or_insert_with(|| IconPathCache::load(theme, target_size))
            .insert(name, path.as_path());
    }
    Some(raster)
}

fn resolve_icon_path_direct(name: &str, theme: &str, target_size: u32) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() && path.is_file() {
        return Some(path.to_path_buf());
    }
    let lookup_name = icon_lookup_name(name);
    resolve_sized_icon(target_size, |size| {
        freedesktop_icons::lookup(lookup_name)
            .with_size(size)
            .with_theme(theme)
            .force_svg()
            .find()
    })
}

/// Keep the normal theme lookup unless it supplies an undersized bitmap. Probe
/// larger sizes through the same resolver, without walking or indexing the icon tree.
fn resolve_sized_icon(
    target_size: u32,
    mut lookup: impl FnMut(u16) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let target_size = target_size.clamp(1, 4096);
    let mut size = target_size;
    let mut best = None;
    let mut best_dimension = 0;
    loop {
        if let Some(path) = lookup(size as u16) {
            if is_svg(&path) {
                return Some(path);
            }
            let dimension = image::ImageReader::open(&path)
                .ok()
                .and_then(|reader| reader.with_guessed_format().ok())
                .and_then(|reader| reader.into_dimensions().ok())
                .map(|(width, height)| width.max(height))
                .unwrap_or(0);
            if dimension >= target_size {
                return Some(path);
            }
            if best.is_none() || dimension > best_dimension {
                best = Some(path);
                best_dimension = dimension;
            }
        } else if best.is_none() {
            return None;
        }
        if size == 4096 {
            return best;
        }
        size = (size * 2).min(4096);
    }
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

/// Desktop icon identifiers commonly contain dots (for example,
/// org.mozilla.Thunderbird). Strip only a real image suffix supplied in an Icon= value.
fn icon_lookup_name(name: &str) -> &str {
    let path = Path::new(name);
    let has_image_suffix = path
        .extension()
        .and_then(|suffix| suffix.to_str())
        .is_some_and(|suffix| {
            matches!(
                suffix.to_ascii_lowercase().as_str(),
                "png" | "svg" | "jpg" | "jpeg" | "xpm" | "xmp"
            )
        });
    if has_image_suffix {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name)
    } else {
        name
    }
}

fn load_icon(path: &Path, target_size: u32) -> Option<IconRaster> {
    if is_svg(path) {
        return render_svg_data(&fs::read(path).ok()?, Some(path), target_size);
    }
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let mut pixels = reader.decode().ok()?.into_rgba8();
    // Filter premultiplied pixels so transparent edges do not acquire dark halos.
    for pixel in pixels.pixels_mut() {
        for channel in 0..3 {
            pixel[channel] = ((u16::from(pixel[channel]) * u16::from(pixel[3]) + 127) / 255) as u8;
        }
    }
    let target_size = target_size.clamp(1, 4096);
    let dimension = pixels.width().max(pixels.height());
    if dimension > target_size {
        let width = (u64::from(pixels.width()) * u64::from(target_size) / u64::from(dimension))
            .max(1) as u32;
        let height = (u64::from(pixels.height()) * u64::from(target_size) / u64::from(dimension))
            .max(1) as u32;
        pixels = image::imageops::resize(
            &pixels,
            width,
            height,
            image::imageops::FilterType::Lanczos3,
        );
        // Lanczos ringing can exceed alpha; retain the premultiplied invariant.
        for pixel in pixels.pixels_mut() {
            for channel in 0..3 {
                pixel[channel] = pixel[channel].min(pixel[3]);
            }
        }
    }
    ImageData::from_rgba(pixels.width(), pixels.height(), pixels.into_raw().into())
        .ok()
        .map(Arc::new)
}
fn render_svg_data(data: &[u8], path: Option<&Path>, size: u32) -> Option<IconRaster> {
    let options = resvg::usvg::Options {
        resources_dir: path.and_then(Path::parent).map(Path::to_path_buf),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_data(data, &options).ok()?;
    let original = tree.size();
    let scale = size.clamp(1, 4096) as f32 / original.width().max(original.height());
    let width = (original.width() * scale).round().max(1.0) as u32;
    let height = (original.height() * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    ImageData::from_rgba(width, height, pixmap.take().into())
        .ok()
        .map(Arc::new)
}
impl IconCache {
    pub(super) fn result_icon(
        &mut self,
        result: &LiftResult,
        config: &LiftConfig,
    ) -> Option<(Arc<ImageData>, bool)> {
        if let Some(name) = result.icon_name.as_deref() {
            match self.load(name) {
                IconLookup::Ready(data) => return Some((data.clone(), false)),
                IconLookup::Pending => return None,
                IconLookup::Missing => {}
            }
        }
        let mode = match result.kind {
            LiftResultKind::App => LiftMode::Apps,
            LiftResultKind::Cluster | LiftResultKind::CreateCluster => LiftMode::Clusters,
            LiftResultKind::Term => LiftMode::Term,
            LiftResultKind::Action => LiftMode::Actions,
            LiftResultKind::Config => LiftMode::Config,
            LiftResultKind::Node => LiftMode::Nodes,
        };
        self.search_glyph(config.icon_size, mode)
            .map(|image| (image.clone(), true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app_result() -> LiftResult {
        LiftResult {
            section: "Apps".into(),
            title: "App".into(),
            subtitle: None,
            icon_name: Some("test-app".into()),
            kind: LiftResultKind::App,
            score: 0.0,
            is_field_pinned: false,
            shortcut_hint: None,
            action: crate::model::LiftAction::ReloadConfig,
        }
    }
    #[test]
    fn pending_icon_stays_blank_and_missing_icon_uses_a_tinted_glyph() {
        let config = LiftConfig::default();
        let mut cache = IconCache::new(&config);
        cache.entries.insert("test-app".into(), IconSlot::Pending);
        assert!(cache.result_icon(&app_result(), &config).is_none());
        cache.entries.insert(
            "test-app".into(),
            IconSlot::Missing {
                retry_at: Instant::now() + Duration::from_secs(30),
            },
        );
        let (image, tinted) = cache.result_icon(&app_result(), &config).unwrap();
        assert!(tinted);
        assert!(image.pixels().chunks_exact(4).any(|p| p[3] != 0));
    }

    #[test]
    fn dotted_icon_identifiers_are_not_mistaken_for_file_names() {
        assert_eq!(
            icon_lookup_name("org.mozilla.Thunderbird"),
            "org.mozilla.Thunderbird"
        );
        assert_eq!(
            icon_lookup_name("com.obsproject.Studio"),
            "com.obsproject.Studio"
        );
        assert_eq!(icon_lookup_name("firefox.png"), "firefox");
        assert_eq!(icon_lookup_name("symbolic-icon.SVG"), "symbolic-icon");
    }

    #[test]
    fn small_svg_is_rasterized_at_the_requested_size_with_its_aspect_ratio() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8"><path d="M0 8 L16 0 L16 8 Z" fill="white"/></svg>"#;
        for (size, expected) in [(28, (28, 14)), (8, (8, 4))] {
            let icon = render_svg_data(svg, None, size).unwrap();
            assert_eq!((icon.width(), icon.height()), expected);
            assert!(
                icon.pixels()
                    .chunks_exact(4)
                    .any(|p| p[3] > 0 && p[3] < 255)
            );
        }
    }

    struct RasterFixture(PathBuf);

    impl RasterFixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "halley-lift-raster-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn save(&self, name: &str, pixels: image::RgbaImage) -> PathBuf {
            let path = self.0.join(name);
            pixels.save(&path).unwrap();
            path
        }
    }

    impl Drop for RasterFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn undersized_bitmap_lookup_selects_a_larger_source_and_remains_bounded() {
        let fixture = RasterFixture::new();
        let small = fixture.save("small.png", image::RgbaImage::new(16, 16));
        let large = fixture.save("large.png", image::RgbaImage::new(64, 64));
        let mut requested = Vec::new();
        let chosen = resolve_sized_icon(28, |size| {
            requested.push(size);
            Some(if size < 64 {
                small.clone()
            } else {
                large.clone()
            })
        });
        assert_eq!(chosen, Some(large));
        assert_eq!(requested, vec![28, 56, 112]);

        requested.clear();
        assert_eq!(
            resolve_sized_icon(28, |size| {
                requested.push(size);
                Some(small.clone())
            }),
            Some(small)
        );
        assert_eq!(requested.last(), Some(&4096));
        assert!(requested.len() <= 9);

        requested.clear();
        assert!(
            resolve_sized_icon(28, |size| {
                requested.push(size);
                None
            })
            .is_none()
        );
        assert_eq!(requested, vec![28]);
    }

    #[test]
    fn bitmap_resampling_preserves_transparent_edges_and_aspect_ratio() {
        let fixture = RasterFixture::new();
        let pixels = image::RgbaImage::from_fn(64, 32, |x, _| {
            if x < 31 {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([255, 0, 0, 0])
            }
        });
        let path = fixture.save("edge.png", pixels);
        let icon = load_icon(&path, 28).unwrap();
        assert_eq!((icon.width(), icon.height()), (28, 14));
        assert!(
            icon.pixels()
                .chunks_exact(4)
                .any(|p| p[3] > 0 && p[3] < 255)
        );
        for pixel in icon.pixels().chunks_exact(4) {
            // Invisible red pixels must not bleed into the filtered white edge.
            assert_eq!(pixel[0], pixel[1]);
            assert_eq!(pixel[1], pixel[2]);
            assert!(pixel[0] <= pixel[3]);
        }
    }

    #[test]
    fn old_path_cache_does_not_retain_undersized_lookup_results() {
        let fixture = RasterFixture::new();
        let icon = fixture.save("small.png", image::RgbaImage::new(16, 16));
        let file = fixture.0.join("paths");
        fs::write(
            &file,
            format!(
                "halley-lift-icon-paths-v1\n{}\t{}\n",
                encode_hex(b"app"),
                encode_hex(icon.as_os_str().as_bytes())
            ),
        )
        .unwrap();
        assert!(IconPathCache::load_from_file(file).get("app").is_none());
    }

    #[test]
    fn persistent_icon_path_cache_round_trips_and_discards_stale_paths() {
        let root = std::env::temp_dir().join(format!(
            "halley-lift-icon-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after Unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temporary cache directory");
        let icon = root.join("org.example.App.svg");
        fs::write(&icon, "<svg/>").expect("temporary icon");
        let file = root.join("paths");

        let mut cache = IconPathCache {
            file: Some(file.clone()),
            paths: HashMap::new(),
        };
        cache.insert("org.example.App", &icon);

        let mut warm = IconPathCache::load_from_file(file.clone());
        assert_eq!(warm.get("org.example.App"), Some(icon.clone()));

        fs::remove_file(&icon).expect("remove temporary icon");
        assert_eq!(warm.get("org.example.App"), None);
        let reloaded = IconPathCache::load_from_file(file);
        assert!(!reloaded.paths.contains_key("org.example.App"));

        fs::remove_dir_all(root).expect("remove temporary cache directory");
    }

    #[test]
    fn expired_missing_icon_is_retried_instead_of_cached_forever() {
        let mut cache = IconCache::new(&LiftConfig::default());
        cache.entries.insert(
            "new-app-icon".into(),
            IconSlot::Missing {
                retry_at: Instant::now(),
            },
        );
        assert!(matches!(cache.load("new-app-icon"), IconLookup::Pending));
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn demand_lookup_queues_only_the_requested_icon() {
        let mut cache = IconCache::new(&LiftConfig::default());
        assert!(matches!(cache.load("new-app-icon"), IconLookup::Pending));
        assert_eq!(cache.entries.len(), 1);
    }
}
