//! One default account, stored entirely in the OS credential store, never a file.
use super::Credentials;
use zeroize::Zeroizing;

/// Exactly one native operation. OS permission prompts are not abortable;
/// keep and join the thread even when the UI bridge exits with an error.
pub struct Job {
    worker: Option<std::thread::JoinHandle<()>>,
    result: tokio::sync::oneshot::Receiver<Result<Outcome, String>>,
}
impl Job {
    pub fn start(vault: Vault, action: Action) -> Result<Self, String> {
        let (sender, result) = tokio::sync::oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("pttstella-credentials".into())
            .spawn(move || {
                let _ = sender.send(vault.perform(action));
            })
            .map_err(|_| crate::msg!("vault-start-failed").to_owned())?;
        Ok(Self {
            worker: Some(worker),
            result,
        })
    }
    pub async fn wait(&mut self) -> Result<Outcome, String> {
        (&mut self.result)
            .await
            .map_err(|_| crate::msg!("vault-incomplete").to_owned())?
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct Vault {
    service: String,
}
/// Data identities stay explicit while the native candidate is evaluated.
/// Switching the candidate to the product service is a separate migration step.
#[derive(Clone, Copy)]
pub enum Scope {
    Product,
    SyntheticProbe,
    SlintResearch,
    SlintSyntheticProbe,
}
pub enum Action {
    Open,
    Submit {
        account: String,
        password: Zeroizing<Vec<u8>>,
        remember: bool,
    },
    Forget,
}
pub enum Outcome {
    Opened { account: String, saved: bool },
    Ready(Credentials),
    Forgotten,
}

/// The same consent/validation reducer is exercised without touching a user's
/// real vault. Native stores only read/write the single opaque binary record.
trait Store {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String>;
    fn save(&self, bytes: &[u8]) -> Result<(), String>;
    fn forget(&self) -> Result<(), String>;
}
struct NativeStore<'a> {
    service: &'a str,
}

pub const fn supported() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))
}

impl Vault {
    pub fn new(probe: bool) -> Self {
        Self::for_scope(if probe {
            Scope::SyntheticProbe
        } else {
            Scope::Product
        })
    }
    pub fn for_scope(scope: Scope) -> Self {
        Self {
            service: match scope {
                Scope::Product => "io.github.yam276.PTTStella.login.ws.ptt.cc.v1",
                Scope::SyntheticProbe => "io.github.yam276.PTTStella.login.synthetic-probe.v1",
                Scope::SlintResearch => {
                    "io.github.yam276.PTTStella.slint-research.login.ws.ptt.cc.v1"
                }
                Scope::SlintSyntheticProbe => {
                    "io.github.yam276.PTTStella.slint-research.login.synthetic-probe.v1"
                }
            }
            .into(),
        }
    }
    pub fn perform(&self, action: Action) -> Result<Outcome, String> {
        self.perform_with(
            &NativeStore {
                service: &self.service,
            },
            action,
        )
    }
    fn perform_with(&self, store: &impl Store, action: Action) -> Result<Outcome, String> {
        let load = || -> Result<Option<Credentials>, String> {
            store
                .load()?
                .map(|bytes| {
                    if bytes.len() > 128 {
                        return Err(crate::msg!("vault-record-too-large"));
                    }
                    Credentials::from_stored(bytes)
                })
                .transpose()
        };
        match action {
            Action::Open => {
                let entry = load()?;
                Ok(Outcome::Opened {
                    account: entry
                        .as_ref()
                        .map(|e| e.account.clone())
                        .unwrap_or_default(),
                    saved: entry.is_some(),
                })
            }
            Action::Submit {
                account,
                password,
                remember,
            } => {
                let credentials = if password.is_empty() {
                    let credentials = load()?.ok_or(crate::msg!("vault-password-missing"))?;
                    if credentials.account != account {
                        return Err(crate::msg!("vault-account-mismatch"));
                    }
                    credentials
                } else {
                    Credentials::new(account, password).map_err(|error| error.to_string())?
                };
                if remember {
                    store.save(&credentials.stored())?;
                }
                Ok(Outcome::Ready(credentials))
            }
            Action::Forget => {
                store.forget()?;
                Ok(Outcome::Forgotten)
            }
        }
    }
}

impl Store for NativeStore<'_> {
    #[cfg(target_os = "macos")]
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        use security_framework::passwords::{PasswordOptions, generic_password};
        match generic_password(PasswordOptions::new_generic_password(
            self.service,
            "default",
        )) {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) => Err(crate::msg!("vault-read-failed", value1 = e.code())),
        }
    }
    #[cfg(target_os = "macos")]
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        security_framework::passwords::set_generic_password(self.service, "default", bytes)
            .map_err(|e| crate::msg!("vault-save-failed", value1 = e.code()))
    }
    #[cfg(target_os = "macos")]
    fn forget(&self) -> Result<(), String> {
        match security_framework::passwords::delete_generic_password(self.service, "default") {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(crate::msg!("vault-remove-failed", value1 = e.code())),
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        match self.entry().and_then(|entry| entry.get_secret()) {
            Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(crate::msg!("vault-read-failed", value1 = error_code(error))),
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        self.entry()
            .and_then(|entry| entry.set_secret(bytes))
            .map_err(|error| crate::msg!("vault-save-failed", value1 = error_code(error)))
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn forget(&self) -> Result<(), String> {
        match self.entry().and_then(|entry| entry.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(crate::msg!(
                "vault-remove-failed",
                value1 = error_code(error)
            )),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        let _ = &self.service;
        Err(crate::msg!("vault-manual-only"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn save(&self, _: &[u8]) -> Result<(), String> {
        Err(crate::msg!("vault-unavailable"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn forget(&self) -> Result<(), String> {
        Err(crate::msg!("vault-unavailable"))
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl NativeStore<'_> {
    fn entry(&self) -> keyring::Result<keyring::Entry> {
        // Explicit native constructors: never fall back to keyring's in-memory
        // mock or a file store when a feature/backend is unavailable.
        #[cfg(target_os = "windows")]
        let credential =
            keyring::windows::WinCredential::new_with_target(None, self.service, "default")?;
        #[cfg(target_os = "linux")]
        let credential =
            keyring::secret_service::SsCredential::new_with_target(None, self.service, "default")?;
        Ok(keyring::Entry::new_with_credential(Box::new(credential)))
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn error_code(error: keyring::Error) -> &'static str {
    // Backend Display/Debug may contain payloads or identifying attributes.
    // Only fixed categories cross the UI/diagnostic boundary.
    match error {
        keyring::Error::NoStorageAccess(_) => "locked-or-denied",
        keyring::Error::Ambiguous(_) => "ambiguous-entry",
        keyring::Error::BadEncoding(bytes) => {
            drop(Zeroizing::new(bytes));
            "invalid-record"
        }
        keyring::Error::Invalid(..) | keyring::Error::TooLong(..) => "invalid-record",
        _ => "service-unavailable",
    }
}

#[cfg(test)]
mod tests;
