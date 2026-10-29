use super::*;
use crate::{encoding, screen::Screen};
fn scene(name: &str) -> String {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/article-open.json")).unwrap();
    f["screens"][name].as_str().unwrap().to_owned()
}
fn info() -> String {
    format!(
        "\x1b[2J\x1b[19;1H┌{}┐\x1b[20;1H│ 文章代碼(AID): \x1b[1m#00000101\x1b[0m (Stella) [ptt.cc] owned query\x1b[20;77H│\x1b[21;1H│ 文章網址: synthetic\x1b[21;77H│\x1b[22;1H│ 這一篇文章值 0 Ptt幣\x1b[22;77H│\x1b[23;1H└──────────────────────── [再按 Q 切換價格列表]  ─┘\x1b[24;1H\x1b[37;44m\x1b[2K 請按任意鍵繼續 \x1b[0m\x1b[24;80H",
        "─".repeat(37)
    )
}
fn feed(screen: &mut Screen, s: &str) {
    screen.feed(&encoding::encode(s).unwrap());
}
fn tail() -> String {
    scene("reading").replace(
        "1/3 頁 (33%) 目前顯示: 第 1~23",
        "2/2 頁 (100%) 目前顯示: 第 23~45",
    )
}
fn begin() -> (Screen, Refresh, Instant) {
    let mut screen = Screen::new(80, 24).unwrap();
    feed(&mut screen, &scene("reading"));
    let rows = text::rows(&screen.frame())
        .unwrap()
        .iter()
        .take(3)
        .map(|r| r.text.trim_end().to_owned())
        .collect::<Vec<_>>();
    let head = rows.try_into().unwrap();
    feed(&mut screen, &tail());
    let now = Instant::now();
    let (flow, bytes) = Refresh::begin(
        &screen.frame(),
        BoardName::parse("Stella").unwrap(),
        head,
        now,
    )
    .unwrap();
    assert_eq!(bytes, b"Q");
    (screen, flow, now)
}
#[test]
fn refresh_queries_the_current_identity_then_uses_verified_open_and_returns_to_old_tail() {
    let (mut screen, mut flow, now) = begin();
    for (input, bytes) in [
        (info(), b" ".as_slice()),
        (scene("articles"), b"#"),
        (scene("aid_prompt"), b"00000101\r"),
        (scene("articles"), b"Q"),
        (scene("information"), b" "),
        (scene("articles"), b"\x1b[C"),
        (scene("reading"), b":23\r"),
    ] {
        feed(&mut screen, &input);
        assert_eq!(
            flow.observe(&screen.frame(), now).unwrap(),
            Advance::Send(bytes.to_vec())
        );
        assert_eq!(flow.observe(&screen.frame(), now).unwrap(), Advance::Wait);
    }
    feed(&mut screen, &tail());
    assert_eq!(
        flow.observe(&screen.frame(), now).unwrap(),
        Advance::Reached(ArticleTarget::new("Stella", "00000101").unwrap())
    );
    assert!(flow.observe(&screen.frame(), now).is_err());
}
#[test]
fn refresh_refuses_wrong_identity_changed_header_and_unowned_prompts() {
    for changed in ["wrong_aid", "header"] {
        let (mut screen, mut flow, now) = begin();
        for (input, expected) in [
            (info(), b" ".as_slice()),
            (scene("articles"), b"#"),
            (scene("aid_prompt"), b"00000101\r"),
            (scene("articles"), b"Q"),
        ] {
            feed(&mut screen, &input);
            assert_eq!(
                flow.observe(&screen.frame(), now).unwrap(),
                Advance::Send(expected.to_vec())
            );
        }
        if changed == "wrong_aid" {
            feed(&mut screen, &scene("wrong_aid"));
            assert_eq!(flow.observe(&screen.frame(), now), Err(Error::Identity));
        } else {
            for input in [scene("information"), scene("articles")] {
                feed(&mut screen, &input);
                assert!(matches!(
                    flow.observe(&screen.frame(), now),
                    Ok(Advance::Send(_))
                ));
            }
            feed(
                &mut screen,
                &scene("reading").replace("合成帳號", "changed"),
            );
            assert_eq!(flow.observe(&screen.frame(), now), Err(Error::Identity));
        }
    }
    let (mut screen, mut flow, now) = begin();
    feed(&mut screen, &scene("entry"));
    assert_eq!(flow.observe(&screen.frame(), now).unwrap(), Advance::Wait);
    assert_eq!(
        flow.observe(&screen.frame(), now + Duration::from_secs(6)),
        Err(Error::Timeout)
    );
    feed(&mut screen, &info());
    assert!(flow.observe(&screen.frame(), now).is_err());
}
#[test]
fn incomplete_information_cannot_be_dismissed_and_non_tail_sources_cannot_refresh() {
    let (mut screen, mut flow, now) = begin();
    screen.feed(b"\x1b[?2026h");
    feed(&mut screen, &info());
    assert_eq!(flow.observe(&screen.frame(), now).unwrap(), Advance::Wait);
    screen.feed(b"\x1b[?2026l");
    assert_eq!(
        flow.observe(&screen.frame(), now).unwrap(),
        Advance::Send(b" ".to_vec())
    );
    let mut screen = Screen::new(80, 24).unwrap();
    feed(&mut screen, &scene("reading"));
    assert!(
        Refresh::begin(
            &screen.frame(),
            BoardName::parse("Stella").unwrap(),
            std::array::from_fn(|_| String::new()),
            now
        )
        .is_err()
    );
}
