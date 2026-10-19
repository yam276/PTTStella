//! Validated station targets and bounded local favourites. No credentials or session data.
use crate::encoding::TextEncoding;
use serde::{Deserialize, Serialize};
use url::Url;

#[cfg(feature = "desktop")]
pub mod store;
pub const PTT: &str = "wss://ws.ptt.cc/bbs";
pub const MAX_SITES: usize = 32;
pub const MAX_BYTES: usize = 65536;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Endpoint(String);
impl Endpoint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn origin(&self) -> Result<String, String> {
        let url = Url::parse(&self.0).map_err(|_| crate::msg!("site-address-invalid"))?;
        Ok(match (url.scheme(), url.host_str(), url.path()) {
            ("wss", Some("ws.ptt.cc"), "/bbs") => "https://term.ptt.cc".into(),
            ("wss", Some("ws.ptt2.cc"), "/bbs") => "https://term.ptt2.cc".into(),
            _ => {
                let mut origin = url;
                let scheme = if origin.scheme() == "wss" {
                    "https"
                } else {
                    "http"
                };
                origin
                    .set_scheme(scheme)
                    .map_err(|_| crate::msg!("site-address-invalid"))?;
                origin.origin().ascii_serialization()
            }
        })
    }
}
impl TryFrom<String> for Endpoint {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        if value.chars().any(char::is_control) {
            return Err(crate::msg!("site-address-invalid"));
        }
        let value = value.trim();
        if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            return Err(crate::msg!("site-address-invalid"));
        }
        let candidate = if value.contains("://") {
            value.to_owned()
        } else {
            format!("telnet://{value}")
        };
        let mut url = Url::parse(&candidate).map_err(|_| crate::msg!("site-address-invalid"))?;
        // Only these official web entry points have an unambiguous BBS endpoint.
        let alias = match (url.scheme(), url.host_str(), url.path()) {
            ("https", Some("term.ptt.cc"), "/") => Some(PTT),
            ("https", Some("term.ptt2.cc"), "/") => Some("wss://ws.ptt2.cc/bbs"),
            ("https", Some("term.gamer.com.tw"), "/") => Some("wss://term.gamer.com.tw/bbs"),
            _ => None,
        };
        if let Some(alias) = alias {
            if !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || url.port().is_some()
            {
                return Err(crate::msg!("site-address-invalid"));
            }
            url = Url::parse(alias).map_err(|_| crate::msg!("site-address-invalid"))?;
        }
        if !matches!(url.scheme(), "wss" | "ws" | "telnet")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query().is_some()
            || url.port() == Some(0)
            || url.as_str().len() > 512
            || (url.scheme() == "telnet" && !matches!(url.path(), "" | "/"))
        {
            return Err(crate::msg!("site-address-invalid"));
        }
        if url.scheme() == "telnet" {
            url.set_path("");
            if url.port() == Some(23) {
                url.set_port(None)
                    .map_err(|_| crate::msg!("site-address-invalid"))?;
            }
        }
        Ok(Self(url.into()))
    }
}
impl From<Endpoint> for String {
    fn from(value: Endpoint) -> String {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub endpoint: Endpoint,
    pub encoding: TextEncoding,
}
impl Default for Target {
    fn default() -> Self {
        Self {
            endpoint: Endpoint(PTT.into()),
            encoding: TextEncoding::Uao,
        }
    }
}
impl Target {
    pub fn new(address: String, encoding: TextEncoding) -> Result<Self, String> {
        Ok(Self {
            endpoint: Endpoint::try_from(address)?,
            encoding,
        })
    }
    pub fn ptt_features(&self) -> bool {
        self.endpoint.as_str() == PTT && self.encoding == TextEncoding::Uao
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Favourite {
    pub name: String,
    pub target: Target,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sites {
    pub version: u8,
    pub default: Target,
    pub favourites: Vec<Favourite>,
}
impl Default for Sites {
    fn default() -> Self {
        Self {
            version: 1,
            default: Target::default(),
            favourites: vec![
                Favourite {
                    name: "PTT".into(),
                    target: Target::default(),
                },
                Favourite {
                    name: "PTT2".into(),
                    target: Target {
                        endpoint: Endpoint("wss://ws.ptt2.cc/bbs".into()),
                        encoding: TextEncoding::Uao,
                    },
                },
                Favourite {
                    name: "Bahamut BBS".into(),
                    target: Target {
                        endpoint: Endpoint("wss://term.gamer.com.tw/bbs".into()),
                        encoding: TextEncoding::Uao,
                    },
                },
                Favourite {
                    name: "SMTH".into(),
                    target: Target {
                        endpoint: Endpoint("telnet://bbs.newsmth.net".into()),
                        encoding: TextEncoding::Gbk,
                    },
                },
            ],
        }
    }
}
impl Sites {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.favourites.len() > MAX_SITES {
            return Err(crate::msg!("sites-invalid"));
        }
        for (index, item) in self.favourites.iter().enumerate() {
            if item.name.trim() != item.name
                || item.name.is_empty()
                || item.name.len() > 128
                || item.name.chars().any(char::is_control)
                || self.favourites[..index]
                    .iter()
                    .any(|previous| previous.target == item.target)
            {
                return Err(crate::msg!("sites-invalid"));
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("sites-invalid"));
        }
        let sites: Self =
            serde_json::from_slice(bytes).map_err(|_| crate::msg!("sites-invalid"))?;
        sites.validate()?;
        Ok(sites)
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| crate::msg!("sites-invalid"))?;
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("sites-invalid"));
        }
        Ok(bytes)
    }
    pub fn save_favourite(&mut self, name: String, target: Target) -> Result<(), String> {
        let mut next = self.clone();
        let item = Favourite {
            name: name.trim().into(),
            target: target.clone(),
        };
        if let Some(existing) = next
            .favourites
            .iter_mut()
            .find(|item| item.target == target)
        {
            *existing = item;
        } else {
            next.favourites.push(item);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn remove(&mut self, target: &Target) {
        self.favourites.retain(|item| &item.target != target);
    }
}
