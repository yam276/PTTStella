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
            Self::Cancelled => f.write_str("視窗正在關閉；尚未開始的檔案操作已取消"),
            Self::Stale => f.write_str("畫面、規則或選取已變更；PNG 未儲存，請重新匯出"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}
type FileResult<T> = Result<T, Error>;
pub(super) enum Completion {
    Loaded(FileResult<Box<Initial>>),
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
            Self::Loaded(result) => result.err(),
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
}
impl Files {
    pub(super) fn busy(&self) -> bool {
        self.work.is_some()
    }
    /// Admission is synchronous; a rejected request never opens another picker.
    pub(super) fn start<F: Future<Output = Completion> + 'static>(
        &mut self,
        begin: impl FnOnce(Permit) -> F,
    ) -> Result<(), String> {
        self.permit.check().map_err(|e| e.to_string())?;
        if self.busy() {
            return Err("正在等待上一個檔案操作；請稍後再試".into());
        }
        self.work = Some(Box::pin(begin(self.permit.clone())));
        Ok(())
    }
    pub(super) async fn next(&mut self) -> Completion {
        let result = match self.work.as_mut() {
            Some(work) => work.await,
            None => std::future::pending().await,
        };
        self.work = None;
        result
    }
    pub(super) fn revoke(&self) {
        self.permit.0.store(true, Ordering::Release);
    }
    pub(super) async fn finish(&mut self) -> Result<(), String> {
        self.revoke();
        let error = if self.busy() {
            self.next().await.failure()
        } else {
            None
        };
        match error {
            None | Some(Error::Cancelled | Error::Stale) => Ok(()),
            Some(Error::Failed(error)) => Err(format!("關閉前的檔案操作失敗：{error}")),
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
    .map_err(|e| Error::Failed(format!("檔案工作未完成：{e}")))?
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
                preferences: Preferences::load(probe.as_deref())?,
                filter_store,
                rules,
                filter_warning,
            }))
        })
        .await,
    )
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
                return Err("請使用 .pttstella.json 副檔名".into());
            }
            storage::replace(&path, &PortableProfile::new(name, display)?.encode()?)?;
            Ok(ProfileResult::Exported {
                notice: "已匯出目前生效設定；未套用的草稿不包含在內".into(),
            })
        }
        ProfileRequest::Import => {
            let profile =
                PortableProfile::decode(&Store::at(path).read()?.ok_or("找不到顯示設定檔")?)?;
            let values = profile.display.resolve().map_err(str::to_owned)?;
            let sample = super::presentation::font_sample()?;
            Ok(ProfileResult::Imported {
                profile: Box::new(ImportedProfile {
                    profile: profile.display,
                    name: profile.name,
                    sample: sample_packet(&sample, &values),
                    values,
                }),
                notice: "設定已載入預覽；套用並儲存後才會生效".into(),
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
                .add_filter("PTTStella 顯示設定", &["json"]);
            let chosen = match &request {
                ProfileRequest::Import => dialog.set_title("載入顯示設定至預覽").pick_file().await,
                ProfileRequest::Export { .. } => {
                    dialog
                        .set_title("匯出目前生效的顯示設定")
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
        .ok_or("PNG 格式不正確")?;
    if encoded.len() > export::MAX_BASE64_BYTES {
        return Err("PNG 超過匯出大小上限".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "PNG 編碼不正確")?;
    export::check_export_header(&bytes, rows).map_err(str::to_owned)?;
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
            .set_title("匯出 PTT 畫面")
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
                return Err(Error::Failed("請使用 .png 副檔名；尚未儲存".into()));
            }
            storage::replace(&path, &bytes).map_err(Error::Failed)?;
            Ok(true)
        })
        .await
    }
    .await;
    Completion::Export { selected, result }
}
