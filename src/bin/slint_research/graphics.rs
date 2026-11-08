use skia_safe::{Canvas, Paint, Path, Rect};

pub fn is_graphic(ch: char) -> bool {
    matches!(ch, '─' | '│' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼'
        | '\u{2580}'..='\u{2590}' | '\u{2594}'..='\u{259f}' | '◢'..='◥')
}

pub fn draw(canvas: &Canvas, ch: char, cell: Rect, paint: &Paint) {
    let (w, h) = (cell.width(), cell.height());
    let x = |fraction: f32| cell.left + w * fraction;
    let y = |fraction: f32| cell.top + h * fraction;
    let rect = |l, t, r, b| {
        canvas.draw_rect(
            Rect::new(x(l).round(), y(t).round(), x(r).round(), y(b).round()),
            paint,
        );
    };
    match ch {
        '▀' => rect(0., 0., 1., 0.5),
        '▁'..='█' => rect(0., 1. - (u32::from(ch) - 0x2580) as f32 / 8., 1., 1.),
        '▉'..='▏' => rect(0., 0., (0x2590 - u32::from(ch)) as f32 / 8., 1.),
        '▐' => rect(0.5, 0., 1., 1.),
        '▔' => rect(0., 0., 1., 0.125),
        '▕' => rect(0.875, 0., 1., 1.),
        '▖'..='▟' => {
            let mask = [4, 8, 1, 13, 9, 7, 11, 2, 6, 14][ch as usize - 0x2596];
            for q in 0..4 {
                if mask & (1 << q) != 0 {
                    let (l, t) = ((q % 2) as f32 / 2., (q / 2) as f32 / 2.);
                    rect(l, t, l + 0.5, t + 0.5);
                }
            }
        }
        '◢'..='◥' => {
            let corners = [
                [(1., 0.), (1., 1.), (0., 1.)],
                [(0., 0.), (0., 1.), (1., 1.)],
                [(0., 0.), (1., 0.), (0., 1.)],
                [(0., 0.), (1., 0.), (1., 1.)],
            ][ch as usize - '◢' as usize];
            let points = corners.map(|(u, v)| (x(u), y(v)).into());
            let mut antialiased = paint.clone();
            antialiased.set_anti_alias(true);
            canvas.draw_path(&Path::polygon(&points, true, None, None), &antialiased);
        }
        _ => {
            let (mx, my) = (x(0.5).round(), y(0.5).round());
            let line = (h / 24.).round().max(1.);
            let left = mx - (line / 2.).floor();
            let top = my - (line / 2.).floor();
            let right = left + line;
            let bottom = top + line;
            for (connected, arm) in [
                (
                    matches!(ch, '─' | '┐' | '┘' | '┤' | '┬' | '┴' | '┼'),
                    Rect::new(cell.left, top, right, bottom),
                ),
                (
                    matches!(ch, '─' | '┌' | '└' | '├' | '┬' | '┴' | '┼'),
                    Rect::new(left, top, cell.right, bottom),
                ),
                (
                    matches!(ch, '│' | '└' | '┘' | '├' | '┤' | '┴' | '┼'),
                    Rect::new(left, cell.top, right, bottom),
                ),
                (
                    matches!(ch, '│' | '┌' | '┐' | '├' | '┤' | '┬' | '┼'),
                    Rect::new(left, top, right, cell.bottom),
                ),
            ] {
                if connected {
                    canvas.draw_rect(arm, paint);
                }
            }
        }
    }
}
