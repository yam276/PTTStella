use pttstella::{encoding, screen::Screen, telnet::Framing};

#[test]
fn ansi_intensity_is_explicit_resettable_and_independent_for_each_half() {
    let mut screen = Screen::new(20, 2).unwrap();
    screen.feed(b"\x1b[31mA\x1b[1mB\x1b[22mC\x1b[91mD\x1b[0mE");
    let frame = screen.frame();
    let styles: Vec<_> = frame
        .glyphs
        .iter()
        .take(5)
        .map(|g| (g.left.fg, g.left.bright))
        .collect();
    assert_eq!(
        styles,
        [(1, false), (1, true), (1, false), (9, false), (7, false)]
    );
    screen.feed(b"\x1b[1;31m\xa4\x1b[22;32m\xa4");
    let frame = screen.frame();
    let pair = &frame.glyphs[5];
    assert_eq!(pair.text, "中");
    assert!(pair.left.bright);
    assert!(!pair.right.bright);
    assert_eq!((pair.left.fg, pair.right.fg), (1, 2));
}

#[test]
fn double_colour_pair_is_one_glyph_with_independent_backgrounds() {
    let bytes = b"\x1b[31;44m\xa4\x1b[32;45m\xa4";
    let mut expected = Screen::new(8, 2).unwrap();
    expected.feed(bytes);
    let frame = expected.frame();
    let glyph = &frame.glyphs[0];
    assert_eq!(
        (&*glyph.text, glyph.width, glyph.left.fg, glyph.right.fg),
        ("中", 2, 1, 2)
    );
    assert_eq!((glyph.left.bg, glyph.right.bg), (4, 5));
    for i in 0..=bytes.len() {
        let mut actual = Screen::new(8, 2).unwrap();
        actual.feed(&bytes[..i]);
        actual.feed(&bytes[i..]);
        assert_eq!(actual, expected, "split at {i}");
    }
}
#[test]
fn uao_and_checked_input_do_not_silently_lose_characters() {
    assert_eq!(encoding::pair([0x9d, 0xde]).as_deref(), Some("♥"));
    assert_eq!(encoding::encode("abc中♥").unwrap(), b"abc\xa4\xa4\x9d\xde");
    assert!(encoding::encode("abc\u{1f680}").is_err());
}
#[test]
fn ascii_range_trail_is_not_a_second_ascii_glyph() {
    let mut screen = Screen::new(8, 2).unwrap();
    screen.feed(b"\xa4\x40!");
    assert_eq!(screen.frame().glyphs[0].text, "一");
    assert_eq!(screen.frame().glyphs[1].col, 2);
}
#[test]
fn half_overwrite_invalidates_the_old_pair_and_keeps_the_source() {
    let mut screen = Screen::new(8, 2).unwrap();
    screen.feed(b"\xa4\xa4\x1b[1;2H ");
    let glyph = &screen.frame().glyphs[0];
    assert!(glyph.unmapped);
    assert_eq!(glyph.width, 1);
    assert_eq!(screen.cell(0, 0).unwrap().byte, 0xa4);
}
#[test]
fn no_pairing_across_rows_and_wrap_is_delayed_through_sgr() {
    let mut screen = Screen::new(2, 2).unwrap();
    screen.feed(b"A\xa4\x1b[31m\xa4");
    assert!(screen.frame().glyphs[1].unmapped);
    assert_eq!(screen.cursor(), (1, 1));
    assert_eq!(screen.cell(0, 1).unwrap().byte, 0xa4);
}
#[test]
fn scrolling_region_does_not_move_the_header() {
    let mut screen = Screen::new(4, 3).unwrap();
    screen.feed(b"HEAD\x1b[2;3r\x1b[2;1Hone\r\ntwo\r\n");
    assert_eq!(screen.cell(0, 0).unwrap().byte, b'H');
    assert_eq!(screen.cell(0, 1).unwrap().byte, b't');
    assert_eq!(screen.cell(0, 2).unwrap().byte, 0);
}
#[test]
fn insert_delete_and_erase_are_bounded_to_the_current_row() {
    let mut screen = Screen::new(5, 2).unwrap();
    screen.feed(b"ABCDE\x1b[1;2H\x1b[P");
    assert_eq!(screen.cell(1, 0).unwrap().byte, b'C');
    screen.feed(b"\x1b[@X");
    assert_eq!(screen.cell(1, 0).unwrap().byte, b'X');
    assert_eq!(screen.cell(2, 0).unwrap().byte, b'C');
}
#[test]
fn unsupported_and_long_sequences_are_visible_and_bounded() {
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(b"\x1b[999z");
    screen.feed(&[b"\x1b[".as_slice(), &[b'1'; 100], b"mX"].concat());
    assert!(screen.frame().unsupported >= 2);
    assert!(Screen::new(usize::MAX, 2).is_err());
    assert!(Screen::new(0, 2).is_err());
}
#[test]
fn arbitrary_bytes_and_chunk_boundaries_do_not_panic_or_change_state() {
    let bytes: Vec<_> = (0..4096).map(|i| ((i * 73 + 19) % 256) as u8).collect();
    let mut whole = Screen::new(80, 24).unwrap();
    whole.feed(&bytes);
    let mut split = Screen::new(80, 24).unwrap();
    for chunk in bytes.chunks(7) {
        split.feed(chunk);
    }
    assert_eq!(whole, split);
}
#[test]
fn telnet_negotiation_and_split_iac_are_not_displayed() {
    let mut framing = Framing::default();
    assert!(framing.feed(&[255], 80, 24).0.is_empty());
    let (text, reply) = framing.feed(&[253, 31, b'X'], 80, 24);
    assert_eq!(text, b"X");
    assert_eq!(reply, [255, 251, 31, 255, 250, 31, 0, 80, 0, 24, 255, 240]);
    assert!(framing.feed(&[255, 253, 31], 80, 24).1.is_empty());
    assert_eq!(framing.feed(&[255, 255], 80, 24).0, [255]);
}

#[test]
fn oversized_subnegotiation_is_discarded_until_its_terminator() {
    let bytes = [b"\xff\xfa".as_slice(), &[b'X'; 100], b"\xff\xffY\xff\xf0OK"].concat();
    for split in 0..=bytes.len() {
        let mut framing = Framing::default();
        let mut text = framing.feed(&bytes[..split], 80, 24).0;
        text.extend(framing.feed(&bytes[split..], 80, 24).0);
        assert_eq!(text, b"OK", "split at {split}");
    }
}

// Each layout is checked against hand-written facts, not just another feed of
// the same parser. Multiple deterministic partitions complement single splits.
fn assert_control_chunks(bytes: &[u8], expected: impl Fn(&Screen)) {
    for split in 0..=bytes.len() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&bytes[..split]);
        screen.feed(&bytes[split..]);
        expected(&screen);
    }
    for seed in 0..8 {
        let mut screen = Screen::new(80, 24).unwrap();
        let mut offset = 0;
        let mut random = seed + 1usize;
        while offset < bytes.len() {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let size = if seed == 0 { 1 } else { random % 13 + 1 };
            let end = (offset + size).min(bytes.len());
            screen.feed(&bytes[offset..end]);
            offset = end;
        }
        expected(&screen);
    }
}

#[test]
fn unknown_control_families_consume_their_complete_sequence() {
    for bytes in [
        b"\x1b[?2027hX".as_slice(),
        b"\x1b[>1;2cX",
        b"\x1b[1:2mX",
        b"\x1b[1 qX",
        b"\x1b[!pX",
        b"\x1b(0X",
        b"\x1b#8X",
        b"\x1b]52;c;not-a-clipboard-write\x07X",
        b"\x1b]8;;https://example.invalid\x1b\\X",
        b"\x1bPignored\x07still-ignored\x1b\\X",
        b"\x1bXignored\x1b\\X",
        b"\x1b^ignored\x1b\\X",
        b"\x1b_ignored\x1b\\X",
    ] {
        assert_control_chunks(bytes, |screen| {
            assert_eq!(screen.cursor(), (1, 0));
            assert_eq!(screen.cell(0, 0).unwrap().byte, b'X');
            assert_eq!(screen.cell(1, 0).unwrap().byte, 0);
            assert_eq!(screen.cell(0, 0).unwrap().style.fg, 7);
            assert!(screen.frame().control_complete);
        });
    }
}

#[test]
fn csi_overflow_and_invalid_parameter_order_never_leak_the_tail() {
    for size in [64, 65, 200] {
        let bytes = [b"\x1b[".as_slice(), &vec![b'1'; size], b";32mX"].concat();
        assert_control_chunks(&bytes, |screen| {
            assert_eq!(screen.cursor(), (1, 0));
            let cell = screen.cell(0, 0).unwrap();
            assert_eq!((cell.byte, cell.style.fg), (b'X', 7));
            assert_eq!(screen.frame().unsupported, 1);
            assert!(screen.frame().control_complete);
        });
    }
    for bytes in [b"\x1b[1$2;32mX".as_slice(), b"\x1b[1\x80;32mX"] {
        assert_control_chunks(bytes, |screen| {
            assert_eq!(screen.cursor(), (1, 0));
            assert_eq!(screen.cell(0, 0).unwrap().byte, b'X');
            assert_eq!(screen.cell(0, 0).unwrap().style.fg, 7);
            assert_eq!(screen.frame().unsupported, 1);
        });
    }
    // Exactly 64 parameter bytes remain accepted; the 65th is discarded.
    let bytes = [b"\x1b[".as_slice(), &[b'0'; 62], b"31mR"].concat();
    assert_control_chunks(&bytes, |screen| {
        assert_eq!(screen.cell(0, 0).unwrap().style.fg, 1);
        assert_eq!(screen.frame().unsupported, 0);
    });
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(b"\x1b[");
    screen.feed(&[b'1'; 4096]);
    assert!(!screen.frame().control_complete);
    assert_eq!(screen.cursor(), (0, 0));
    screen.feed(b"mOK");
    assert_eq!(screen.cursor(), (2, 0));
    assert_eq!(screen.cell(0, 0).unwrap().byte, b'O');
}

#[test]
fn c0_executes_inside_csi_and_del_does_not_finish_it() {
    assert_control_chunks(b"\x1b[3\n1mR\x1b[3\x7f2mG", |screen| {
        assert_eq!(screen.cursor(), (2, 1));
        assert_eq!(screen.cell(0, 1).unwrap().style.fg, 1);
        assert_eq!(screen.cell(1, 1).unwrap().style.fg, 2);
        assert_eq!(screen.cell(0, 0).unwrap().byte, 0);
        assert_eq!(screen.frame().unsupported, 0);
    });
    assert_control_chunks(b"\x1b\n(\x7f0X", |screen| {
        assert_eq!(screen.cursor(), (1, 1));
        assert_eq!(screen.cell(0, 1).unwrap().byte, b'X');
        assert_eq!(screen.frame().unsupported, 1);
    });
}

#[test]
fn cancel_restart_and_session_reset_recover_incomplete_controls() {
    for prefix in [
        b"\x1b[31".as_slice(),
        b"\x1b[1$3",
        b"\x1b(",
        b"\x1b]ignored",
        b"\x1bPignored",
    ] {
        for cancel in [0x18, 0x1a] {
            let bytes = [prefix, &[cancel], b"X"].concat();
            assert_control_chunks(&bytes, |screen| {
                assert_eq!(screen.cursor(), (1, 0));
                assert_eq!(screen.cell(0, 0).unwrap().byte, b'X');
                assert!(screen.frame().control_complete);
            });
        }
        let bytes = [prefix, b"\x1b[31mR"].concat();
        assert_control_chunks(&bytes, |screen| {
            assert_eq!(screen.cursor(), (1, 0));
            assert_eq!(screen.cell(0, 0).unwrap().style.fg, 1);
        });
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(prefix);
        assert!(!screen.frame().control_complete);
        screen.reset_session();
        screen.feed(b"X");
        assert_eq!(screen.cursor(), (1, 0));
        assert_eq!(screen.cell(0, 0).unwrap().byte, b'X');
        assert_eq!(screen.frame().unsupported, 0);
    }
    assert_control_chunks(b"\x9d\xde", |screen| {
        assert_eq!(screen.frame().glyphs[0].text, "♥");
        assert_eq!(screen.cursor(), (2, 0));
    });
}

#[test]
fn synchronized_output_tracks_the_first_begin_without_stopping_byte_processing() {
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(b"\x1b[?2026hA");
    let begin = screen.synchronized_since().unwrap();
    assert!(!screen.frame().control_complete);
    assert_eq!(screen.cell(0, 0).unwrap().byte, b'A');
    screen.feed(b"\x1b[?2026h\x1b[31mB");
    assert_eq!(screen.synchronized_since(), Some(begin));
    assert_eq!(screen.cell(1, 0).unwrap().style.fg, 1);
    screen.feed(b"\x1b[?2026l");
    assert!(screen.frame().control_complete);
    assert_eq!(screen.frame().unsupported, 0);
    screen.feed(b"\x1b[?2026h");
    assert!(screen.synchronized_since().unwrap() > begin);
    screen.release_synchronized_output();
    assert!(screen.frame().control_complete);
    screen.feed(b"\x1b[?2026h\x1bc");
    assert!(screen.synchronized_since().is_none());
    screen.feed(b"\x1b[?2026h");
    screen.reset_session();
    assert!(screen.frame().control_complete);
    assert_eq!(screen.revision(), 0);
}

#[test]
fn synchronized_modes_and_multiple_batches_are_independent_of_packet_boundaries() {
    assert_control_chunks(
        b"\x1b[?2026l\x1b[?7;2026;25hA\x1b[?2026l\x1b[?2026h\x1b[32mB\x1b[?2026l",
        |screen| {
            assert!(screen.synchronized_since().is_none());
            assert!(screen.frame().control_complete);
            assert!(screen.frame().cursor_visible);
            assert_eq!(screen.cursor(), (2, 0));
            assert_eq!(screen.cell(0, 0).unwrap().byte, b'A');
            assert_eq!(screen.cell(1, 0).unwrap().byte, b'B');
            assert_eq!(screen.cell(1, 0).unwrap().style.fg, 2);
            assert_eq!(screen.frame().unsupported, 0);
        },
    );
}
