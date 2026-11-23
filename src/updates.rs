//! Best-effort stable release discovery. No credentials, installer or retry loop.
use std::{future::Future, pin::Pin, time::Duration};

use reqwest::{Client, redirect::Policy};
use semver::Version;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.github.com/repos/yam276/PTTStella/releases/latest";
const RELEASES: &str = "https://github.com/yam276/PTTStella/releases/tag/";
const MAX_BYTES: usize = 256 * 1024;
const DEADLINE: Duration = Duration::from_secs(8);
type Request = Pin<Box<dyn Future<Output = Result<Option<Release>, Error>> + Send>>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Release {
    pub version: String,
    pub url: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Status {
    pub checking: bool,
    pub available: Option<Release>,
    pub up_to_date: bool,
}
#[derive(Debug, PartialEq, Eq)]
enum Error {
    Client,
    Network,
    Http(u16),
    TooLarge,
    Invalid,
    Timeout,
}
#[derive(Deserialize)]
struct PublishedRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

fn compare(bytes: &[u8], current: &str) -> Result<Option<Release>, Error> {
    let release: PublishedRelease = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    if release.tag_name.len() > 128 {
        return Err(Error::Invalid);
    }
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| Error::Invalid)?;
    let current = Version::parse(current).map_err(|_| Error::Invalid)?;
    if release.draft || release.prerelease || !version.pre.is_empty() {
        return Err(Error::Invalid);
    }
    if version.cmp_precedence(&current).is_gt() {
        Ok(Some(Release {
            version: version.to_string(),
            // Never open a URL provided by a remote JSON response.
            url: format!("{RELEASES}{}", release.tag_name),
        }))
    } else {
        Ok(None)
    }
}

async fn fetch(
    client: Client,
    endpoint: &str,
    current: &str,
    deadline: Duration,
) -> Result<Option<Release>, Error> {
    tokio::time::timeout(deadline, async {
        let mut response = client
            .get(endpoint)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|_| Error::Network)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(Error::Http(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_BYTES as u64)
        {
            return Err(Error::TooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
            if chunk.len() > MAX_BYTES.saturating_sub(bytes.len()) {
                return Err(Error::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        compare(&bytes, current)
    })
    .await
    .map_err(|_| Error::Timeout)?
}

async fn check() -> Result<Option<Release>, Error> {
    let tls = crate::tls::client_config().map_err(|_| Error::Client)?;
    let client = Client::builder()
        .use_preconfigured_tls(tls)
        .user_agent(concat!("PTTStella/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(4))
        .timeout(DEADLINE)
        .redirect(Policy::none())
        .referer(false)
        .build()
        .map_err(|_| Error::Client)?;
    fetch(client, ENDPOINT, env!("CARGO_PKG_VERSION"), DEADLINE).await
}

#[derive(Default)]
enum Source {
    #[default]
    Published,
    Loopback {
        endpoint: String,
        client: Client,
    },
}

/// Polled by the surface select loop, never spawned. Dropping this owner cancels
/// the request on every surface exit; no detached worker can update a new UI.
#[derive(Default)]
pub struct Checker {
    request: Option<Request>,
    status: Status,
    source: Source,
}
impl Checker {
    /// Explicit synthetic checks never fall back to the public endpoint, even
    /// after failure/retry. This is not a configurable product update server.
    pub fn loopback(endpoint: &str) -> Result<Self, String> {
        let url = url::Url::parse(endpoint).map_err(|_| "invalid update fixture URL")?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("update fixture must use an explicit IPv4 loopback port".into());
        }
        let client = Client::builder()
            // The fixture admits HTTP only. Still configure reqwest's pinned
            // per-client provider; never install a process-global provider.
            .use_preconfigured_tls(crate::tls::client_config_with_roots(
                rustls::RootCertStore::empty(),
            )?)
            .no_proxy()
            .redirect(Policy::none())
            .user_agent(concat!("PTTStella/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| format!("update fixture client: {error:?}"))?;
        Ok(Self {
            source: Source::Loopback {
                endpoint: url.into(),
                client,
            },
            ..Self::default()
        })
    }
    pub fn status(&self) -> &Status {
        &self.status
    }
    pub fn start(&mut self) {
        if self.request.is_none() {
            self.request = Some(match &self.source {
                Source::Published => Box::pin(check()),
                Source::Loopback { endpoint, client } => {
                    let endpoint = endpoint.clone();
                    let client = client.clone();
                    Box::pin(async move {
                        fetch(client, &endpoint, env!("CARGO_PKG_VERSION"), DEADLINE).await
                    })
                }
            });
            self.status.checking = true;
            self.status.up_to_date = false;
        }
    }
    pub async fn next(&mut self) -> Status {
        let Some(request) = self.request.as_mut() else {
            return std::future::pending().await;
        };
        let result = request.await;
        self.request = None;
        self.status.checking = false;
        if let Ok(release) = result {
            self.status.up_to_date = release.is_none();
            self.status.available = release;
        }
        // Failure leaves no error message, and never claims a successful check.
        self.status.clone()
    }
}

/// Native adapters share the same Checker on their existing Tokio runtime.
/// One bounded command slot and one latest status; no thread, timer or retry
/// loop is added. The adapter must await shutdown before claiming cleanup.
pub struct Worker {
    commands: Option<tokio::sync::mpsc::Sender<()>>,
    status: tokio::sync::watch::Receiver<Status>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Worker {
    pub fn start(mut checker: Checker, automatic: bool) -> Result<Self, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        if automatic {
            checker.start();
        }
        let (commands, mut requests) = tokio::sync::mpsc::channel(1);
        let (send, status) = tokio::sync::watch::channel(checker.status().clone());
        let task = runtime.spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    command = requests.recv() => {
                        if command.is_none() || requests.is_closed() { break; }
                        checker.start();
                        send.send_replace(checker.status().clone());
                    }
                    status = checker.next() => { send.send_replace(status); }
                }
            }
            // Drop the in-flight response here, before the join resolves.
            drop(checker);
        });
        Ok(Self {
            commands: Some(commands),
            status,
            task: Some(task),
        })
    }
    pub fn updates(&self) -> tokio::sync::watch::Receiver<Status> {
        self.status.clone()
    }
    pub fn request(&self) -> bool {
        !self.status.borrow().checking
            && self
                .commands
                .as_ref()
                .is_some_and(|send| send.try_send(()).is_ok())
    }
    pub async fn shutdown(&mut self) -> Result<(), String> {
        self.cancel();
        if let Some(task) = self.task.take() {
            task.await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// Revoke queued checks immediately; retain the task for the later join.
    pub fn cancel(&mut self) {
        self.commands.take();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(task) = &self.task {
            // Exceptional adapter destruction cancels work; only shutdown's
            // successful join proves cleanup, never this fallback.
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests;
