use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct MemoryStore {
    bytes: RefCell<Option<Zeroizing<Vec<u8>>>>,
    calls: RefCell<Vec<&'static str>>,
    failure: Option<&'static str>,
}
impl Store for MemoryStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        self.calls.borrow_mut().push("load");
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        Ok(self
            .bytes
            .borrow()
            .as_ref()
            .map(|bytes| Zeroizing::new(bytes.to_vec())))
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        self.calls.borrow_mut().push("save");
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        *self.bytes.borrow_mut() = Some(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
    fn forget(&self) -> Result<(), String> {
        self.calls.borrow_mut().push("forget");
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        self.bytes.borrow_mut().take();
        Ok(())
    }
}
fn submit(account: &str, password: &[u8], remember: bool) -> Action {
    Action::Submit {
        account: account.into(),
        password: Zeroizing::new(password.to_vec()),
        remember,
    }
}
fn ready(result: Result<Outcome, String>, account: &str, password: &[u8]) {
    let Ok(Outcome::Ready(credentials)) = result else {
        panic!("expected ready credentials");
    };
    assert_eq!(credentials.account, account);
    let mut expected = Zeroizing::new(password.to_vec());
    expected.push(b'\r');
    assert_eq!(&*credentials.password_input(), &*expected);
}

#[test]
fn unchecked_password_is_ephemeral_even_if_storage_is_unavailable() {
    let store = MemoryStore {
        failure: Some("unavailable"),
        ..Default::default()
    };
    let vault = Vault::new(true);
    assert!(vault.perform_with(&store, Action::Open).is_err());
    store.calls.borrow_mut().clear();
    ready(
        vault.perform_with(&store, submit("Synthetic", b"only this login", false)),
        "Synthetic",
        b"only this login",
    );
    assert!(store.calls.borrow().is_empty());
    assert!(store.bytes.borrow().is_none());
}

#[test]
fn opt_in_persists_one_binary_record_and_reopening_never_exposes_the_password() {
    let store = MemoryStore::default();
    ready(
        Vault::new(true).perform_with(&store, submit("Synthetic", b" pass with spaces ", true)),
        "Synthetic",
        b" pass with spaces ",
    );
    assert_eq!(*store.calls.borrow(), ["save"]);
    assert_eq!(
        store.bytes.borrow().as_ref().unwrap().as_slice(),
        b"Synthetic\0 pass with spaces "
    );
    let Ok(Outcome::Opened { account, saved }) =
        Vault::new(true).perform_with(&store, Action::Open)
    else {
        panic!("open failed");
    };
    assert_eq!(account, "Synthetic");
    assert!(saved);
    ready(
        Vault::new(true).perform_with(&store, submit("Synthetic", b"", false)),
        "Synthetic",
        b" pass with spaces ",
    );
    assert_eq!(*store.calls.borrow(), ["save", "load", "load"]);
    // Supplying another password without consent neither loads nor replaces it.
    ready(
        Vault::new(true).perform_with(&store, submit("Another", b"ephemeral", false)),
        "Another",
        b"ephemeral",
    );
    assert_eq!(*store.calls.borrow(), ["save", "load", "load"]);
    ready(
        Vault::new(true).perform_with(&store, submit("Another", b"replacement", true)),
        "Another",
        b"replacement",
    );
    ready(
        Vault::new(true).perform_with(&store, submit("Another", b"", false)),
        "Another",
        b"replacement",
    );
}

#[test]
fn missing_or_mismatched_account_never_reuses_or_overwrites_a_password() {
    let store = MemoryStore::default();
    let vault = Vault::new(true);
    assert!(
        vault
            .perform_with(&store, submit("Synthetic", b"", false))
            .is_err()
    );
    ready(
        vault.perform_with(&store, submit("Synthetic", b"original", true)),
        "Synthetic",
        b"original",
    );
    assert!(
        vault
            .perform_with(&store, submit("Another", b"", true))
            .is_err()
    );
    ready(
        vault.perform_with(&store, submit("Synthetic", b"", false)),
        "Synthetic",
        b"original",
    );
    assert_eq!(
        store
            .calls
            .borrow()
            .iter()
            .filter(|c| **c == "save")
            .count(),
        1
    );
}

#[test]
fn invalid_input_and_invalid_stored_records_never_write() {
    let store = MemoryStore::default();
    let vault = Vault::new(true);
    for (account, password) in [
        ("guest", b"bad".as_slice()),
        ("Synthetic", b"bad\n"),
        ("Synthetic", b"\x1b[31m"),
    ] {
        assert!(
            vault
                .perform_with(&store, submit(account, password, true))
                .is_err()
        );
    }
    assert!(store.calls.borrow().is_empty());
    for bytes in [
        vec![b'a'; 129],
        b"missing separator".to_vec(),
        b"\xff\0secret".to_vec(),
        b"Synthetic\0bad\0".to_vec(),
        b"Synthetic\0".to_vec(),
    ] {
        *store.bytes.borrow_mut() = Some(Zeroizing::new(bytes));
        assert!(vault.perform_with(&store, Action::Open).is_err());
        assert!(
            vault
                .perform_with(&store, submit("Synthetic", b"", true))
                .is_err()
        );
    }
    assert!(store.calls.borrow().iter().all(|c| *c == "load"));
}

#[test]
fn failed_save_never_returns_ready_and_failed_delete_preserves_the_entry() {
    let store = MemoryStore {
        bytes: RefCell::new(Some(Zeroizing::new(b"Synthetic\0original".to_vec()))),
        failure: Some("locked"),
        ..Default::default()
    };
    let vault = Vault::new(true);
    assert!(
        vault
            .perform_with(&store, submit("Another", b"new", true))
            .is_err()
    );
    assert!(vault.perform_with(&store, Action::Forget).is_err());
    assert_eq!(
        store.bytes.borrow().as_ref().unwrap().as_slice(),
        b"Synthetic\0original"
    );
}

#[test]
fn forget_removes_only_the_selected_record_and_missing_is_idempotent() {
    let store = MemoryStore::default();
    let vault = Vault::new(true);
    ready(
        vault.perform_with(&store, submit("Synthetic", b"original", true)),
        "Synthetic",
        b"original",
    );
    assert!(matches!(
        vault.perform_with(&store, Action::Forget),
        Ok(Outcome::Forgotten)
    ));
    assert!(matches!(
        vault.perform_with(&store, Action::Forget),
        Ok(Outcome::Forgotten)
    ));
    assert!(
        matches!(vault.perform_with(&store, Action::Open), Ok(Outcome::Opened {saved:false, account}) if account.is_empty())
    );
    assert_ne!(vault.service, Vault::new(false).service);
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
#[test]
fn backend_errors_are_sanitized_before_crossing_the_bridge() {
    for error in [
        keyring::Error::PlatformFailure(std::io::Error::other("secret payload").into()),
        keyring::Error::NoStorageAccess(std::io::Error::other("secret payload").into()),
        keyring::Error::BadEncoding(b"secret payload".to_vec()),
        keyring::Error::Ambiguous(vec![]),
    ] {
        assert!(!error_code(error).contains("secret"));
    }
}

fn synthetic_native_vault() -> Vault {
    Vault {
        service: format!(
            "io.github.yam276.PTTStella.login.synthetic-native.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ),
    }
}

struct Cleanup<'a>(&'a Vault);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = self.0.perform(Action::Forget);
    }
}

// Explicit opt-in native probe. The unique service and fixed synthetic payload
// cannot address production entries; the child proves cross-process persistence.
#[test]
#[ignore = "explicit isolated OS credential-store probe; may require an unlock prompt"]
fn native_roundtrip() {
    let vault = synthetic_native_vault();
    let cleanup = Cleanup(&vault);
    assert!(matches!(
        vault.perform(Action::Open),
        Ok(Outcome::Opened { saved: false, .. })
    ));
    ready(
        vault.perform(submit("Synthetic", b"only native test", false)),
        "Synthetic",
        b"only native test",
    );
    assert!(matches!(
        vault.perform(Action::Open),
        Ok(Outcome::Opened { saved: false, .. })
    ));
    ready(
        vault.perform(submit("Synthetic", b"only native test", true)),
        "Synthetic",
        b"only native test",
    );
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "login::vault::tests::native_read_child",
        ])
        .env("PTTSTELLA_SYNTHETIC_VAULT", &vault.service)
        .output()
        .unwrap();
    assert!(child.status.success(), "native persistence child failed");
    assert!(!String::from_utf8_lossy(&child.stdout).contains("only native test"));
    assert!(vault.perform(submit("Another", b"", false)).is_err());
    assert!(matches!(
        vault.perform(Action::Forget),
        Ok(Outcome::Forgotten)
    ));
    assert!(matches!(
        vault.perform(Action::Forget),
        Ok(Outcome::Forgotten)
    ));
    assert!(matches!(
        vault.perform(Action::Open),
        Ok(Outcome::Opened { saved: false, .. })
    ));
    drop(cleanup);
}

fn native_child_vault() -> Vault {
    let service = std::env::var("PTTSTELLA_SYNTHETIC_VAULT").expect("parent probe service");
    assert!(service.starts_with("io.github.yam276.PTTStella.login.synthetic-native."));
    assert!(service.len() < 160);
    Vault { service }
}

#[test]
#[ignore = "child process of native_roundtrip only"]
fn native_read_child() {
    let vault = native_child_vault();
    let Ok(Outcome::Opened {
        account,
        saved: true,
    }) = vault.perform(Action::Open)
    else {
        panic!("saved record missing");
    };
    assert_eq!(account, "Synthetic");
    ready(
        vault.perform(submit("Synthetic", b"", false)),
        "Synthetic",
        b"only native test",
    );
}

#[cfg(all(target_os = "macos", feature = "desktop-services"))]
#[test]
#[ignore = "explicit macOS Keychain probe with an independently signed test copy"]
fn native_keychain_legacy_and_signing() {
    use security_framework::os::macos::keychain::SecKeychain;

    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let vault = synthetic_native_vault();
    let cleanup = Cleanup(&vault);
    let store = NativeStore {
        service: &vault.service,
    };
    assert!(store.load().unwrap().is_none());
    println!("synthetic_keychain_service={}", vault.service);

    // Independently seed the v0.4.1/Dioxus account-NUL-password format.
    let legacy = b"Synthetic\0only native test";
    store.save(legacy).unwrap();
    let writer = std::env::current_exe().unwrap();
    let launch = |reader: &std::path::Path, test| {
        std::process::Command::new(reader)
            .args(["--ignored", "--exact", test, "--nocapture"])
            .env("PTTSTELLA_SYNTHETIC_VAULT", &vault.service)
            .output()
            .unwrap()
    };
    let test = "login::vault::tests::native_keychain_signed_reader";
    let control = launch(&writer, test);
    assert!(
        control.status.success(),
        "same-identity legacy reader failed"
    );
    assert!(
        String::from_utf8_lossy(&control.stdout)
            .lines()
            .any(|line| line == "keychain_read=readable")
    );

    let directory = tempfile::tempdir().unwrap();
    let reader = directory.path().join("reissued-native-vault-test");
    std::fs::copy(&writer, &reader).unwrap();
    assert!(
        std::process::Command::new("/usr/bin/codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--identifier",
                "io.github.yam276.PTTStella.synthetic-native.reissued",
            ])
            .arg(&reader)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict"])
            .arg(&reader)
            .status()
            .unwrap()
            .success()
    );
    let cross = launch(&reader, test);
    let output = String::from_utf8(cross.stdout).unwrap();
    let diagnostic = output
        .lines()
        .find(|line| line.starts_with("keychain_error="));
    assert!(
        cross.status.success(),
        "reissued-identity reader failed: {}, {diagnostic:?}",
        cross.status
    );
    let result = output
        .lines()
        .find(|line| line.starts_with("keychain_read="))
        .expect("missing reissued-identity result");
    println!("reissued_{result}");
    let signed_bytes = reader.metadata().unwrap().len();
    let staging = directory.path().to_owned();
    directory.close().unwrap();
    assert!(!staging.exists());
    println!(
        "reissued_copy_removed={} regular_bytes={signed_bytes}",
        staging.display()
    );
    assert!(
        store
            .load()
            .unwrap()
            .is_some_and(|bytes| bytes.as_slice() == legacy)
    );

    for malformed in [b"missing separator".as_slice(), &[b'x'; 129]] {
        store.save(malformed).unwrap();
        assert!(vault.perform(Action::Open).is_err());
        assert!(vault.perform(submit("Synthetic", b"", true)).is_err());
        assert!(
            store
                .load()
                .unwrap()
                .is_some_and(|bytes| bytes.as_slice() == malformed)
        );
    }
    assert!(matches!(
        vault.perform(Action::Forget),
        Ok(Outcome::Forgotten)
    ));
    assert!(store.load().unwrap().is_none());
    println!("synthetic_keychain_entry_removed=true");
    drop(cleanup);
}

#[cfg(all(target_os = "macos", feature = "desktop-services"))]
#[test]
#[ignore = "child process of native_keychain_legacy_and_signing only"]
fn native_keychain_signed_reader() {
    use security_framework::os::macos::keychain::SecKeychain;

    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    assert!(!SecKeychain::user_interaction_allowed().unwrap());
    let vault = native_child_vault();
    match vault.perform(Action::Open) {
        Ok(Outcome::Opened {
            saved: true,
            account,
        }) => {
            assert_eq!(account, "Synthetic");
            ready(
                vault.perform(submit("Synthetic", b"", false)),
                "Synthetic",
                b"only native test",
            );
            println!("keychain_read=readable");
        }
        Err(error) => {
            // Only accept the platform's documented authorization refusals.
            let code = [-25308i32, -25315, -25293].into_iter().find(|&code| {
                let message = crate::msg!("vault-read-failed", value1 = code);
                error == message
            });
            let Some(code) = code else {
                println!("keychain_error={error}");
                panic!("unexpected reissued-identity Keychain error");
            };
            assert!(
                matches!(vault.perform(submit("Synthetic", b"", false)), Err(reuse_error) if reuse_error == error)
            );
            ready(
                vault.perform(submit("Synthetic", b"manual fallback", false)),
                "Synthetic",
                b"manual fallback",
            );
            println!("keychain_read=authorization_refused os_status={code} manual_fallback=true");
        }
        _ => panic!("unexpected reissued-identity Keychain response"),
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "run explicitly with a nonexistent DBUS_SESSION_BUS_ADDRESS"]
fn native_service_unavailable() {
    assert_eq!(
        std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap(),
        "unix:path=/nonexistent-pttstella-synthetic-bus"
    );
    let vault = Vault::new(true);
    assert!(vault.perform(Action::Open).is_err());
    assert!(
        vault
            .perform(submit("Synthetic", b"only native test", true))
            .is_err()
    );
    ready(
        vault.perform(submit("Synthetic", b"only native test", false)),
        "Synthetic",
        b"only native test",
    );
}

#[test]
fn candidate_and_candidate_probe_keep_distinct_services_without_changing_legacy_identity() {
    let product = Vault::for_scope(Scope::Product);
    let probe = Vault::for_scope(Scope::SyntheticProbe);
    let candidate = Vault::for_scope(Scope::SlintResearch);
    let candidate_probe = Vault::for_scope(Scope::SlintSyntheticProbe);
    assert_eq!(
        product.service,
        "io.github.yam276.PTTStella.login.ws.ptt.cc.v1"
    );
    assert_eq!(
        probe.service,
        "io.github.yam276.PTTStella.login.synthetic-probe.v1"
    );
    assert_eq!(Vault::new(false).service, product.service);
    assert_eq!(Vault::new(true).service, probe.service);
    let services = [
        product.service,
        probe.service,
        candidate.service,
        candidate_probe.service,
    ];
    for (i, service) in services.iter().enumerate() {
        for other in &services[i + 1..] {
            assert_ne!(service, other);
        }
    }
}
