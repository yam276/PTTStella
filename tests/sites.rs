use pttstella::{
    encoding::TextEncoding,
    input,
    screen::Screen,
    sites::{Endpoint, Sites, Target},
};

#[test]
fn endpoints_are_canonical_bounded_and_never_accept_credentials_or_web_pages() {
    for (source, expected) in [
        ("https://term.ptt2.cc/", "wss://ws.ptt2.cc/bbs"),
        ("https://term.gamer.com.tw/", "wss://term.gamer.com.tw/bbs"),
        ("example.com:2323", "telnet://example.com:2323"),
        ("telnet://example.com:23/", "telnet://example.com"),
        ("ws://127.0.0.1:1234/bbs", "ws://127.0.0.1:1234/bbs"),
    ] {
        assert_eq!(
            Endpoint::try_from(source.to_owned()).unwrap().as_str(),
            expected
        );
    }
    for invalid in [
        "",
        "https://example.com/",
        "ssh://example.com",
        "wss://user:secret@example.com/bbs",
        "telnet://example.com/command",
        "wss://example.com/?password=secret",
        "wss://example.com/#secret",
        "telnet://example.com:0",
        "wss://example.com/\n",
    ] {
        assert!(Endpoint::try_from(invalid.to_owned()).is_err(), "{invalid}");
    }
    assert!(Endpoint::try_from(format!("wss://example.com/{}", "x".repeat(513))).is_err());
    assert_eq!(
        Endpoint::try_from("wss://ws.ptt2.cc/bbs".to_owned())
            .unwrap()
            .origin()
            .unwrap(),
        "https://term.ptt2.cc"
    );
    assert_eq!(
        Endpoint::try_from("wss://example.com:1234/bbs".to_owned())
            .unwrap()
            .origin()
            .unwrap(),
        "https://example.com:1234"
    );
    assert!(Target::default().ptt_features());
    for target in [
        Target::new("wss://ws.ptt.cc.evil.example/bbs".into(), TextEncoding::Uao).unwrap(),
        Target::new("wss://ws.ptt.cc/bbs".into(), TextEncoding::Gbk).unwrap(),
        Target::new("wss://ws.ptt2.cc/bbs".into(), TextEncoding::Uao).unwrap(),
    ] {
        assert!(!target.ptt_features());
    }
}

#[test]
fn favourites_update_by_target_and_removing_one_does_not_change_default_or_other_encoding() {
    let mut sites = Sites::default();
    let gbk = Target::new("telnet://example.com".into(), TextEncoding::Gbk).unwrap();
    let big5 = Target::new("telnet://example.com".into(), TextEncoding::Big5).unwrap();
    sites.save_favourite(" GBK ".into(), gbk.clone()).unwrap();
    sites.save_favourite("Big5".into(), big5.clone()).unwrap();
    sites.save_favourite("Rename".into(), gbk.clone()).unwrap();
    assert_eq!(sites.favourites.len(), 6);
    sites.default = gbk.clone();
    sites.remove(&gbk);
    assert_eq!(sites.default, gbk);
    assert!(sites.favourites.iter().any(|item| item.target == big5));
    assert_eq!(Sites::decode(&sites.encode().unwrap()).unwrap(), sites);
    let previous = sites.clone();
    assert!(sites.save_favourite("\n".into(), gbk).is_err());
    assert_eq!(sites, previous);
    let bytes = sites.encode().unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["default"]["encoding"] = serde_json::json!("utf8");
    assert!(Sites::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(Sites::decode(&vec![b' '; pttstella::sites::MAX_BYTES + 1]).is_err());
}

#[test]
fn selected_encoding_owns_glyphs_styles_input_and_paste_across_every_split() {
    // Independently specified bytes: Big5 A4A4 A4E5; GBK D6D0 CEC4 = 中文.
    for (encoding, first, second) in [
        (TextEncoding::Uao, [0xa4, 0xa4], [0xa4, 0xe5]),
        (TextEncoding::Big5, [0xa4, 0xa4], [0xa4, 0xe5]),
        (TextEncoding::Gbk, [0xd6, 0xd0], [0xce, 0xc4]),
    ] {
        let mut bytes = b"\x1b[31m".to_vec();
        bytes.push(first[0]);
        bytes.extend(b"\x1b[32m");
        bytes.push(first[1]);
        bytes.extend(second);
        let mut baseline = Screen::with_encoding(8, 2, encoding).unwrap();
        baseline.feed(&bytes);
        let glyph = &baseline.frame().glyphs[0];
        assert_eq!(
            (&*glyph.text, glyph.width, glyph.left.fg, glyph.right.fg),
            ("中", 2, 1, 2)
        );
        assert_eq!(baseline.frame().glyphs[1].text, "文");
        assert_eq!(baseline.cursor(), (4, 0));
        for split in 0..=bytes.len() {
            let mut screen = Screen::with_encoding(8, 2, encoding).unwrap();
            screen.feed(&bytes[..split]);
            screen.feed(&bytes[split..]);
            assert_eq!(screen.frame(), baseline.frame());
        }
        assert_eq!(
            input::encode_with("中文", encoding).unwrap(),
            [first, second].concat()
        );
        assert_eq!(
            input::prepare_paste_with("中\r\n文", encoding)
                .unwrap()
                .bytes,
            [first.as_slice(), b"\r", second.as_slice()].concat()
        );
        assert!(input::encode_with("中🦀文", encoding).is_err());
        baseline.set_encoding(encoding);
        assert_eq!(baseline.cursor(), (0, 0));
        assert!(
            baseline
                .frame()
                .glyphs
                .iter()
                .all(|glyph| glyph.text == " ")
        );
    }
    let mut gbk = Screen::with_encoding(4, 1, TextEncoding::Gbk).unwrap();
    gbk.feed(&[0x80, 0x81]);
    assert_eq!(gbk.frame().glyphs[0].text, "€");
    assert!(gbk.frame().glyphs[1].unmapped);
}
