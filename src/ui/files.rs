//! One retained file operation. Disk work never runs on the surface event loop.
#[path = "files_picture.rs"]
pub(super) mod picture;
#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
use super::{
    outbound::{ImportedProfile, ProfileResult},
    preferences::Preferences,
    presentation::sample_packet,
};
use base64::Engine;
use pttstella::{
    export, filters,
    settings::{AppSettings, DisplayProfile, PortableProfile, store::Store},
    storage,
};
use std::{
    future::Future,
    ops::Range,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct Initial {
    pub sites: super::sites::State,
    pub bookmarks: super::bookmarks::State,
    pub recent: super::recent::State,
    pub preferences: Preferences,
    pub filter_store: Result<filters::store::Store, String>,
    pub rules: filters::Rules,
    pub filter_warning: Option<String>,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Cancelled,
    Stale,
    Failed(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str(&pttstella::msg!("file-cancelled")),
            Self::Stale => f.write_str(&pttstella::msg!("export-stale")),
            Self::Failed(message) => f.write_str(message),
        }
    }
}
type FileResult<T> = Result<T, Error>;
pub(super) enum Completion {
    Clipboard(FileResult<()>),
    ClipboardPaste {
        identity: super::surface_state::Identity,
        stamp: pttstella::navigation::Stamp,
        result: FileResult<pttstella::input::PreparedPaste>,
    },
    Viewport(FileResult<()>),
    Picture {
        destination: ExportDestination,
        result: FileResult<bool>,
    },
    Sites {
        action: super::sites::Edit,
        request: u64,
        result: FileResult<pttstella::sites::Sites>,
    },
    Bookmarks {
        request: u64,
        result: FileResult<pttstella::bookmarks::Bookmarks>,
    },
    Recent(super::files_recent::Completed),
    Loaded(FileResult<Box<Initial>>),
    Fonts {
        request: u64,
        result: FileResult<crate::fonts::Catalog>,
    },
    Display(FileResult<AppSettings>),
    Hover(FileResult<AppSettings>),
    Filters(FileResult<filters::Rules>),
    FilterImport {
        request: u64,
        result: FileResult<Option<filters::import::Parsed>>,
    },
    Profile(FileResult<ProfileResult>),
    Export {
        selected: bool,
        destination: ExportDestination,
        result: FileResult<bool>,
    },
    Probe(FileResult<serde_json::Value>),
}
impl Completion {
    fn failure(self) -> Option<Error> {
        match self {
            Self::Clipboard(result) | Self::Viewport(result) => result.err(),
            Self::ClipboardPaste { result, .. } => result.err(),
            Self::Picture { result, .. } => result.err(),
            Self::Sites { result, .. } => result.err(),
            Self::Bookmarks { result, .. } => result.err(),
            Self::Recent(done) => done.result.err().map(Error::Failed),
            Self::Loaded(result) => result.err(),
            Self::Fonts { result, .. } => result.err(),
            Self::Display(result) | Self::Hover(result) => result.err(),
            Self::Filters(result) => result.err(),
            Self::FilterImport { result, .. } => result.err(),
            Self::Profile(result) => result.err(),
            Self::Export { result, .. } => result.err(),
            Self::Probe(result) => result.err(),
        }
    }
}
type Work = Pin<Box<dyn Future<Output = Completion>>>;
#[derive(Clone, Default)]
pub(super) struct Permit(Arc<AtomicBool>);
impl Permit {
    fn check(&self) -> FileResult<()> {
        if self.0.load(Ordering::Acquire) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}
#[derive(Default)]
pub(super) struct Files {
    work: Option<Work>,
    permit: Permit,
    recent: super::files_recent::Writer,
    clipboard: Clipboard,
}
pub(super) type Clipboard = Arc<Mutex<Option<arboard::Clipboard>>>;
impl Files {
    pub(super) fn clipboard(&self) -> Clipboard {
        self.clipboard.clone()
    }
    pub(super) fn busy(&self) -> bool {
        self.work.is_some() || self.recent.busy()
    }
    /// Admission is synchronous; a rejected request never opens another picker.
    pub(super) fn start<F: Future<Output = Completion> + 'static>(
        &mut self,
        begin: impl FnOnce(Permit) -> F,
    ) -> Result<(), String> {
        self.permit.check().map_err(|e| e.to_string())?;
        if self.work.is_some() {
            return Err(pttstella::msg!("file-busy"));
        }
        self.work = Some(Box::pin(begin(self.permit.clone())));
        Ok(())
    }
    pub(super) async fn next(&mut self) -> Completion {
        tokio::select! {
            result=async { match self.work.as_mut() { Some(work)=>work.await, None=>std::future::pending().await } }=>{
                self.work=None; result
            },
            done=self.recent.next()=>Completion::Recent(done),
        }
    }
    pub(super) fn save_recent(&mut self, write: super::files_recent::Write) -> Result<(), String> {
        self.permit.check().map_err(|e| e.to_string())?;
        self.recent.enqueue(write);
        Ok(())
    }
    pub(super) fn revoke(&self) {
        self.permit.0.store(true, Ordering::Release);
    }
    pub(super) async fn finish(&mut self) -> Result<(), String> {
        self.revoke();
        let mut errors = Vec::new();
        while self.busy() {
            if let Some(Error::Failed(error)) = self.next().await.failure() {
                errors.push(error);
            }
        }
        let clipboard = self.clipboard.clone();
        if let Err(error) = tokio::task::spawn_blocking(move || {
            let mut owner = clipboard
                .lock()
                .map_err(|_| pttstella::msg!("copy-failed"))?;
            *owner = None;
            Ok::<_, String>(())
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result)
        {
            errors.push(error);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(pttstella::msg!(
                "file-close-failed",
                error = errors.join("；")
            ))
        }
    }
}

async fn blocking<T: Send + 'static>(
    permit: Permit,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> FileResult<T> {
    blocking_result(permit, move || work().map_err(Error::Failed)).await
}
async fn blocking_result<T: Send + 'static>(
    permit: Permit,
    work: impl FnOnce() -> FileResult<T> + Send + 'static,
) -> FileResult<T> {
    permit.check()?;
    tokio::task::spawn_blocking(move || {
        permit.check()?;
        work()
    })
    .await
    .map_err(|e| Error::Failed(pttstella::msg!("file-worker-failed", e = e.to_string())))?
}
pub(super) async fn load(permit: Permit, probe: Option<PathBuf>) -> Completion {
    Completion::Loaded(
        blocking(permit, move || {
            let filter_store = probe
                .as_ref()
                .map_or_else(filters::store::Store::system, |p| {
                    Ok(filters::store::Store::at(p.join("probe-filters.json")))
                });
            let (rules, filter_warning) = match filter_store
                .as_ref()
                .map_err(Clone::clone)
                .and_then(filters::store::Store::load)
            {
                Ok(rules) => (rules, None),
                Err(error) => (filters::Rules::default(), Some(error)),
            };
            Ok(Box::new(Initial {
                sites: super::sites::State::load(probe.as_deref()),
                bookmarks: super::bookmarks::State::load(probe.as_deref()),
                recent: super::recent::State::load(probe.as_deref()),
                preferences: Preferences::load(probe.as_deref())?,
                filter_store,
                rules,
                filter_warning,
            }))
        })
        .await,
    )
}
pub(super) async fn font_catalog(permit: Permit, request: u64) -> Completion {
    Completion::Fonts {
        request,
        result: blocking(permit, crate::fonts::installed).await,
    }
}
pub(super) async fn save_display(
    permit: Permit,
    store: Result<Store, String>,
    draft: AppSettings,
) -> Completion {
    Completion::Display(blocking(permit, move || store?.save(draft)).await)
}
pub(super) async fn save_hover(
    permit: Permit,
    store: Result<Store, String>,
    draft: AppSettings,
) -> Completion {
    Completion::Hover(blocking(permit, move || store?.save(draft)).await)
}
pub(super) async fn save_filters(
    permit: Permit,
    store: Result<filters::store::Store, String>,
    rules: Result<filters::Rules, String>,
) -> Completion {
    Completion::Filters(blocking(permit, move || store?.save(rules?)).await)
}
pub(super) async fn save_bookmarks(
    permit: Permit,
    request: u64,
    write: super::bookmarks::Write,
) -> Completion {
    Completion::Bookmarks {
        request,
        result: blocking(permit, move || write.store.save(write.draft)).await,
    }
}
pub(super) async fn probe_report(
    permit: Permit,
    pixels: super::protocol::ProbePixels,
    path: PathBuf,
    reload: bool,
    settings: AppSettings,
) -> Completion {
    Completion::Probe(
        blocking(permit, move || {
            super::probe::pixels_report(pixels, &path, reload, &settings)
        })
        .await,
    )
}

/// Live canvas readback is confined to the explicit synthetic fullscreen probe.
pub(super) async fn viewport_capture(
    permit: Permit,
    capture: super::protocol::ViewportCapture,
    path: PathBuf,
) -> Completion {
    Completion::Viewport(blocking(permit, move || {
        let [width,height]=capture.pixels;
        if !(1..=8).contains(&capture.sequence) || width==0 || height==0 || width>8192 || height>8192
            || u64::from(width)*u64::from(height)>16*1024*1024 || capture.png.len()>export::MAX_BASE64_BYTES+32 {
            return Err("invalid viewport probe bounds".into());
        }
        let data=capture.png.strip_prefix("data:image/png;base64,").ok_or("invalid viewport PNG")?;
        let bytes=base64::engine::general_purpose::STANDARD.decode(data).map_err(|e|e.to_string())?;
        if bytes.len()>export::MAX_PNG_BYTES {return Err("viewport PNG too large".into());}
        let stem=format!("viewport-{}",capture.sequence);
        std::fs::write(path.join(format!("{stem}.png")),&bytes).map_err(|e|e.to_string())?;
        let ink=viewport_ink(&bytes,width,height)?;
        let metadata=serde_json::json!({"pixels":capture.pixels,"bounds":capture.bounds,"window":capture.window,"dpr":capture.dpr,"draws":capture.draws,"glyph_ink":ink});
        std::fs::write(path.join(format!("{stem}.json")),metadata.to_string()).map_err(|e|e.to_string())?;
        Ok(())
    }).await)
}

/// Independent fullscreen fixture oracle: every label cell has ink and every
/// row retains its literal ANSI background, including the first and last rows.
fn viewport_ink(bytes: &[u8], width: u32, height: u32) -> Result<Vec<[usize; 5]>, String> {
    let decoder = png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),
        png::Limits {
            bytes: 64 * 1024 * 1024,
        },
    );
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    if (reader.info().width, reader.info().height) != (width, height) || width < 80 || height < 24 {
        return Err("viewport dimensions differ".into());
    }
    let count = reader
        .output_buffer_size()
        .filter(|n| *n <= 64 * 1024 * 1024)
        .ok_or("viewport raster too large")?;
    let mut data = vec![0; count];
    let info = reader.next_frame(&mut data).map_err(|e| e.to_string())?;
    let channels = match (info.bit_depth, info.color_type) {
        (png::BitDepth::Eight, png::ColorType::Rgba) => 4,
        (png::BitDepth::Eight, png::ColorType::Rgb) => 3,
        _ => return Err("viewport format unsupported".into()),
    };
    let scale = (width as f64 / 960.0).min(height as f64 / 576.0);
    let x = |col| ((col as f64) * 12.0 * scale).round() as usize;
    let y = |row| ((row as f64) * 24.0 * scale).round() as usize;
    let rgb = |col: usize, row: usize| {
        let i = (row * width as usize + col) * channels;
        if channels == 4 && data[i + 3] != 255 {
            [0; 3]
        } else {
            [data[i], data[i + 1], data[i + 2]]
        }
    };
    let colours = [
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
    ];
    let mut rows = Vec::new();
    for row in 0..24 {
        if rgb(x(60), y(row)) != colours[row % 6] {
            return Err(format!("viewport row {} background missing", row + 1));
        }
        let mut counts = [0; 5];
        for (index, col) in [0, 1, 2, 4, 5].into_iter().enumerate() {
            for py in y(row)..y(row + 1) {
                for px in x(col)..x(col + 1) {
                    if rgb(px, py).into_iter().all(|v| v >= 170) {
                        counts[index] += 1;
                    }
                }
            }
        }
        if counts.contains(&0) {
            return Err(format!("viewport row {} label missing", row + 1));
        }
        rows.push(counts);
    }
    Ok(rows)
}

/// Explicit local fault fixture: an actual blocking worker retained until the
/// runner releases it. Never selected by a normal application launch.
pub(super) async fn slow_probe(permit: Permit, directory: PathBuf) -> Completion {
    Completion::Probe(
        blocking(permit, move || {
            std::fs::write(directory.join("file-started.json"), b"{\"started\":true}")
                .map_err(|e| e.to_string())?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            while !directory.join("file-release").exists() {
                if std::time::Instant::now() >= deadline {
                    return Err("synthetic disk fixture timed out".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            std::fs::write(directory.join("file-finished.json"), b"{\"finished\":true}")
                .map_err(|e| e.to_string())?;
            Ok(serde_json::json!({"synthetic_disk":"released"}))
        })
        .await,
    )
}

pub(super) enum ProfileRequest {
    Import,
    Export {
        name: String,
        display: DisplayProfile,
    },
}
pub(super) fn profile_at(
    request: ProfileRequest,
    path: Option<PathBuf>,
) -> Result<ProfileResult, String> {
    let Some(path) = path else {
        return Ok(ProfileResult::Cancelled);
    };
    match request {
        ProfileRequest::Export { name, display } => {
            if !path.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".pttstella.json")
            }) {
                return Err(pttstella::msg!("profile-extension"));
            }
            storage::replace(&path, &PortableProfile::new(name, display)?.encode()?)?;
            Ok(ProfileResult::Exported {
                notice: pttstella::msg!("profile-exported"),
            })
        }
        ProfileRequest::Import => {
            let profile = PortableProfile::decode(
                &Store::at(path)
                    .read()?
                    .ok_or(pttstella::msg!("profile-missing"))?,
            )?;
            let values = profile
                .display
                .resolve()
                .map_err(|error| error.to_string())?;
            let sample = super::presentation::font_sample()?;
            Ok(ProfileResult::Imported {
                profile: Box::new(ImportedProfile {
                    profile: profile.display,
                    name: profile.name,
                    sample: sample_packet(&sample, &values),
                    values,
                }),
                notice: pttstella::msg!("profile-imported"),
            })
        }
    }
}

pub(super) async fn profile(
    permit: Permit,
    request: ProfileRequest,
    probe: Option<PathBuf>,
) -> Completion {
    let result = async {
        permit.check()?;
        let path = if let Some(directory) = probe {
            Some(directory.join("display.pttstella.json"))
        } else {
            let window = dioxus::desktop::window();
            window.window.set_focus();
            let dialog = rfd::AsyncFileDialog::new()
                .set_parent(window.window.as_ref())
                .add_filter(pttstella::msg!("profile-file-type"), &["json"]);
            let chosen = match &request {
                ProfileRequest::Import => {
                    dialog
                        .set_title(pttstella::msg!("profile-import-title"))
                        .pick_file()
                        .await
                }
                ProfileRequest::Export { .. } => {
                    dialog
                        .set_title(pttstella::msg!("profile-export-title"))
                        .set_file_name("display.pttstella.json")
                        .save_file()
                        .await
                }
            };
            chosen.map(|file| file.path().to_path_buf())
        };
        blocking(permit, move || profile_at(request, path)).await
    }
    .await;
    Completion::Profile(result)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ExportDestination {
    #[default]
    File,
    Clipboard,
}

pub(super) struct ExportRequest {
    pub lease: super::surface_state::OutputLease,
    pub source: tokio::sync::watch::Receiver<pttstella::connection::View>,
    pub png: String,
    pub rows: Range<usize>,
    pub selected: bool,
    pub probe: Option<PathBuf>,
}
fn decode_png(png: String, rows: &Range<usize>) -> Result<Vec<u8>, String> {
    let encoded = png
        .strip_prefix("data:image/png;base64,")
        .ok_or(pttstella::msg!("export-invalid"))?;
    if encoded.len() > export::MAX_BASE64_BYTES {
        return Err(pttstella::msg!("export-too-large"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| pttstella::msg!("export-base64-invalid"))?;
    export::check_export_header(&bytes, rows).map_err(|error| error.to_string())?;
    Ok(bytes)
}
fn timestamp_stem() -> String {
    let now = time::OffsetDateTime::now_utc();
    let now = time::UtcOffset::local_offset_at(now).map_or(now, |offset| now.to_offset(offset));
    format!(
        "PTTStella-{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}{}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.millisecond(),
        if now.offset().is_utc() { "Z" } else { "" },
    )
}
async fn pick_export(
    rows: Range<usize>,
    selected: bool,
    probe: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(directory) = probe {
        Some(directory.join(if selected {
            "export-rows.png"
        } else {
            "export.png"
        }))
    } else {
        let stem = timestamp_stem();
        let filename = if selected {
            format!("{stem}-lines-{}-{}.png", rows.start + 1, rows.end)
        } else {
            format!("{stem}.png")
        };
        let window = dioxus::desktop::window();
        window.window.set_focus();
        rfd::AsyncFileDialog::new()
            .set_parent(window.window.as_ref())
            .set_title(pttstella::msg!("export-title"))
            .add_filter("PNG", &["png"])
            .set_file_name(filename)
            .save_file()
            .await
            .map(|file| file.path().to_path_buf())
    }
}

pub(super) async fn export_image(
    permit: Permit,
    request: ExportRequest,
    destination: ExportDestination,
    clipboard: Clipboard,
) -> Completion {
    match destination {
        ExportDestination::File => export_png(permit, request).await,
        ExportDestination::Clipboard => {
            let probe = request.probe.clone();
            let selected = request.selected;
            copy_png_with_writer(permit, request, move |image| {
                write_clipboard_image(
                    clipboard,
                    image,
                    probe.map(|directory| {
                        directory.join(if selected {
                            "clipboard-rows.png"
                        } else {
                            "clipboard.png"
                        })
                    }),
                )
            })
            .await
        }
    }
}
fn write_clipboard_image(
    clipboard: Clipboard,
    image: arboard::ImageData<'static>,
    probe: Option<PathBuf>,
) -> Result<(), String> {
    let mut owner = clipboard
        .lock()
        .map_err(|_| pttstella::msg!("copy-failed"))?;
    if owner.is_none() {
        *owner = Some(arboard::Clipboard::new().map_err(|_| pttstella::msg!("copy-failed"))?);
    }
    let clipboard = owner
        .as_mut()
        .ok_or_else(|| pttstella::msg!("copy-failed"))?;
    // Test-only readback after an explicit copy gesture; normal copies retain no image evidence.
    let expected = probe
        .as_ref()
        .map(|_| (image.width, image.height, image.bytes.clone().into_owned()));
    clipboard
        .set_image(image)
        .map_err(|_| pttstella::msg!("copy-failed"))?;
    if let (Some(path), Some((width, height, expected))) = (probe, expected) {
        let actual = clipboard
            .get_image()
            .map_err(|_| "clipboard probe readback failed".to_owned())?;
        if actual.width != width || actual.height != height || actual.bytes.as_ref() != expected {
            return Err("clipboard probe pixels differ".into());
        }
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer
                .write_image_data(&actual.bytes)
                .map_err(|e| e.to_string())?;
        }
        storage::replace(&path, &bytes)?;
    }
    Ok(())
}
fn clipboard_pixels(
    png: String,
    rows: &Range<usize>,
) -> Result<arboard::ImageData<'static>, String> {
    let bytes = decode_png(png, rows)?;
    let decoder = png::Decoder::new_with_limits(
        std::io::Cursor::new(bytes),
        png::Limits {
            bytes: 8 * 1024 * 1024,
        },
    );
    let mut reader = decoder
        .read_info()
        .map_err(|_| pttstella::msg!("export-invalid"))?;
    let count = reader
        .output_buffer_size()
        .filter(|n| *n <= (export::WIDTH * export::HEIGHT * 4) as usize)
        .ok_or_else(|| pttstella::msg!("export-too-large"))?;
    let mut data = vec![0; count];
    let info = reader
        .next_frame(&mut data)
        .map_err(|_| pttstella::msg!("export-invalid"))?;
    if info.width != export::WIDTH
        || info.height != rows.len() as u32 * export::ROW_HEIGHT
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err(pttstella::msg!("export-invalid"));
    }
    let rgba = match info.color_type {
        png::ColorType::Rgba => data,
        png::ColorType::Rgb => data[..info.buffer_size()]
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => return Err(pttstella::msg!("export-invalid")),
    };
    Ok(arboard::ImageData {
        width: info.width as usize,
        height: info.height as usize,
        bytes: rgba.into(),
    })
}
async fn copy_png_with_writer(
    permit: Permit,
    request: ExportRequest,
    write: impl FnOnce(arboard::ImageData<'static>) -> Result<(), String> + Send + 'static,
) -> Completion {
    let ExportRequest {
        png,
        rows,
        selected,
        lease,
        source,
        ..
    } = request;
    let result = blocking_result(permit, move || {
        lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
        let image = clipboard_pixels(png, &rows).map_err(Error::Failed)?;
        lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
        write(image).map_err(Error::Failed)?;
        Ok(true)
    })
    .await;
    Completion::Export {
        selected,
        destination: ExportDestination::Clipboard,
        result,
    }
}

pub(super) async fn export_png(permit: Permit, request: ExportRequest) -> Completion {
    export_with_picker(permit, request, pick_export).await
}
// The single production pipeline also accepts a deferred chooser in unit tests.
async fn export_with_picker<F: Future<Output = Option<PathBuf>>>(
    permit: Permit,
    request: ExportRequest,
    pick: impl FnOnce(Range<usize>, bool, Option<PathBuf>) -> F,
) -> Completion {
    let ExportRequest {
        png,
        rows,
        selected,
        probe,
        lease,
        source,
    } = request;
    let result = async {
        let decode_rows = rows.clone();
        let bytes = blocking(permit.clone(), move || decode_png(png, &decode_rows)).await?;
        permit.check()?;
        lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
        let path = pick(rows, selected, probe).await;
        permit.check()?;
        let Some(path) = path else {
            return Ok(false);
        };
        // Check immediately before starting disk work, including source watch
        // updates not yet observed by the UI reducer. Once begun, writes join.
        lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
        blocking_result(permit, move || {
            lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
            {
                return Err(Error::Failed(pttstella::msg!("export-extension")));
            }
            storage::replace(&path, &bytes).map_err(Error::Failed)?;
            Ok(true)
        })
        .await
    }
    .await;
    Completion::Export {
        selected,
        destination: ExportDestination::File,
        result,
    }
}

pub(super) async fn save_sites(
    permit: Permit,
    request: u64,
    write: super::sites::Write,
) -> Completion {
    Completion::Sites {
        action: write.action,
        request,
        result: blocking(permit, move || write.store.save(write.draft)).await,
    }
}

pub(super) async fn copy_text(
    permit: Permit,
    clipboard: Clipboard,
    text: String,
    lease: super::surface_state::OutputLease,
    source: tokio::sync::watch::Receiver<pttstella::connection::View>,
) -> Completion {
    copy_with_writer(permit, text, lease, source, move |text| {
        let mut owner = clipboard
            .lock()
            .map_err(|_| pttstella::msg!("copy-failed"))?;
        if owner.is_none() {
            *owner = Some(arboard::Clipboard::new().map_err(|_| pttstella::msg!("copy-failed"))?);
        }
        owner
            .as_mut()
            .ok_or_else(|| pttstella::msg!("copy-failed"))?
            .set_text(text)
            .map_err(|_| pttstella::msg!("copy-failed"))
    })
    .await
}

pub(super) async fn read_editor_clipboard(
    permit: Permit,
    clipboard: Clipboard,
    identity: super::surface_state::Identity,
    stamp: pttstella::navigation::Stamp,
    lease: super::surface_state::OutputLease,
    source: tokio::sync::watch::Receiver<pttstella::connection::View>,
) -> Completion {
    let result = read_editor_with_reader(permit, lease, source, move || {
        let mut owner = clipboard
            .lock()
            .map_err(|_| Error::Failed(pttstella::msg!("paste-unavailable")))?;
        if owner.is_none() {
            *owner = Some(
                arboard::Clipboard::new()
                    .map_err(|_| Error::Failed(pttstella::msg!("paste-unavailable")))?,
            );
        }
        owner
            .as_mut()
            .ok_or_else(|| Error::Failed(pttstella::msg!("paste-unavailable")))?
            .get_text()
            .map_err(|_| Error::Failed(pttstella::msg!("paste-unavailable")))
    })
    .await;
    Completion::ClipboardPaste {
        identity,
        stamp,
        result,
    }
}
async fn read_editor_with_reader(
    permit: Permit,
    lease: super::surface_state::OutputLease,
    source: tokio::sync::watch::Receiver<pttstella::connection::View>,
    read: impl FnOnce() -> FileResult<String> + Send + 'static,
) -> FileResult<pttstella::input::PreparedPaste> {
    blocking_result(permit, move || {
        lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
        let text = read()?;
        let view = source.borrow();
        lease.check(&view).map_err(|_| Error::Stale)?;
        pttstella::input::prepare_editor_paste(&text, view.site.encoding)
            .map_err(|e| Error::Failed(e.to_string()))
    })
    .await
}
async fn copy_with_writer(
    permit: Permit,
    text: String,
    lease: super::surface_state::OutputLease,
    source: tokio::sync::watch::Receiver<pttstella::connection::View>,
    write: impl FnOnce(String) -> Result<(), String> + Send + 'static,
) -> Completion {
    Completion::Clipboard(
        blocking_result(permit, move || {
            lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
            write(text).map_err(Error::Failed)
        })
        .await,
    )
}

pub(super) async fn import_filters(
    permit: Permit,
    request: u64,
    probe: Option<PathBuf>,
) -> Completion {
    let result = async {
        permit.check()?;
        let path = if let Some(directory) = probe {
            Some(directory.join("account-list.txt"))
        } else {
            let window = dioxus::desktop::window();
            rfd::AsyncFileDialog::new()
                .set_parent(window.window.as_ref())
                .set_title(pttstella::msg!("filters-import-title"))
                .add_filter(pttstella::msg!("filters-import-file"), &["txt"])
                .pick_file()
                .await
                .map(|file| file.path().to_path_buf())
        };
        blocking(permit, move || {
            let Some(path) = path else {
                return Ok(None);
            };
            use std::io::Read;
            let file = std::fs::File::open(path)
                .map_err(|e| pttstella::msg!("filters-read-failed", e = e.to_string()))?;
            let mut bytes = Vec::new();
            file.take((filters::MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            filters::import::parse(&bytes).map(Some)
        })
        .await
    }
    .await;
    Completion::FilterImport { request, result }
}

/// The settings window may retain one final edit across shutdown. Run only after
/// Files::finish has drained earlier writes; preserve unrelated committed fields.
pub(super) async fn flush_settings(
    store: Store,
    change: super::preferences::Change,
) -> Result<(), String> {
    blocking(Permit::default(), move || {
        let before = store.load()?;
        let app = change.apply(before.clone())?;
        if app != before {
            store.save(app)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())
}
