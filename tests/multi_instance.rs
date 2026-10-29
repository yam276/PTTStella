#![cfg(feature = "desktop")]
//! Independent processes exercise the production shared-document transactions.
use pttstella::{
    article_target::ArticleTarget, board_navigation::BoardName, bookmarks, filters, recent_boards,
    settings, sites,
};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Worker(Child);
impl Worker {
    fn start(root: &Path, mode: &str, role: &str) -> Self {
        Self(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "process_child", "--ignored", "--nocapture"])
                .env("PTTSTELLA_TEST_SHARED_ROOT", root)
                .env("PTTSTELLA_TEST_MODE", mode)
                .env("PTTSTELLA_TEST_ROLE", role)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
    fn finish(mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline, "child did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn wait(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "missing barrier {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn ready(root: &Path, role: &str) {
    std::fs::write(root.join(format!("ready-{role}")), b"ready").unwrap();
    wait(&root.join("release"));
}
fn recent(name: &str) -> recent_boards::RecentBoards {
    let mut visits = recent_boards::RecentBoards::default();
    visits.visit(BoardName::parse(name).unwrap());
    visits
}
fn target(name: &str) -> sites::Target {
    sites::Target::new(
        format!("telnet://{name}.example:23"),
        pttstella::encoding::TextEncoding::Uao,
    )
    .unwrap()
}

#[test]
#[ignore = "invoked only as an owned subprocess by the tests below"]
fn process_child() {
    let root = PathBuf::from(std::env::var_os("PTTSTELLA_TEST_SHARED_ROOT").unwrap());
    let mode = std::env::var("PTTSTELLA_TEST_MODE").unwrap();
    let role = std::env::var("PTTSTELLA_TEST_ROLE").unwrap();
    if mode == "hold" {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("settings.json.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        std::fs::write(root.join("ready-a"), b"ready").unwrap();
        wait(&root.join("release"));
        return;
    }
    if mode == "history" {
        let store = recent_boards::store::Store::at(root.join("recent.json"));
        assert_eq!(store.load().unwrap().boards()[0].as_str(), "Old");
        ready(&root, &role);
        store.record(&recent("New"), false).unwrap();
        return;
    }
    let preferences = settings::store::Store::at(root.join("settings.json"));
    let base = preferences.load().unwrap();
    if mode == "conflict" {
        ready(&root, &role);
        let mut draft = base.clone();
        draft.display.font_size = if role == "a" { 18.0 } else { 19.0 };
        let result = preferences.save_from(&base, draft);
        if let Err(error) = &result {
            assert_eq!(error, &pttstella::msg!("storage-document-conflict"));
        }
        std::fs::write(
            root.join(format!("result-{role}")),
            if result.is_ok() { "saved" } else { "conflict" },
        )
        .unwrap();
        return;
    }
    let rules = filters::store::Store::at(root.join("filters.json"));
    let rule_base = rules.load().unwrap();
    let marks = bookmarks::store::Store::at(root.join("bookmarks.json"));
    let mark_base = marks.load().unwrap();
    let stations = sites::store::Store::at(root.join("sites.json"));
    let station_base = stations.load().unwrap();
    ready(&root, &role);
    let mut draft = base.clone();
    if role == "a" {
        draft.display.font_size = 18.0;
    } else {
        draft.language = pttstella::i18n::Language::English;
    }
    preferences.save_from(&base, draft).unwrap();
    let mut draft = rule_base
        .edit(
            if role == "a" { "Alpha" } else { "Beta" },
            "synthetic",
            false,
        )
        .unwrap();
    if role == "a" {
        draft = draft.edit("Removed", "", true).unwrap();
    }
    rules.save_from(&rule_base, draft).unwrap();
    let mut draft = mark_base
        .upsert(
            ArticleTarget::new(if role == "a" { "Alpha" } else { "Beta" }, "00000101").unwrap(),
            "synthetic",
            100,
        )
        .unwrap();
    if role == "a" {
        draft = draft
            .remove(&ArticleTarget::new("Removed", "00000101").unwrap())
            .unwrap();
    }
    marks.save_from(&mark_base, draft).unwrap();
    let mut draft = station_base.clone();
    draft.save_favourite(role.clone(), target(&role)).unwrap();
    if role == "a" {
        draft.default = target(&role);
    }
    stations.save_from(&station_base, draft).unwrap();
}

#[test]
fn independent_processes_merge_distinct_edits_and_preserve_deletions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    filters::store::Store::at(root.join("filters.json"))
        .save(
            filters::Rules::default()
                .edit("Removed", "", false)
                .unwrap(),
        )
        .unwrap();
    bookmarks::store::Store::at(root.join("bookmarks.json"))
        .save(
            bookmarks::Bookmarks::default()
                .upsert(ArticleTarget::new("Removed", "00000101").unwrap(), "", 100)
                .unwrap(),
        )
        .unwrap();
    let a = Worker::start(root, "merge", "a");
    let b = Worker::start(root, "merge", "b");
    wait(&root.join("ready-a"));
    wait(&root.join("ready-b"));
    std::fs::write(root.join("release"), b"go").unwrap();
    a.finish();
    b.finish();
    let app = settings::store::Store::at(root.join("settings.json"))
        .load()
        .unwrap();
    assert_eq!(app.display.font_size, 18.0);
    assert_eq!(app.language, pttstella::i18n::Language::English);
    let rules = filters::store::Store::at(root.join("filters.json"))
        .load()
        .unwrap();
    assert_eq!(rules.entries.len(), 2);
    assert!(rules.entries.iter().all(|e| e.id != "Removed"));
    let marks = bookmarks::store::Store::at(root.join("bookmarks.json"))
        .load()
        .unwrap();
    assert_eq!(marks.entries().len(), 2);
    assert!(
        marks
            .entries()
            .iter()
            .all(|e| e.target().board().as_str() != "Removed")
    );
    let stations = sites::store::Store::at(root.join("sites.json"))
        .load()
        .unwrap();
    assert_eq!(stations.default, target("a"));
    assert!(stations.favourites.iter().any(|e| e.target == target("a")));
    assert!(stations.favourites.iter().any(|e| e.target == target("b")));
    // Every resulting backup is complete, valid JSON as well.
    for name in ["settings", "filters", "bookmarks", "sites"] {
        let _: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(format!("{name}.json.bak"))).unwrap())
                .unwrap();
    }
}

#[test]
fn concurrent_same_field_edits_reject_one_instead_of_overwriting() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let a = Worker::start(root, "conflict", "a");
    let b = Worker::start(root, "conflict", "b");
    wait(&root.join("ready-a"));
    wait(&root.join("ready-b"));
    std::fs::write(root.join("release"), b"go").unwrap();
    a.finish();
    b.finish();
    let mut results = [
        std::fs::read_to_string(root.join("result-a")).unwrap(),
        std::fs::read_to_string(root.join("result-b")).unwrap(),
    ];
    results.sort();
    assert_eq!(results, ["conflict", "saved"]);
}

#[test]
fn clear_from_another_process_is_not_undone_by_a_new_visit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = recent_boards::store::Store::at(root.join("recent.json"));
    store.record(&recent("Old"), false).unwrap();
    let child = Worker::start(root, "history", "a");
    wait(&root.join("ready-a"));
    store
        .record(&recent_boards::RecentBoards::default(), true)
        .unwrap();
    std::fs::write(root.join("release"), b"go").unwrap();
    child.finish();
    let history = store.load().unwrap();
    assert_eq!(history.boards().len(), 1);
    assert_eq!(history.boards()[0].as_str(), "New");
}

#[test]
fn busy_transaction_is_bounded_and_a_process_exit_releases_the_kernel_lock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let store = settings::store::Store::at(root.join("settings.json"));
    let base = store.load().unwrap();
    let child = Worker::start(root, "hold", "a");
    wait(&root.join("ready-a"));
    let at = Instant::now();
    assert!(store.save_from(&base, base.clone()).is_err());
    assert!(at.elapsed() < Duration::from_secs(3));
    assert!(!root.join("settings.json").exists());
    drop(child);
    store.save_from(&base, base.clone()).unwrap();
    assert_eq!(store.load().unwrap(), base);
}

#[test]
fn same_id_conflict_and_merged_capacity_failure_preserve_current_and_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("filters.json");
    let a = filters::store::Store::at(path.clone());
    let b = filters::store::Store::at(path.clone());
    a.save(
        filters::Rules::default()
            .edit("Alice", "original", false)
            .unwrap(),
    )
    .unwrap();
    let base_a = a.load().unwrap();
    let base_b = b.load().unwrap();
    a.save_from(&base_a, base_a.edit("alice", "changed", false).unwrap())
        .unwrap();
    let current = std::fs::read(&path).unwrap();
    let backup = std::fs::read(path.with_extension("json.bak")).unwrap();
    assert!(
        b.save_from(&base_b, base_b.edit("ALICE", "", true).unwrap())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), current);
    assert_eq!(
        std::fs::read(path.with_extension("json.bak")).unwrap(),
        backup
    );

    let mut full = a.load().unwrap();
    for index in 0..filters::MAX_ENTRIES - 1 {
        full = full.edit(&format!("User{index}"), "", false).unwrap();
    }
    a.save(full).unwrap();
    let base_a = a.load().unwrap();
    let base_b = b.load().unwrap();
    // Both drafts individually fit; their union would exceed the production cap.
    let removed_a = base_a.edit("User0", "", true).unwrap();
    a.save_from(&base_a, removed_a.edit("ExtraA", "", false).unwrap())
        .unwrap();
    let removed_b = base_b.edit("User0", "", true).unwrap();
    let current = std::fs::read(&path).unwrap();
    let backup = std::fs::read(path.with_extension("json.bak")).unwrap();
    assert!(
        b.save_from(&base_b, removed_b.edit("ExtraB", "", false).unwrap())
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), current);
    assert_eq!(
        std::fs::read(path.with_extension("json.bak")).unwrap(),
        backup
    );
}

#[test]
fn a_newly_corrupted_document_is_never_treated_as_observed_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let store = settings::store::Store::at(path.clone());
    let base = store.load().unwrap();
    std::fs::write(&path, b"new unknown contents").unwrap();
    std::fs::write(path.with_extension("json.bak"), b"preserve backup").unwrap();
    assert!(store.save_from(&base, base.clone()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"new unknown contents");
    assert_eq!(
        std::fs::read(path.with_extension("json.bak")).unwrap(),
        b"preserve backup"
    );
}

#[test]
fn merged_bookmarks_over_capacity_reject_without_damaging_saved_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bookmarks.json");
    let a = bookmarks::store::Store::at(path.clone());
    let b = bookmarks::store::Store::at(path.clone());
    let mut initial = bookmarks::Bookmarks::default();
    for index in 0..bookmarks::LIMIT - 1 {
        initial = initial
            .upsert(
                ArticleTarget::new(&format!("Board{index}"), "00000101").unwrap(),
                "",
                100,
            )
            .unwrap();
    }
    a.save(initial).unwrap();
    let base_a = a.load().unwrap();
    let base_b = b.load().unwrap();
    let draft_a = base_a
        .upsert(ArticleTarget::new("NewA", "00000101").unwrap(), "", 100)
        .unwrap();
    let draft_b = base_b
        .upsert(ArticleTarget::new("NewB", "00000101").unwrap(), "", 100)
        .unwrap();
    assert_eq!(draft_a.entries().len(), bookmarks::LIMIT);
    assert_eq!(draft_b.entries().len(), bookmarks::LIMIT);
    a.save_from(&base_a, draft_a.clone()).unwrap();
    let current = std::fs::read(&path).unwrap();
    let backup = std::fs::read(path.with_extension("json.bak")).unwrap();
    assert!(
        b.save_from(&base_b, draft_b).is_err(),
        "the merged 501st bookmark must be rejected"
    );
    assert_eq!(std::fs::read(&path).unwrap(), current);
    assert_eq!(
        std::fs::read(path.with_extension("json.bak")).unwrap(),
        backup
    );
    assert_eq!(bookmarks::store::Store::at(path).load().unwrap(), draft_a);
}
