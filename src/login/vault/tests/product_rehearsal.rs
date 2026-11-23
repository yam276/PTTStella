//! Exercise the product service label in an explicitly selected test keychain.
//! The UI and the production default keychain are outside this probe.
use super::*;
use security_framework::os::macos::keychain::{CreateOptions, SecKeychain};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const BUNDLE_ID: &str = "io.github.yam276.pttstella";
const MARKER: &[u8] = b"PTTStella isolated product credential rehearsal";
const DB: &str = "isolated.keychain-db";
const DB_PASSWORD: &str = "local test keychain only";
const CHILD: &str = "login::vault::tests::product_rehearsal::signed_child";

struct IsolatedStore<'a> {
    chain: &'a SecKeychain,
    service: &'a str,
}

impl Store for IsolatedStore<'_> {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        match self.chain.find_generic_password(self.service, "default") {
            Ok((password, _)) => Ok(Some(Zeroizing::new(password.to_vec()))),
            Err(error) if error.code() == -25300 => Ok(None),
            Err(error) => Err(crate::msg!("vault-read-failed", value1 = error.code())),
        }
    }

    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        self.chain
            .set_generic_password(self.service, "default", bytes)
            .map_err(|error| crate::msg!("vault-save-failed", value1 = error.code()))
    }

    fn forget(&self) -> Result<(), String> {
        match self.chain.find_generic_password(self.service, "default") {
            Ok((_, item)) => {
                item.delete();
                Ok(())
            }
            Err(error) if error.code() == -25300 => Ok(()),
            Err(error) => Err(crate::msg!("vault-remove-failed", value1 = error.code())),
        }
    }
}

struct OwnedKeychain(PathBuf);

impl Drop for OwnedKeychain {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = Command::new("/usr/bin/security")
                .arg("delete-keychain")
                .arg(&self.0)
                .output();
        }
    }
}

fn signed_fixture(root: &Path, version: u8) -> (PathBuf, String) {
    let app = root.join(format!("Rehearsal {version}.app"));
    let contents = app.join("Contents");
    let binary = contents.join("MacOS/fixture");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
    fs::write(
        contents.join("Info.plist"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>\
             <key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>\
             <key>CFBundleExecutable</key><string>fixture</string>\
             <key>CFBundlePackageType</key><string>APPL</string>\
             <key>CFBundleVersion</key><string>{version}</string></dict></plist>"
        ),
    )
    .unwrap();
    for arguments in [
        ["--force", "--sign", "-"],
        ["--verify", "--deep", "--strict"],
    ] {
        let result = Command::new("/usr/bin/codesign")
            .args(arguments)
            .arg(&app)
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}", result.stderr);
    }
    let details = Command::new("/usr/bin/codesign")
        .args(["-d", "-vvvv"])
        .arg(&app)
        .output()
        .unwrap();
    assert!(details.status.success());
    let details = String::from_utf8(details.stderr).unwrap();
    assert!(
        details
            .lines()
            .any(|line| line.strip_prefix("Identifier=") == Some(BUNDLE_ID))
    );
    let hash = details
        .lines()
        .find_map(|s| s.strip_prefix("CDHash="))
        .unwrap();
    (binary, hash.to_owned())
}

#[test]
#[ignore = "explicit macOS product-label test using a separate disposable keychain"]
fn native_product_service_rehearsal() {
    let keychain_list = || {
        let output = Command::new("/usr/bin/security")
            .args(["list-keychains", "-d", "user"])
            .output()
            .unwrap();
        assert!(output.status.success());
        output.stdout
    };
    let search_list = keychain_list();
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let root = tempfile::Builder::new()
        .prefix("stella-product-keychain-")
        .tempdir()
        .unwrap();
    let root_path = root.path().canonicalize().unwrap();
    fs::write(root_path.join("synthetic-marker"), MARKER).unwrap();
    let keychain = OwnedKeychain(root_path.join(DB));
    let chain = CreateOptions::new()
        .password(DB_PASSWORD)
        .create(&keychain.0)
        .unwrap();
    assert!(keychain.0.is_file());
    drop(chain);
    let (original, original_hash) = signed_fixture(&root_path, 1);
    let (candidate, candidate_hash) = signed_fixture(&root_path, 2);
    assert_ne!(original_hash, candidate_hash);
    for (binary, stage, expected) in [
        (&original, "seed", "product_seeded=true"),
        (&candidate, "upgrade", "product_upgrade_refused=true"),
        (&original, "rollback", "product_rollback_and_forget=true"),
    ] {
        let result = Command::new(binary)
            .args(["--ignored", "--exact", CHILD, "--nocapture"])
            .env("PTTSTELLA_TEST_PRODUCT_KEYCHAIN", &keychain.0)
            .env("PTTSTELLA_TEST_PRODUCT_STAGE", stage)
            .output()
            .unwrap();
        let output = String::from_utf8(result.stdout).unwrap();
        assert!(
            result.status.success() && output.lines().any(|line| line == expected),
            "{stage}: {output}\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        println!("{expected}");
    }
    println!(
        "same_bundle_id={BUNDLE_ID} original_cdhash={original_hash} candidate_cdhash={candidate_hash}"
    );
    drop(keychain);
    root.close().unwrap();
    assert!(!root_path.exists());
    assert_eq!(search_list, keychain_list());
    println!("isolated_keychain_removed=true user_search_list_unchanged=true");
}

#[test]
#[ignore = "child process of native_product_service_rehearsal only"]
fn signed_child() {
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let path = PathBuf::from(std::env::var_os("PTTSTELLA_TEST_PRODUCT_KEYCHAIN").unwrap());
    let root = path.parent().unwrap();
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("stella-product-keychain-")
    );
    assert!(path.file_name().unwrap() == DB && path.canonicalize().unwrap() == path);
    assert_eq!(fs::read(root.join("synthetic-marker")).unwrap(), MARKER);
    let mut chain = SecKeychain::open(path).unwrap();
    chain.unlock(Some(DB_PASSWORD)).unwrap();
    let product = Vault::for_scope(Scope::Product);
    let store = IsolatedStore {
        chain: &chain,
        service: &product.service,
    };
    match std::env::var("PTTSTELLA_TEST_PRODUCT_STAGE")
        .unwrap()
        .as_str()
    {
        "seed" => {
            assert!(store.load().unwrap().is_none());
            store.save(b"Synthetic\0legacy fixture").unwrap();
            println!("product_seeded=true");
        }
        "upgrade" => {
            let research = Vault::for_scope(Scope::SlintResearch);
            let isolated = IsolatedStore {
                chain: &chain,
                service: &research.service,
            };
            assert!(matches!(
                research.perform_with(&isolated, Action::Open),
                Ok(Outcome::Opened { saved: false, .. })
            ));
            let error = product
                .perform_with(&store, Action::Open)
                .err()
                .expect("expected independent signature refusal");
            assert!(
                [-25308i32, -25315, -25293]
                    .into_iter()
                    .any(|code| error == crate::msg!("vault-read-failed", value1 = code)),
                "{error}"
            );
            assert!(
                product
                    .perform_with(&store, submit("Synthetic", b"", false))
                    .is_err()
            );
            ready(
                product.perform_with(&store, submit("Synthetic", b"manual fixture", false)),
                "Synthetic",
                b"manual fixture",
            );
            println!("product_upgrade_refused=true");
        }
        "rollback" => {
            ready(
                product.perform_with(&store, submit("Synthetic", b"", false)),
                "Synthetic",
                b"legacy fixture",
            );
            assert!(matches!(
                product.perform_with(&store, Action::Forget),
                Ok(Outcome::Forgotten)
            ));
            assert!(store.load().unwrap().is_none());
            println!("product_rollback_and_forget=true");
        }
        _ => panic!("unknown synthetic product step"),
    }
}
