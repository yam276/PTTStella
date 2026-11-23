//! One passive reading workload for both real desktop frontends.
use crate::{encoding, screen::Frame};

pub const PATHS: [&str; 2] = ["motion-a.gif", "motion-b.gif"];
const TITLE: &str = "PTTStella dual GIF / owned loopback benchmark";

pub fn origin(value: &str) -> Result<String, String> {
    let parsed = url::Url::parse(value).map_err(|_| "invalid benchmark media origin")?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.port().is_none_or(|p| p == 0)
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || value.contains(['\r', '\n', '\t'])
    {
        return Err("benchmark needs an explicit 127.0.0.1 HTTP origin with a port".into());
    }
    Ok(parsed.as_str().trim_end_matches('/').to_owned())
}

pub fn page(media_origin: &str) -> Result<Vec<u8>, String> {
    let origin = origin(media_origin)?;
    encoding::encode(&format!(
        "\x1b[0m\x1b[2J\x1b[H{TITLE}\x1b[2;1H{origin}/{}\x1b[3;1HFirst animation: alternating red / blue\x1b[4;1H{origin}/{}\x1b[5;1HSecond animation: same bytes, independent URL\x1b[7;1HUse real UI to expand, pause, resume and return.\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 1/1 頁 (100%) 目前顯示: 第 1~23 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[?25l",
        PATHS[0], PATHS[1]
    )).map_err(|e| e.to_string())
}

pub fn verify(frame: &Frame, media_origin: &str) -> Result<(), String> {
    let origin = origin(media_origin)?;
    let row = |index| {
        frame
            .glyphs
            .iter()
            .filter(|g| g.row == index)
            .map(|g| g.text.as_str())
            .collect::<String>()
    };
    if frame.cols != 80
        || frame.rows != 24
        || frame.cursor_visible
        || !frame.control_complete
        || row(0).trim_end() != TITLE
        || row(1).trim_end() != format!("{origin}/{}", PATHS[0])
        || row(3).trim_end() != format!("{origin}/{}", PATHS[1])
        || !row(23).contains("第 1/1 頁 (100%)")
    {
        return Err("dual GIF frame differs from the shared reading workload".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;

    #[test]
    fn only_an_explicit_local_origin_can_enter_the_shared_workload() {
        assert_eq!(
            origin("http://127.0.0.1:9876/").unwrap(),
            "http://127.0.0.1:9876"
        );
        for bad in [
            "http://example.com:9876",
            "http://localhost:9876",
            "https://127.0.0.1:9876",
            "http://127.0.0.1",
            "http://127.0.0.1:0",
            "http://name@127.0.0.1:9876",
            "http://127.0.0.1:9876/path",
            "http://127.0.0.1:9876/?q=1",
            "http://127.0.0.1:9876/#x",
            "http://127.0.0.1:9876/\n",
        ] {
            assert!(page(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn two_independent_urls_and_single_complete_page_survive_the_production_reducer() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&page("http://127.0.0.1:9876").unwrap());
        let frame = screen.frame();
        assert!(verify(&frame, "http://127.0.0.1:9876").is_ok());
        assert!(verify(&frame, "http://127.0.0.1:9877").is_err());
        assert!(
            frame
                .glyphs
                .iter()
                .filter(|g| g.row == 1)
                .map(|g| g.text.as_str())
                .collect::<String>()
                .starts_with("http://127.0.0.1:9876/motion-a.gif")
        );
        assert!(
            frame
                .glyphs
                .iter()
                .filter(|g| g.row == 3)
                .map(|g| g.text.as_str())
                .collect::<String>()
                .starts_with("http://127.0.0.1:9876/motion-b.gif")
        );
        screen.feed(b"\x1b[?25h");
        assert!(verify(&screen.frame(), "http://127.0.0.1:9876").is_err());
        screen.feed(b"\x1b[?25l\x1b[4;1Hwrong second URL\x1b[K");
        assert!(verify(&screen.frame(), "http://127.0.0.1:9876").is_err());
    }
}
