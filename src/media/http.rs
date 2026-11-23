//! Shared media HTTP policy; image and native video transports use one owner.
use reqwest::{Client, redirect::Policy};
use std::time::Duration;
use url::Url;

fn redirect_allowed(previous: &[Url], next: &Url) -> bool {
    previous.len() <= 5
        && previous
            .last()
            .is_none_or(|old| old.scheme() != "https" || next.scheme() == "https")
        && super::validate(next.as_str()).is_ok()
}
pub(super) fn client() -> Result<Client, String> {
    let tls = crate::tls::client_config()?;
    Client::builder()
        .use_preconfigured_tls(tls)
        .connect_timeout(Duration::from_secs(8))
        .pool_max_idle_per_host(1)
        .referer(false)
        .user_agent(concat!("PTTStella/", env!("CARGO_PKG_VERSION")))
        .redirect(Policy::custom(|attempt| {
            if redirect_allowed(attempt.previous(), attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("media redirect rejected")
            }
        }))
        .build()
        .map_err(|_| crate::msg!("http-client-failed"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_keep_media_url_and_https_policy() {
        let https = Url::parse("https://example.invalid/video.mp4").unwrap();
        let http = Url::parse("http://example.invalid/video.mp4").unwrap();
        assert!(redirect_allowed(std::slice::from_ref(&http), &https));
        assert!(!redirect_allowed(std::slice::from_ref(&https), &http));
        assert!(redirect_allowed(&vec![https.clone(); 5], &https));
        assert!(!redirect_allowed(&vec![https.clone(); 6], &https));
        for invalid in [
            "https://user:pass@example.invalid/a.mp4",
            "file:///tmp/a.mp4",
            "ftp://example.invalid/a.mp4",
        ] {
            assert!(!redirect_allowed(&[], &Url::parse(invalid).unwrap()));
        }
    }
}
