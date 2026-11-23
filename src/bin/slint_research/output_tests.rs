use super::*;
use pttstella::{replay, screen::Screen};
use std::io::Cursor;

fn fixture() -> Frame {
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&replay::parity_grid().unwrap());
    screen.frame()
}

#[test]
fn click_lease_rejects_shift_drag_double_click_geometry_and_source_changes() {
    let frame = fixture();
    let geometry = Geometry {
        size: Size {
            width: 960,
            height: 576,
        },
        left: 70.,
        top: 30.,
        width: 1100.,
        height: 680.,
        dpi: 1.,
    };
    let stamp = Stamp {
        generation: 1,
        revision: frame.revision,
        input_epoch: 0,
    };
    let origin = (550., 180.);
    let mut selected = Selected::default();
    selected
        .pointer(&frame, stamp, geometry, 0, origin, false)
        .unwrap();
    assert!(
        selected
            .pointer(&frame, stamp, geometry, 2, (551., 180.), false)
            .unwrap()
            .is_some()
    );
    for (kind, point, shift, changed_geometry, changed_stamp) in [
        (2, origin, true, geometry, stamp),
        (2, (554., 180.), false, geometry, stamp),
        (3, origin, false, geometry, stamp),
        (
            2,
            origin,
            false,
            Geometry {
                left: 71.,
                ..geometry
            },
            stamp,
        ),
        (
            2,
            origin,
            false,
            geometry,
            Stamp {
                input_epoch: 1,
                ..stamp
            },
        ),
    ] {
        selected
            .pointer(&frame, stamp, geometry, 0, origin, false)
            .unwrap();
        assert!(
            selected
                .pointer(&frame, changed_stamp, changed_geometry, kind, point, shift)
                .unwrap()
                .is_none()
        );
    }
    selected
        .pointer(&frame, stamp, geometry, 0, origin, false)
        .unwrap();
    selected
        .pointer(&frame, stamp, geometry, 1, origin, true)
        .unwrap();
    assert!(
        selected
            .pointer(&frame, stamp, geometry, 2, origin, false)
            .unwrap()
            .is_none()
    );
    // Slint reports double-click before the second release. Cancel consumes it.
    selected
        .pointer(&frame, stamp, geometry, 0, origin, false)
        .unwrap();
    selected
        .pointer(&frame, stamp, geometry, 3, origin, false)
        .unwrap();
    assert!(
        selected
            .pointer(&frame, stamp, geometry, 2, origin, false)
            .unwrap()
            .is_none()
    );
    // Margins support clicks but never manufacture a text selection.
    selected
        .pointer(&frame, stamp, geometry, 0, (1., 180.), false)
        .unwrap();
    assert!(
        selected
            .pointer(&frame, stamp, geometry, 2, (1., 180.), false)
            .unwrap()
            .is_some()
    );
    assert!(selected.text.is_none());
}

#[test]
fn fractional_pixel_hit_and_reverse_drag_expand_the_split_glyph_once() {
    let frame = fixture();
    let size = Size {
        width: 943,
        height: 565,
    };
    assert_eq!(
        size.hit(11.99, 0., &frame),
        Some(Position { col: 0, row: 0 })
    );
    assert_eq!(size.hit(12., 0., &frame), Some(Position { col: 1, row: 0 }));
    assert_eq!(
        size.hit(942.9, 564.9, &frame),
        Some(Position { col: 79, row: 23 })
    );
    assert!(size.hit(943., 0., &frame).is_none());
    assert!(size.hit(f32::NAN, 0., &frame).is_none());
    let stamp = Stamp {
        generation: 1,
        revision: frame.revision,
        input_epoch: 0,
    };
    let geometry = Geometry {
        size,
        left: 0.,
        top: 0.,
        width: size.width as f32,
        height: size.height as f32,
        dpi: 1.,
    };
    let left = size.cell(4, 6, 1, &frame).center();
    let right = size.cell(5, 6, 1, &frame).center();
    let mut selected = Selected::default();
    selected
        .pointer(&frame, stamp, geometry, 0, (right.x, right.y), false)
        .unwrap();
    selected
        .pointer(&frame, stamp, geometry, 1, (left.x, left.y), false)
        .unwrap();
    selected
        .pointer(&frame, stamp, geometry, 2, (left.x, left.y), false)
        .unwrap();
    let (_, text) = selected.text.as_ref().unwrap();
    assert_eq!(text.text, "中");
    assert_eq!(text.rows, 6..7);
    assert_eq!(
        text.spans,
        vec![selection::Span {
            row: 6,
            start: 4,
            end: 6
        }]
    );
    let ansi = selection::ansi(&frame, &text.spans, text.rows.clone());
    let mut replayed = Screen::new(80, 24).unwrap();
    replayed.feed(&pttstella::encoding::encode(&ansi).unwrap());
    let actual = replayed
        .frame()
        .glyphs
        .into_iter()
        .find(|glyph| glyph.col == 0 && glyph.row == 0)
        .unwrap();
    assert_eq!(actual.text, "中");
    assert_ne!(actual.left, actual.right);
    selected
        .pointer(&frame, stamp, geometry, 0, (right.x, right.y), false)
        .unwrap();
    selected
        .pointer(
            &frame,
            Stamp {
                input_epoch: 1,
                ..stamp
            },
            geometry,
            1,
            (left.x, left.y),
            false,
        )
        .unwrap();
    assert!(selected.text.is_none() && selected.gesture.is_none());
}

#[test]
fn selected_png_has_complete_row_width_independent_pixels_and_no_cursor() {
    let frame = fixture();
    let display = replay::parity_display_profile().resolve().unwrap();
    let cropped = render_png(frame.clone(), display.clone(), 6..7).unwrap();
    let mut reader = png::Decoder::new(Cursor::new(&cropped.png))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (960, 24));
    crate::verification::check_dual_backgrounds(&pixels, 4).unwrap();
    let full = render_png(frame.clone(), display.clone(), 0..24).unwrap();
    assert_eq!(
        cropped.image.bytes.as_ref(),
        &full.image.bytes[6 * 24 * 960 * 4..7 * 24 * 960 * 4]
    );
    let mut cursor = frame;
    cursor.cursor = (4, 6);
    cursor.cursor_visible = true;
    assert_eq!(render_png(cursor, display, 6..7).unwrap().png, cropped.png);
}

#[tokio::test]
async fn shared_output_lease_rejects_epoch_revision_disconnect_and_revocation() {
    let connection = Connection::start_ui_probe(replay::parity_grid().unwrap()).unwrap();
    let mut updates = connection.updates.clone();
    connection.connect().unwrap();
    let view = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        updates.wait_for(|view| view.received > 0),
    )
    .await
    .unwrap()
    .unwrap()
    .clone();
    let lease = Lease::new(view.stamp(), true);
    assert!(check_output(&WorkLease::Terminal(lease.clone()), &view, Phase::Connected).is_ok());
    let mut changed = view.clone();
    changed.input_epoch += 1;
    assert!(
        check_output(
            &WorkLease::Terminal(lease.clone()),
            &changed,
            Phase::Connected
        )
        .is_err()
    );
    changed = view.clone();
    changed.frame.revision += 1;
    assert!(
        check_output(
            &WorkLease::Terminal(lease.clone()),
            &changed,
            Phase::Connected
        )
        .is_err()
    );
    changed = view.clone();
    changed.phase = Phase::Disconnected;
    assert!(
        check_output(
            &WorkLease::Terminal(lease.clone()),
            &changed,
            Phase::Connected
        )
        .is_err()
    );
    let offline = Lease::new(changed.stamp(), true);
    assert!(check_output(&WorkLease::Terminal(offline), &changed, Phase::Disconnected).is_ok());
    lease.clone().revoke();
    assert!(check_output(&WorkLease::Terminal(lease.clone()), &view, Phase::Connected).is_err());
    assert_eq!(
        connection.shutdown().await.unwrap(),
        pttstella::connection::CloseOutcome::Acknowledged
    );
}

#[test]
fn history_crop_copies_only_requested_rows_and_keeps_the_dual_glyph_png() {
    let mut frame = fixture();
    let mut row = frame
        .glyphs
        .iter()
        .filter(|g| g.row == 6)
        .cloned()
        .collect::<Vec<_>>();
    for g in &mut row {
        g.row = 660;
    }
    frame.glyphs.extend(row);
    frame.rows = 838;
    let cropped = crop_rows(&frame, 660..661).unwrap();
    assert_eq!(cropped.rows, 24);
    assert!(cropped.glyphs.iter().all(|g| g.row == 0));
    assert!(!cropped.cursor_visible);
    let glyph = cropped.glyphs.iter().find(|g| g.col == 4).unwrap();
    assert_eq!(glyph.text, "中");
    assert_ne!(glyph.left, glyph.right);
    let display = replay::parity_display_profile().resolve().unwrap();
    let png = render_png(cropped, display, 0..1).unwrap();
    let mut reader = png::Decoder::new(Cursor::new(png.png)).read_info().unwrap();
    assert_eq!((reader.info().width, reader.info().height), (960, 24));
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut pixels).unwrap();
    crate::verification::check_dual_backgrounds(&pixels, 4).unwrap();
    assert!(crop_rows(&frame, 660..685).is_err());
    assert!(crop_rows(&frame, 838..839).is_err());
}

#[test]
#[ignore = "explicit native acceptance only: first copy the synthetic dual-colour history row in the actual Slint app"]
fn native_acceptance_clipboard_image_matches_the_synthetic_row() {
    let mut clipboard = arboard::Clipboard::new().unwrap();
    let image = clipboard.get_image().unwrap();
    assert!(
        image.width == 960 && image.height == 24,
        "clipboard is not the explicit native acceptance fixture"
    );
    crate::verification::check_dual_backgrounds(&image.bytes, 6).unwrap();
    // No clipboard contents or hashes are printed on a mismatch.
}
