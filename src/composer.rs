//! Local-document projections. ANSI is interpreted by the production screen core.
use crate::{
    drafts::MAX_BODY,
    screen::{Screen, Style},
};
use serde::{Deserialize, Serialize};
pub mod document;
mod projection;

fn safe_sgr(rest: &str) -> Option<&str> {
    rest.strip_prefix("\x1b[")
        .and_then(|r| {
            r.bytes()
                .take(65)
                .position(|b| b == b'm')
                .map(|n| &rest[..n + 3])
        })
        .filter(|s| {
            crate::input::prepare_editor_source(s, crate::encoding::TextEncoding::Uao, 128).is_ok()
        })
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub from: usize,
    pub to: usize,
}
#[derive(Serialize)]
pub struct Decoration {
    pub from: usize,
    pub to: usize,
    pub hidden: bool,
    pub left: Style,
    pub right: Style,
    pub width: usize,
}
#[derive(Serialize)]
pub struct Change {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}
/// Incomplete controls remain visible and editable. Only complete safe SGR is hidden.
pub fn project(source: &str) -> Result<Vec<Decoration>, String> {
    if source.len() > MAX_BODY || source.split('\n').count() > 4096 {
        return Err(crate::msg!("draft-limit"));
    }
    let mut core = Screen::new(2, 1).map_err(str::to_owned)?;
    let mut result = Vec::new();
    let mut byte = 0;
    let mut offset = 0;
    while byte < source.len() {
        let rest = &source[byte..];
        let sgr = safe_sgr(rest);
        if let Some(s) = sgr {
            result.push(Decoration {
                from: offset,
                to: offset + s.len(),
                hidden: true,
                left: Style::default(),
                right: Style::default(),
                width: 0,
            });
            core.feed(s.as_bytes());
            offset += s.len();
            byte += s.len();
            continue;
        }
        let c = rest.chars().next().ok_or(crate::msg!("draft-invalid"))?;
        let len = c.len_utf16();
        if !c.is_control()
            && let Ok(encoded) = crate::encoding::encode(c.encode_utf8(&mut [0; 4]))
            && let Some((left, right)) = core.project_source_glyph(&encoded)
        {
            if let Some(last) = result.last_mut().filter(|d| {
                !d.hidden
                    && left == right
                    && d.to == offset
                    && d.left == left
                    && d.right == right
                    && d.width == encoded.len()
            }) {
                last.to += len;
            } else {
                result.push(Decoration {
                    from: offset,
                    to: offset + len,
                    hidden: false,
                    left,
                    right,
                    width: encoded.len(),
                });
            }
        }
        if matches!(c, '\n' | '\t') {
            // Real source C0 boundaries cancel a pending byte latch. Incomplete
            // or unsupported escape text remains editable and is never fed.
            core.feed(c.encode_utf8(&mut [0; 4]).as_bytes());
        }
        byte += c.len_utf8();
        offset += len;
    }
    Ok(result)
}
pub fn color(
    source: &str,
    ranges: &[Range],
    color: crate::editor_color::Color,
) -> Result<Vec<Change>, String> {
    let ranges = ranges.iter().map(|r| (r.from, r.to)).collect::<Vec<_>>();
    let changed = crate::editor_color::local_ranges(source, &ranges, color)?;
    let old: Vec<char> = source.chars().collect();
    let new: Vec<char> = changed.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    if prefix == old.len() && prefix == new.len() {
        return Ok(Vec::new());
    }
    Ok(vec![Change {
        from: old[..prefix].iter().map(|c| c.len_utf16()).sum(),
        to: old[..old.len() - suffix]
            .iter()
            .map(|c| c.len_utf16())
            .sum(),
        insert: new[prefix..new.len() - suffix].iter().collect(),
    }])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_preserves_sgr66_byte_halves_and_real_control_boundaries() {
        let source = "\x1b[31m\x1b[66;34m中A\x1b[66;32m\n文";
        let projected = project(source).unwrap();
        let at = |needle: &str| {
            source[..source.find(needle).unwrap()]
                .encode_utf16()
                .count()
        };
        let chinese = projected.iter().find(|d| d.from == at("中")).unwrap();
        assert_eq!(
            (chinese.width, chinese.left.fg, chinese.right.fg),
            (2, 1, 4)
        );
        let ascii = projected.iter().find(|d| d.from == at("A")).unwrap();
        assert_eq!((ascii.left.fg, ascii.right.fg), (4, 4));
        let next_line = projected.iter().find(|d| d.from == at("文")).unwrap();
        assert_eq!((next_line.left.fg, next_line.right.fg), (2, 2));
        let tab = project("\x1b[31;66;34m\t文").unwrap();
        let after_tab = tab.last().unwrap();
        assert_eq!((after_tab.left.fg, after_tab.right.fg), (4, 4));
    }
    #[test]
    fn projection_keeps_utf16_chinese_existing_sgr_and_incomplete_source() {
        let p = project("中\x1b[1;31m紅\x1b[m\n\x1b[").unwrap();
        assert!(p.iter().any(|d| d.from == 1 && d.to == 8 && d.hidden));
        let red = p.iter().find(|d| d.from == 8).unwrap();
        assert_eq!(red.width, 2);
        assert_eq!(red.left.fg, 1);
        assert!(red.left.bright);
        assert_eq!(p.iter().filter(|d| d.hidden).count(), 2);
    }
    #[test]
    fn color_whole_lines_skips_empty_and_restores_existing_colors() {
        let s = "中文\n\n\x1b[32m第三行\x1b[m";
        let c = color(
            s,
            &[Range {
                from: 0,
                to: s.encode_utf16().count(),
            }],
            crate::editor_color::Color {
                foreground: 9,
                background: 0,
            },
        )
        .unwrap();
        assert_eq!(c.len(), 1);
        assert!(c[0].insert.contains("中文\x1b[0;22;24;25;27;37;40m\n\n"));
        assert!(
            color(
                s,
                &[Range { from: 1, to: 4 }, Range { from: 3, to: 5 }],
                crate::editor_color::Color {
                    foreground: 1,
                    background: 0
                }
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "explicit release-mode performance comparison"]
    fn benchmark_rectangular_coloring_and_bounded_projection() {
        use std::time::Instant;
        let source = vec!["abcdefghijklmnopqrst"; 240].join("\n");
        let ranges = (0..240)
            .map(|n| (n * 21 + 3, n * 21 + 9))
            .collect::<Vec<_>>();
        let color = crate::editor_color::Color {
            foreground: 9,
            background: 0,
        };
        let before = Instant::now();
        let mut scalar = source.clone();
        for (a, b) in ranges.iter().rev() {
            scalar = crate::editor_color::local_source(&scalar, *a, *b, color).unwrap();
        }
        let scalar_ms = before.elapsed().as_secs_f64() * 1000.;
        let before = Instant::now();
        let batch = crate::editor_color::local_ranges(&source, &ranges, color).unwrap();
        let batch_ms = before.elapsed().as_secs_f64() * 1000.;
        assert_eq!(scalar, batch);
        let long = vec!["中文中文中文中文\x1b[1;31m紅色\x1b[m"; 2048].join("\n");
        let before = Instant::now();
        let p = project(&long).unwrap();
        let project_ms = before.elapsed().as_secs_f64() * 1000.;
        println!(
            "{{\"rect_rows\":240,\"scalar_ms\":{scalar_ms},\"batch_ms\":{batch_ms},\"projection_bytes\":{},\"projection_ms\":{project_ms},\"decorations\":{}}}",
            long.len(),
            p.len()
        );
    }
}
