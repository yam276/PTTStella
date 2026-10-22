//! One cancellable future per preview. No spawned HTTP/decoder tasks to abandon.
use futures_util::{StreamExt, future::BoxFuture};
use reqwest::{Client, header, redirect::Policy};
use std::{
    collections::VecDeque,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
const CACHE_BYTES: usize = 24 * 1024 * 1024;
const CACHE_ENTRIES: usize = 8;
const CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Url,
    Network,
    Timeout,
    Http(u16),
    TooLarge,
    Format,
    Dimensions,
    Setup(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url => f.write_str(&crate::msg!("media-url-unsupported")),
            Self::Network => f.write_str(&crate::msg!("image-connect-failed")),
            Self::Timeout => f.write_str(&crate::msg!("image-timeout")),
            Self::Http(status) => {
                f.write_str(&crate::msg!("image-http", status = status.to_string()))
            }
            Self::TooLarge => f.write_str(&crate::msg!("image-too-large")),
            Self::Format => f.write_str(&crate::msg!("image-invalid")),
            Self::Dimensions => f.write_str(&crate::msg!("image-dimensions")),
            Self::Setup(error) => {
                f.write_str(&crate::msg!("image-client-failed", error = error.as_str()))
            }
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub width: usize,
    pub height: usize,
    pub animated_format: bool,
    pub animation_pixels: usize,
}

pub struct Loaded {
    pub key: String,
    pub image: Arc<Image>,
    pub cached: bool,
    expires: Instant,
}
struct Entry {
    key: String,
    image: Arc<Image>,
    expires: Instant,
}
pub struct Loader {
    client: Result<Client, Error>,
    cache: VecDeque<Entry>,
    bytes: usize,
    deadline: Duration,
}
impl Default for Loader {
    fn default() -> Self {
        Self::new()
    }
}
impl Loader {
    pub fn new() -> Self {
        let client = crate::tls::client_config()
            .map_err(Error::Setup)
            .and_then(|tls| {
                Client::builder()
                    .use_preconfigured_tls(tls)
                    .connect_timeout(Duration::from_secs(8))
                    .pool_max_idle_per_host(1)
                    .referer(false)
                    // Image hosts may close requests that omit an application identity.
                    .user_agent(concat!("PTTStella/", env!("CARGO_PKG_VERSION")))
                    .redirect(Policy::custom(|attempt| {
                        let url = attempt.url();
                        let downgrade = attempt.previous().last().is_some_and(|previous| {
                            previous.scheme() == "https" && url.scheme() != "https"
                        });
                        if attempt.previous().len() > 5
                            || downgrade
                            || super::validate(url.as_str()).is_err()
                        {
                            attempt.error("media redirect rejected")
                        } else {
                            attempt.follow()
                        }
                    }))
                    .build()
                    .map_err(|_| Error::Setup(crate::msg!("http-client-failed")))
            });
        Self {
            client,
            cache: VecDeque::new(),
            bytes: 0,
            deadline: Duration::from_secs(20),
        }
    }

    pub fn load(&mut self, value: &str) -> BoxFuture<'static, Result<Loaded, Error>> {
        let mut url = match super::validate(value) {
            Ok(url) => url,
            Err(_) => return Box::pin(async { Err(Error::Url) }),
        };
        url.set_fragment(None);
        let key = url.to_string();
        let now = Instant::now();
        self.prune(now);
        if let Some(index) = self.cache.iter().position(|entry| entry.key == key)
            && let Some(entry) = self.cache.remove(index)
        {
            let result = Loaded {
                key: key.clone(),
                image: entry.image.clone(),
                cached: true,
                expires: entry.expires,
            };
            self.cache.push_back(entry);
            return Box::pin(async { Ok(result) });
        }
        let client = self.client.clone();
        let deadline = self.deadline;
        Box::pin(async move {
            let response = client?
                .get(url)
                .timeout(deadline)
                .header(header::ACCEPT, "image/png,image/jpeg,image/gif,image/webp")
                .send()
                .await
                .map_err(network_error)?;
            if !response.status().is_success() {
                return Err(Error::Http(response.status().as_u16()));
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_BYTES as u64)
            {
                return Err(Error::TooLarge);
            }
            let ttl = cache_ttl(response.headers());
            let mut stream = response.bytes_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(network_error)?;
                if chunk.len() > MAX_BYTES - bytes.len() {
                    return Err(Error::TooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            let image = Arc::new(inspect(bytes)?);
            Ok(Loaded {
                key,
                image,
                cached: false,
                expires: Instant::now() + ttl,
            })
        })
    }

    pub fn retain(&mut self, loaded: &Loaded) {
        if loaded.cached || loaded.expires <= Instant::now() {
            return;
        }
        self.prune(Instant::now());
        if let Some(index) = self.cache.iter().position(|entry| entry.key == loaded.key)
            && let Some(old) = self.cache.remove(index)
        {
            self.bytes -= old.image.bytes.len();
        }
        while self.cache.len() >= CACHE_ENTRIES
            || self.bytes + loaded.image.bytes.len() > CACHE_BYTES
        {
            let Some(entry) = self.cache.pop_front() else {
                break;
            };
            self.bytes -= entry.image.bytes.len();
        }
        self.bytes += loaded.image.bytes.len();
        self.cache.push_back(Entry {
            key: loaded.key.clone(),
            image: loaded.image.clone(),
            expires: loaded.expires,
        });
    }
    fn prune(&mut self, now: Instant) {
        self.cache.retain(|entry| entry.expires > now);
        self.bytes = self.cache.iter().map(|entry| entry.image.bytes.len()).sum();
    }
}

fn network_error(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Timeout
    } else {
        Error::Network
    }
}
fn cache_ttl(headers: &header::HeaderMap) -> Duration {
    let mut ttl = CACHE_TTL;
    for value in headers.get_all(header::CACHE_CONTROL) {
        let Ok(value) = value.to_str() else {
            return Duration::ZERO;
        };
        for directive in value.split(',') {
            let directive = directive.trim().to_ascii_lowercase();
            let (name, value) = directive.split_once('=').unwrap_or((&directive, ""));
            match name.trim() {
                "no-store" | "no-cache" => return Duration::ZERO,
                "max-age" => match value.trim().trim_matches('"').parse::<u64>() {
                    Ok(seconds) => ttl = ttl.min(Duration::from_secs(seconds)),
                    Err(_) => return Duration::ZERO,
                },
                _ => {}
            }
        }
    }
    let age = headers
        .get(header::AGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    ttl.saturating_sub(Duration::from_secs(age.unwrap_or(0)))
}
fn inspect(bytes: Vec<u8>) -> Result<Image, Error> {
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        return Err(Error::Format);
    };
    let dimensions = imagesize::blob_size(&bytes).map_err(|_| Error::Format)?;
    let frames = frame_count(&bytes, mime)?;
    if dimensions.width == 0
        || dimensions.height == 0
        || dimensions.width > 8192
        || dimensions.height > 8192
        || dimensions.width.saturating_mul(dimensions.height) > 16_777_216
        || dimensions
            .width
            .saturating_mul(dimensions.height)
            .saturating_mul(frames)
            > 67_108_864
    {
        return Err(Error::Dimensions);
    }
    Ok(Image {
        bytes,
        mime,
        width: dimensions.width,
        height: dimensions.height,
        animated_format: frames > 1,
        animation_pixels: dimensions
            .width
            .saturating_mul(dimensions.height)
            .saturating_mul(frames),
    })
}

// Count format records without decompressing pixels. Bound browser animation work
// before creating a source; the browser remains responsible for actual decoding.
fn frame_count(bytes: &[u8], mime: &str) -> Result<usize, Error> {
    let mut frames = 0usize;
    let mut declared = 0usize;
    if mime == "image/gif" {
        let packed = *bytes.get(10).ok_or(Error::Format)?;
        let palette = |flags: u8| {
            if flags & 0x80 != 0 {
                3 * (2usize << (flags & 7))
            } else {
                0
            }
        };
        let mut at = 13 + palette(packed);
        loop {
            match bytes.get(at) {
                Some(0x3b) if frames > 0 => return Ok(frames),
                Some(0x21) => at += 2,
                Some(0x2c) => {
                    frames += 1;
                    if frames > 256 {
                        return Err(Error::Dimensions);
                    }
                    let packed = *bytes.get(at + 9).ok_or(Error::Format)?;
                    at += 10 + palette(packed) + 1; // descriptor, local palette, LZW minimum size
                }
                _ => return Err(Error::Format),
            }
            loop {
                let length = usize::from(*bytes.get(at).ok_or(Error::Format)?);
                at += 1;
                if length == 0 {
                    break;
                }
                if length > bytes.len().saturating_sub(at) {
                    return Err(Error::Format);
                }
                at += length;
            }
        }
    }
    if mime == "image/webp" || mime == "image/png" {
        let mut at = if mime == "image/webp" {
            12usize
        } else {
            8usize
        };
        while at < bytes.len() {
            let header = bytes.get(at..at + 8).ok_or(Error::Format)?;
            let count = if mime == "image/webp" {
                let size = u32::from_le_bytes(header[4..8].try_into().map_err(|_| Error::Format)?)
                    as usize;
                if &header[..4] == b"ANMF" {
                    frames += 1;
                }
                size.checked_add(size % 2)
                    .and_then(|size| size.checked_add(8))
            } else {
                let size =
                    u32::from_be_bytes(header[..4].try_into().map_err(|_| Error::Format)?) as usize;
                if &header[4..8] == b"acTL" {
                    let count = bytes.get(at + 8..at + 12).ok_or(Error::Format)?;
                    declared =
                        u32::from_be_bytes(count.try_into().map_err(|_| Error::Format)?) as usize;
                    if declared == 0 {
                        return Err(Error::Format);
                    }
                }
                if &header[4..8] == b"fcTL" {
                    frames += 1;
                }
                size.checked_add(12)
            }
            .ok_or(Error::Format)?;
            if count > bytes.len() - at {
                return Err(Error::Format);
            }
            at += count;
            if frames.max(declared) > 256 {
                return Err(Error::Dimensions);
            }
            if mime == "image/png" && &header[4..8] == b"IEND" {
                break;
            }
        }
    }
    Ok(frames.max(declared).max(1))
}

#[cfg(test)]
mod tests;
