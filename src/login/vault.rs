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
            .name("pttstella-keychain".into())
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
impl Vault {
    pub fn new(probe: bool) -> Self {
        Self {
            service: if probe {
                "io.github.yam276.PTTStella.login.synthetic-probe.v1"
            } else {
                "io.github.yam276.PTTStella.login.ws.ptt.cc.v1"
            }
            .into(),
        }
    }
    pub fn perform(&self, action: Action) -> Result<Outcome, String> {
        match action {
            Action::Open => {
                let entry = self.load()?;
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
                    let credentials = self.load()?.ok_or(crate::msg!("vault-password-missing"))?;
                    if credentials.account != account {
                        return Err(crate::msg!("vault-account-mismatch"));
                    }
                    credentials
                } else {
                    Credentials::new(account, password).map_err(|error| error.to_string())?
                };
                if remember {
                    self.save(&credentials)?;
                }
                Ok(Outcome::Ready(credentials))
            }
            Action::Forget => {
                self.forget()?;
                Ok(Outcome::Forgotten)
            }
        }
    }
    #[cfg(target_os = "macos")]
    fn load(&self) -> Result<Option<Credentials>, String> {
        use security_framework::passwords::{PasswordOptions, generic_password};
        match generic_password(PasswordOptions::new_generic_password(
            &self.service,
            "default",
        )) {
            Ok(bytes) => {
                let bytes = Zeroizing::new(bytes);
                if bytes.len() > 128 {
                    return Err(crate::msg!("vault-record-too-large"));
                }
                Credentials::from_stored(bytes)
                    .map(Some)
                    .map_err(|error| error.to_string())
            }
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) => Err(crate::msg!("vault-read-failed", value1 = e.code())),
        }
    }
    #[cfg(target_os = "macos")]
    fn save(&self, credentials: &Credentials) -> Result<(), String> {
        security_framework::passwords::set_generic_password(
            &self.service,
            "default",
            &credentials.stored(),
        )
        .map_err(|e| crate::msg!("vault-save-failed", value1 = e.code()))
    }
    #[cfg(target_os = "macos")]
    fn forget(&self) -> Result<(), String> {
        match security_framework::passwords::delete_generic_password(&self.service, "default") {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(crate::msg!("vault-remove-failed", value1 = e.code())),
        }
    }
    #[cfg(not(target_os = "macos"))]
    fn load(&self) -> Result<Option<Credentials>, String> {
        let _ = &self.service;
        Err(crate::msg!("vault-manual-only"))
    }
    #[cfg(not(target_os = "macos"))]
    fn save(&self, _: &Credentials) -> Result<(), String> {
        Err(crate::msg!("vault-unavailable"))
    }
    #[cfg(not(target_os = "macos"))]
    fn forget(&self) -> Result<(), String> {
        Err(crate::msg!("vault-unavailable"))
    }
}
