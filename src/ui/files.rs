//! One retained file operation. Disk work never runs on the surface event loop.
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
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct Initial {
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
    Profile(FileResult<ProfileResult>),
    Export {
        selected: bool,
        result: FileResult<bool>,
    },
    Probe(FileResult<serde_json::Value>),
}
impl Completion {
    fn failure(self) -> Option<Error> {
        match self {
            Self::Bookmarks { result, .. } => result.err(),
            Self::Recent(done) => done.result.err().map(Error::Failed),
            Self::Loaded(result) => result.err(),
            Self::Fonts { result, .. } => result.err(),
            Self::Display(result) | Self::Hover(result) => result.err(),
            Self::Filters(result) => result.err(),
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
}
impl Files {
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

pub(super) struct ExportRequest {
    pub lease: super::surface_state::ExportLease,
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
        let window = dioxus::desktop::window();
        window.window.set_focus();
        rfd::AsyncFileDialog::new()
            .set_parent(window.window.as_ref())
            .set_title(pttstella::msg!("export-title"))
            .add_filter("PNG", &["png"])
            .set_file_name(if selected {
                format!("PTTStella-lines-{}-{}.png", rows.start + 1, rows.end)
            } else {
                "PTTStella.png".into()
            })
            .save_file()
            .await
            .map(|file| file.path().to_path_buf())
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
    Completion::Export { selected, result }
}
