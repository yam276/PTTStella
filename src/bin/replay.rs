use pttstella::screen::Screen;
use serde::Deserialize;
use std::{error::Error, fs::File, io::Read};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    cols: usize,
    rows: usize,
    chunks: Vec<Vec<u8>>,
    cursor: (usize, usize),
    checks: Vec<Check>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    col: usize,
    row: usize,
    text: String,
    width: usize,
    fg: u8,
    right_fg: u8,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: replay FIXTURE.json".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(512 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 512 * 1024 {
        return Err("fixture exceeds 512 KiB".into());
    }
    let case: Case = serde_json::from_slice(&bytes)?;
    if case.chunks.is_empty() || case.checks.is_empty() {
        return Err("empty replay coverage".into());
    }
    let mut screen = Screen::new(case.cols, case.rows)?;
    for chunk in &case.chunks {
        screen.feed(chunk);
    }
    let frame = screen.frame();
    if frame.cursor != case.cursor {
        return Err("cursor differs from independent expected state".into());
    }
    for check in &case.checks {
        let glyph = frame
            .glyphs
            .iter()
            .find(|g| g.col == check.col && g.row == check.row)
            .ok_or("expected glyph is missing")?;
        if glyph.text != check.text
            || glyph.width != check.width
            || glyph.left.fg != check.fg
            || glyph.right.fg != check.right_fg
        {
            return Err("glyph differs from independent expected state".into());
        }
    }
    let combined: Vec<_> = case.chunks.iter().flatten().copied().collect();
    for split in 0..=combined.len() {
        let mut alternate = Screen::new(case.cols, case.rows)?;
        alternate.feed(&combined[..split]);
        alternate.feed(&combined[split..]);
        if alternate != screen {
            return Err("chunk boundary changes screen state".into());
        }
    }
    println!(
        "{}",
        serde_json::json!({"status":"passed", "case":case.name,
        "bytes":combined.len(), "split_points":combined.len()+1, "glyph_checks":case.checks.len()})
    );
    Ok(())
}
