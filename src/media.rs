//! Media is derived from validated links, never from a second BBS parser.
use serde::Serialize;
use url::Url;
pub mod hover;

#[cfg(feature = "desktop")]
pub mod loader;
#[cfg(feature = "desktop")]
pub mod preview;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Image,
    Video,
    Youtube,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub url: String,
    pub kind: Kind,
    pub image_url: Option<String>,
}

pub fn validate(value: &str) -> Result<Url, String> {
    let url = crate::links::validate(value)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(crate::msg!("media-url-credentials"));
    }
    Ok(url)
}

pub fn classify(value: &str) -> Result<Option<Target>, String> {
    let url = validate(value)?;
    let path = url.path().to_ascii_lowercase();
    let extension = path.rsplit('.').next().unwrap_or_default();
    let kind = match extension {
        "jpg" | "jpeg" | "png" | "gif" | "webp" => Kind::Image,
        "mp4" | "webm" | "ogv" => Kind::Video,
        _ => {
            let host = url.host_str().unwrap_or_default();
            let segments: Vec<_> = url.path().trim_matches('/').split('/').collect();
            let id = match (host, segments.as_slice()) {
                ("youtu.be", [id]) => Some((*id).to_owned()),
                ("youtube.com" | "www.youtube.com" | "m.youtube.com", ["watch"]) => url
                    .query_pairs()
                    .find(|(key, _)| key == "v")
                    .map(|(_, value)| value.into_owned()),
                ("youtube.com" | "www.youtube.com" | "m.youtube.com", ["shorts" | "live", id]) => {
                    Some((*id).to_owned())
                }
                _ => None,
            };
            let Some(id) = id.filter(|id| {
                id.len() == 11
                    && id
                        .bytes()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'_' | b'-'))
            }) else {
                return Ok(None);
            };
            return Ok(Some(Target {
                url: url.into(),
                kind: Kind::Youtube,
                image_url: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
            }));
        }
    };
    Ok(Some(Target {
        image_url: (kind == Kind::Image).then(|| url.to_string()),
        url: url.into(),
        kind,
    }))
}
