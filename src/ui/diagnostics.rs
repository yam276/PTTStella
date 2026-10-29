//! Opt-in interaction diagnostics. Never accepts screen, key, URL or credential text.
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const FILE_BYTES: u64 = 2 * 1024 * 1024;
const FILE_COUNT: usize = 4;
const QUEUE: usize = 256;
const FINISHED_RUNS: usize = 8;
const RUN_PREFIX: &str = "pttstella-run-";
const OWNER: &[u8] = b"PTTStella diagnostics v1\n";

pub(crate) fn diagnostic_build() -> bool {
    option_env!("PTTSTELLA_DIAGNOSTIC_BUILD") == Some("1")
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WebTrace {
    pub event: WebEvent,
    pub phase: u8,
    pub flags: u32,
    pub target: u8,
    pub generation: u64,
    pub revision: u64,
    pub presentation: u64,
    pub elapsed_ms: u32,
    pub scroll: u32,
    pub extent: u32,
    pub suppressed: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WebEvent {
    Pulse,
    Key,
    Text,
    PointerDown,
    PointerUp,
    Click,
    Wheel,
    Focus,
    Visibility,
    Composition,
    Navigate,
    ReadingMove,
    Packet,
    Marker,
    Error,
}

#[derive(Clone, Serialize)]
pub(super) struct State {
    pub generation: u64,
    pub revision: u64,
    pub input_epoch: u64,
    pub received_bytes: usize,
    pub phase: &'static str,
    pub input_pending: bool,
    pub output_pending: bool,
    pub paint_pending: bool,
    pub dirty: bool,
    pub command_busy: bool,
    pub files_busy: bool,
    pub login_open: bool,
    pub settings_open: bool,
    pub paste_pending: bool,
    pub full_load: bool,
    pub keyboard_pending: bool,
}

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event {
    Start {
        version: &'static str,
        os: &'static str,
        arch: &'static str,
    },
    State(State),
    Web {
        trace: WebTrace,
    },
    Bridge {
        direction: &'static str,
        kind: &'static str,
        success: bool,
        generation: u64,
        revision: u64,
        presentation: u64,
    },
    Watchdog {
        ui_age_ms: u64,
        web_age_ms: u64,
        dropped: u64,
    },
    Finish {
        run_ok: bool,
        cleanup_ok: bool,
    },
}
struct Entry {
    at_ms: u64,
    event: Event,
}
struct Shared {
    start: Instant,
    ui: AtomicU64,
    web: AtomicU64,
    dropped: AtomicU64,
    stop: AtomicBool,
    failed: AtomicBool,
    finish: AtomicU8,
}

#[derive(Clone, Default)]
pub(super) struct Handle {
    inner: Option<(mpsc::SyncSender<Entry>, Arc<Shared>)>,
}
impl Handle {
    pub(super) fn enabled(&self) -> bool {
        self.inner.is_some()
    }
    fn record(&self, event: Event) {
        if let Some((sender, shared)) = &self.inner {
            if shared.stop.load(Ordering::Acquire) {
                return;
            }
            let entry = Entry {
                at_ms: millis(shared.start.elapsed()),
                event,
            };
            if sender.try_send(entry).is_err() {
                shared.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    pub(super) fn state(&self, state: State) {
        if let Some((_, shared)) = &self.inner {
            shared
                .ui
                .store(millis(shared.start.elapsed()), Ordering::Relaxed);
            self.record(Event::State(state));
        }
    }
    pub(super) fn web(&self, trace: WebTrace) {
        if let Some((_, shared)) = &self.inner {
            shared
                .web
                .store(millis(shared.start.elapsed()), Ordering::Relaxed);
            self.record(Event::Web { trace });
        }
    }
    pub(super) fn bridge(
        &self,
        direction: &'static str,
        kind: &'static str,
        success: bool,
        generation: u64,
        revision: u64,
        presentation: u64,
    ) {
        self.record(Event::Bridge {
            direction,
            kind,
            success,
            generation,
            revision,
            presentation,
        });
    }
    pub(super) fn failed(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(|(_, shared)| shared.failed.load(Ordering::Relaxed))
    }
}

pub(super) struct Journal {
    handle: Handle,
    worker: Option<JoinHandle<std::io::Result<()>>>,
}
impl Journal {
    pub(super) fn start(probe: Option<&Path>) -> Result<Option<Self>, String> {
        let configured = std::env::var_os("PTTSTELLA_DIAGNOSTICS_DIR");
        if configured.is_none() && !diagnostic_build() {
            return Ok(None);
        }
        let directory = match configured {
            Some(path) => PathBuf::from(path),
            None => match probe {
                Some(path) => path.join("diagnostics"),
                None => directories::ProjectDirs::from("", "", "PTTStella")
                    .ok_or("Cannot locate diagnostic directory")?
                    .data_local_dir()
                    .join("diagnostics"),
            },
        };
        Self::isolated(&directory, FILE_BYTES)
            .map(Some)
            .map_err(|error| format!("診斷紀錄無法啟動（{}）：{error}", directory.display()))
    }
    fn isolated(root: &Path, limit: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(root)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let directory = tempfile::Builder::new()
            .prefix(&format!("{RUN_PREFIX}{stamp:020}-{}-", std::process::id()))
            .tempdir_in(root)?;
        options()
            .write(true)
            .create_new(true)
            .open(directory.path().join("owner"))?
            .write_all(OWNER)?;
        let journal = Self::at(directory.path(), limit)?;
        let _ = directory.keep();
        prune_finished(root);
        Ok(journal)
    }
    fn at(directory: &Path, limit: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(directory)?;
        let lock = options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("writer.lock"))?;
        lock.try_lock().map_err(std::io::Error::other)?;
        let writer = Rotating::new(directory.to_path_buf(), limit)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE);
        let shared = Arc::new(Shared {
            start: Instant::now(),
            ui: AtomicU64::new(0),
            web: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            finish: AtomicU8::new(0),
        });
        let handle = Handle {
            inner: Some((sender, shared.clone())),
        };
        let session = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let retention_root = directory
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.starts_with(RUN_PREFIX))
            .and_then(|_| directory.parent())
            .map(Path::to_path_buf);
        let worker = std::thread::Builder::new()
            .name("pttstella-diagnostics".into())
            .spawn(move || {
                let result = write_loop(writer, receiver, &shared, session);
                if result.is_err() {
                    shared.failed.store(true, Ordering::Relaxed);
                }
                drop(lock);
                if let Some(root) = retention_root {
                    prune_finished(&root);
                }
                result
            })?;
        handle.record(Event::Start {
            version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        });
        Ok(Self {
            handle,
            worker: Some(worker),
        })
    }
    pub(super) fn handle(&self) -> Handle {
        self.handle.clone()
    }
    pub(super) async fn finish(&mut self, run_ok: bool, cleanup_ok: bool) -> Result<(), String> {
        if let Some((_, shared)) = &self.handle.inner {
            shared.finish.store(
                u8::from(run_ok) | (u8::from(cleanup_ok) << 1),
                Ordering::Relaxed,
            );
        }
        self.stop();
        if let Some(worker) = self.worker.take() {
            tokio::task::spawn_blocking(move || worker.join())
                .await
                .map_err(|_| "diagnostic join task failed")?
                .map_err(|_| "diagnostic writer panicked")?
                .map_err(|error| format!("診斷紀錄寫入失敗：{error}"))?;
        }
        Ok(())
    }
    fn stop(&self) {
        if let Some((_, shared)) = &self.handle.inner {
            shared.stop.store(true, Ordering::Release);
        }
    }
}

/// Only our marked, inactive, exact-layout run directories are disposable.
/// The root lock serializes pruning; active writers and legacy logs are retained.
fn prune_finished(root: &Path) {
    let Ok(root_lock) = options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("retention.lock"))
    else {
        return;
    };
    if root_lock.try_lock().is_err() {
        return;
    }
    let Ok(children) = std::fs::read_dir(root) else {
        return;
    };
    let mut finished = Vec::new();
    // An unexpectedly large/mixed directory is retained, never scanned unboundedly.
    let children: Vec<_> = children.take(513).collect();
    if children.len() > 512 {
        return;
    }
    for child in children.into_iter().flatten() {
        let path = child.path();
        let Ok(kind) = child.file_type() else {
            continue;
        };
        if !kind.is_dir() || !child.file_name().to_string_lossy().starts_with(RUN_PREFIX) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&path) else {
            continue;
        };
        let mut names = Vec::new();
        let mut safe = true;
        for entry in entries.take(8) {
            let Ok(entry) = entry else {
                safe = false;
                break;
            };
            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                safe = false;
                break;
            }
            let name = entry.file_name();
            if !matches!(
                name.to_str(),
                Some(
                    "owner"
                        | "writer.lock"
                        | "events-0.jsonl"
                        | "events-1.jsonl"
                        | "events-2.jsonl"
                        | "events-3.jsonl"
                )
            ) {
                safe = false;
                break;
            }
            names.push(name);
        }
        if !safe
            || !names.iter().any(|name| name == "writer.lock")
            || !names.iter().any(|name| name == "owner")
        {
            continue;
        }
        // Never read an unbounded or linked ownership file.
        let Ok(Some(owner)) = pttstella::storage::read_bounded(&path.join("owner"), OWNER.len())
        else {
            continue;
        };
        if owner != OWNER {
            continue;
        }
        let Ok(lock) = options()
            .read(true)
            .write(true)
            .open(path.join("writer.lock"))
        else {
            continue;
        };
        if lock.try_lock().is_err() {
            continue;
        }
        finished.push((child.file_name(), path, lock));
    }
    finished.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path, lock) in finished.into_iter().skip(FINISHED_RUNS) {
        // Session names are unique and never reused. Drop handles for Windows.
        drop(lock);
        let _ = std::fs::remove_dir_all(path);
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        self.stop();
        // Normal shutdown joins off the UI thread. This fallback retains ownership
        // if preflight or its future is dropped before the async finish boundary.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}
fn options() -> OpenOptions {
    let options = OpenOptions::new();
    #[cfg(unix)]
    let options = {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = options;
        options.mode(0o600);
        options
    };
    options
}
struct Rotating {
    directory: PathBuf,
    file: Option<File>,
    bytes: u64,
    limit: u64,
}
impl Rotating {
    fn new(directory: PathBuf, limit: u64) -> std::io::Result<Self> {
        let file = options()
            .append(true)
            .create(true)
            .open(directory.join("events-0.jsonl"))?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            directory,
            file: Some(file),
            bytes,
            limit,
        })
    }
    fn write(&mut self, value: &impl Serialize) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec(value)?;
        bytes.push(b'\n');
        if bytes.len() as u64 > self.limit {
            return Err(std::io::Error::other(
                "diagnostic record exceeds file budget",
            ));
        }
        if self.bytes + bytes.len() as u64 > self.limit {
            self.file.take();
            for index in (1..FILE_COUNT).rev() {
                let old = self.directory.join(format!("events-{}.jsonl", index - 1));
                let new = self.directory.join(format!("events-{index}.jsonl"));
                if new.exists() {
                    std::fs::remove_file(&new)?;
                }
                if old.exists() {
                    std::fs::rename(old, new)?;
                }
            }
            self.file = Some(
                options()
                    .append(true)
                    .create_new(true)
                    .open(self.directory.join("events-0.jsonl"))?,
            );
            self.bytes = 0;
        }
        self.file
            .as_mut()
            .ok_or_else(|| std::io::Error::other("diagnostic file unavailable"))?
            .write_all(&bytes)?;
        self.bytes += bytes.len() as u64;
        Ok(())
    }
}
fn write_loop(
    mut writer: Rotating,
    receiver: mpsc::Receiver<Entry>,
    shared: &Shared,
    session: u128,
) -> std::io::Result<()> {
    let mut heartbeat = Instant::now();
    loop {
        let stopped = shared.stop.load(Ordering::Acquire);
        let item = if stopped {
            receiver.try_recv().ok()
        } else {
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(value) => Some(value),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };
        if let Some(entry) = item {
            writer.write(
                &serde_json::json!({"schema":1,"session":session,"pid":std::process::id(),
                "ms":entry.at_ms,"written_ms":millis(shared.start.elapsed()),"data":entry.event}),
            )?;
        } else if stopped {
            break;
        }
        if heartbeat.elapsed() >= Duration::from_secs(1) {
            heartbeat = Instant::now();
            let now = millis(shared.start.elapsed());
            writer.write(&serde_json::json!({"schema":1,"session":session,"pid":std::process::id(),"ms":now,
                "data":Event::Watchdog { ui_age_ms:now.saturating_sub(shared.ui.load(Ordering::Relaxed)),
                    web_age_ms:now.saturating_sub(shared.web.load(Ordering::Relaxed)),dropped:shared.dropped.load(Ordering::Relaxed) }}))?;
        }
    }
    let finish = shared.finish.load(Ordering::Relaxed);
    writer.write(&serde_json::json!({"schema":1,"session":session,"pid":std::process::id(),"ms":millis(shared.start.elapsed()),
        "data":Event::Finish {run_ok:finish&1!=0,cleanup_ok:finish&2!=0}}))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn runs_are_isolated_and_retention_never_touches_active_or_foreign_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("events-0.jsonl"), b"legacy").unwrap();
        let foreign = root
            .path()
            .join(format!("{RUN_PREFIX}00000000000000000000-foreign"));
        std::fs::create_dir(&foreign).unwrap();
        std::fs::write(foreign.join("keep.txt"), b"foreign").unwrap();
        let mut active = Journal::isolated(root.path(), 4096).unwrap();
        for _ in 0..FINISHED_RUNS + 3 {
            let mut run = Journal::isolated(root.path(), 4096).unwrap();
            run.handle().bridge("recv", "key", true, 1, 2, 3);
            run.finish(true, true).await.unwrap();
        }
        assert!(active.handle().enabled());
        active
            .handle()
            .bridge("recv", "still_active", true, 1, 2, 3);
        let owned = || {
            std::fs::read_dir(root.path())
                .unwrap()
                .flatten()
                .filter(|entry| entry.path().join("owner").is_file())
                .count()
        };
        assert_eq!(owned(), FINISHED_RUNS + 1);
        active.finish(true, true).await.unwrap();
        assert_eq!(owned(), FINISHED_RUNS);
        assert_eq!(
            std::fs::read(root.path().join("events-0.jsonl")).unwrap(),
            b"legacy"
        );
        assert_eq!(std::fs::read(foreign.join("keep.txt")).unwrap(), b"foreign");
    }
    #[tokio::test]
    #[ignore = "owned subprocess used only by the isolation test"]
    async fn isolated_child() {
        let root = PathBuf::from(std::env::var_os("PTTSTELLA_TEST_LOG_ROOT").unwrap());
        let role = std::env::var("PTTSTELLA_TEST_LOG_ROLE").unwrap();
        let mut journal = Journal::isolated(&root, 4096).unwrap();
        journal.handle().bridge("recv", "key", true, 1, 2, 3);
        std::fs::write(root.join(format!("ready-{role}")), b"ready").unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            while !root.join("release").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        journal.finish(true, true).await.unwrap();
    }
    #[test]
    fn independent_processes_write_the_same_root_without_lock_contention() {
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                if self.0.try_wait().ok().flatten().is_none() {
                    let _ = self.0.kill();
                }
                let _ = self.0.wait();
            }
        }
        let root = tempfile::tempdir().unwrap();
        let mut children = Vec::new();
        for role in ["a", "b"] {
            children.push(Child(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "ui::diagnostics::tests::isolated_child",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("PTTSTELLA_TEST_LOG_ROOT", root.path())
                    .env("PTTSTELLA_TEST_LOG_ROLE", role)
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while !["ready-a", "ready-b"]
            .iter()
            .all(|name| root.path().join(name).exists())
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        std::fs::write(root.path().join("release"), b"go").unwrap();
        for child in &mut children {
            loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        let runs: Vec<_> = std::fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().join("owner").is_file())
            .collect();
        assert_eq!(runs.len(), 2);
        let mut pids = std::collections::BTreeSet::new();
        for run in runs {
            let records = std::fs::read_to_string(run.path().join("events-0.jsonl")).unwrap();
            let records: Vec<serde_json::Value> = records
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert!(
                records
                    .iter()
                    .any(|r| r["data"]["event"] == "finish" && r["data"]["cleanup_ok"] == true)
            );
            let pid = records[0]["pid"].as_u64().unwrap();
            assert!(records.iter().all(|r| r["pid"].as_u64() == Some(pid)));
            pids.insert(pid);
        }
        assert_eq!(pids.len(), 2);
    }
    #[test]
    fn rotation_is_bounded_and_only_owns_its_four_log_files() {
        let directory = tempfile::tempdir().unwrap();
        let unrelated = directory.path().join("keep.txt");
        std::fs::write(&unrelated, "keep").unwrap();
        let mut writer = Rotating::new(directory.path().to_path_buf(), 256).unwrap();
        for index in 0..100 {
            writer
                .write(&serde_json::json!({"sequence":index,"value":[1,2,3,4,5,6,7,8]}))
                .unwrap();
        }
        drop(writer);
        let mut last = Vec::new();
        for index in 0..FILE_COUNT {
            let path = directory.path().join(format!("events-{index}.jsonl"));
            assert!(path.metadata().unwrap().len() <= 256);
            for line in std::fs::read_to_string(path).unwrap().lines() {
                let value: serde_json::Value = serde_json::from_str(line).unwrap();
                last.push(value["sequence"].as_u64().unwrap());
            }
        }
        assert!(last.contains(&99));
        assert!(!last.contains(&0));
        assert_eq!(std::fs::read_to_string(unrelated).unwrap(), "keep");
    }
    #[tokio::test]
    async fn writer_is_exclusive_flushes_and_releases_its_owner() {
        let directory = tempfile::tempdir().unwrap();
        let mut journal = Journal::at(directory.path(), 4096).unwrap();
        assert!(Journal::at(directory.path(), 4096).is_err());
        journal.handle().bridge("recv", "key", true, 3, 4, 5);
        journal.finish(true, true).await.unwrap();
        assert!(journal.worker.is_none());
        let text = std::fs::read_to_string(directory.path().join("events-0.jsonl")).unwrap();
        let entries: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(entries.iter().any(|value| value["data"]["kind"] == "key"));
        assert!(
            entries
                .iter()
                .any(|value| value["data"]["event"] == "finish"
                    && value["data"]["cleanup_ok"] == true)
        );
        let mut next = Journal::at(directory.path(), 4096).unwrap();
        next.finish(true, true).await.unwrap();
    }
    #[test]
    fn telemetry_accepts_no_arbitrary_strings_or_sensitive_fields() {
        let trace = serde_json::json!({"event":"key","phase":0,"flags":0,"target":1,"generation":1,
            "revision":2,"presentation":3,"elapsed_ms":0,"scroll":0,"extent":576,"suppressed":0});
        assert!(serde_json::from_value::<WebTrace>(trace.clone()).is_ok());
        for (key, value) in [
            ("key", serde_json::json!("secret")),
            ("text", serde_json::json!("article")),
            ("event", serde_json::json!("password")),
            ("target", serde_json::json!("account")),
        ] {
            let mut invalid = trace.clone();
            invalid[key] = value;
            assert!(serde_json::from_value::<WebTrace>(invalid).is_err());
        }
    }
    #[test]
    fn a_full_channel_drops_telemetry_without_blocking_input() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let shared = Arc::new(Shared {
            start: Instant::now(),
            ui: AtomicU64::new(0),
            web: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            finish: AtomicU8::new(0),
        });
        let handle = Handle {
            inner: Some((sender, shared.clone())),
        };
        for _ in 0..100 {
            handle.bridge("recv", "key", true, 1, 2, 3);
        }
        assert_eq!(shared.dropped.load(Ordering::Relaxed), 99);
        shared.stop.store(true, Ordering::Release);
        handle.bridge("recv", "key", true, 1, 2, 3);
        assert_eq!(shared.dropped.load(Ordering::Relaxed), 99);
    }
    #[tokio::test]
    async fn independent_watchdog_survives_stalled_ui_and_web_and_writer_failures_are_reported() {
        let directory = tempfile::tempdir().unwrap();
        let mut journal = Journal::at(directory.path(), 4096).unwrap();
        tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                if std::fs::read_to_string(directory.path().join("events-0.jsonl"))
                    .unwrap()
                    .contains("watchdog")
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        journal.finish(true, true).await.unwrap();
        let text = std::fs::read_to_string(directory.path().join("events-0.jsonl")).unwrap();
        let entries: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(entries.iter().any(|v| v["data"]["event"] == "watchdog"
            && v["data"]["ui_age_ms"].as_u64().unwrap_or(0) >= 1000));
        // A record cannot fit: failure stays local to logging, and is visible to UI.
        let other = tempfile::tempdir().unwrap();
        let mut failed = Journal::at(other.path(), 1).unwrap();
        assert!(failed.finish(false, false).await.is_err());
        assert!(failed.handle().failed());
    }
}
