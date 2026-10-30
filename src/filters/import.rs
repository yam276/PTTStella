//! Bounded import of line-based account rules. Only declared policies are used.
use super::{Entry, MAX_BYTES, MAX_ENTRIES, Policy, Rules};

#[derive(Debug)]
pub struct Parsed {
    pub entries: Vec<Entry>,
    pub skipped: usize,
    pub adjusted: usize,
}

pub fn parse(bytes: &[u8]) -> Result<Parsed, String> {
    if bytes.len() > MAX_BYTES {
        return Err(crate::msg!("filters-too-large"));
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| crate::msg!("filters-import-invalid"))?;
    let mut rules = Rules::default();
    let mut skipped = 0;
    let mut adjusted = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line
            .split('=')
            .map(str::trim)
            .filter(|part| !part.is_empty());
        let Some(id) = fields.next() else {
            skipped += 1;
            continue;
        };
        let value = fields.next().unwrap_or("");
        let (policy, color, note, adapted) = match value.split_once(':') {
            None => (Policy::Mask, None, value, false),
            Some((prefix, note)) => {
                let prefix = prefix.trim();
                let classes: Vec<_> = prefix.chars().filter(|c| c.is_ascii_alphabetic()).collect();
                if classes.len() > 1
                    || !prefix.chars().all(|c| {
                        matches!(
                            c,
                            'B' | 'b' | 'C' | 'c' | 'W' | 'w' | 'N' | 'n' | '_' | '1'..='9'
                        )
                    })
                {
                    skipped += 1;
                    continue;
                }
                // Numbered markers are categories, not RGB or ANSI indices.
                // One ID has one local colour: use the lowest numbered marker
                // so source ordering and duplicate markers do not change it.
                let marker = prefix.bytes().filter(|b| matches!(b, b'1'..=b'9')).min();
                let color =
                    marker.map(|m| [9, 10, 11, 12, 13, 14, 15, 6, 5][usize::from(m - b'1')]);
                match classes.first().map(char::to_ascii_uppercase) {
                    Some('B') => (Policy::Mask, None, note, prefix != "B"),
                    Some('C') => (Policy::MaskRow, None, note, true),
                    Some('W') => (Policy::Allow, color, note, prefix != "W"),
                    Some('N') | None if marker.is_some() => (Policy::Allow, color, note, true),
                    _ => {
                        skipped += 1;
                        continue;
                    }
                }
            }
        };
        if !super::valid_id(id) || note.chars().count() > 128 || note.chars().any(char::is_control)
        {
            skipped += 1;
            continue;
        }
        // Stable, case-insensitive identity. Explicit exceptions win regardless
        // of line order, including when merging with existing local rules.
        if rules
            .entries
            .iter()
            .any(|e| e.id.eq_ignore_ascii_case(id) && e.policy == Policy::Allow)
            && policy != Policy::Allow
        {
            continue;
        }
        rules = rules.edit_color(id, note, policy, color, false)?;
        adjusted += usize::from(adapted);
    }
    if rules.entries.is_empty() {
        return Err(crate::msg!("filters-import-invalid"));
    }
    if rules.entries.len() > MAX_ENTRIES {
        return Err(crate::msg!("filters-capacity"));
    }
    Ok(Parsed {
        entries: rules.entries,
        skipped,
        adjusted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_examples_cover_bom_notes_delimiters_and_white_priority() {
        let p = parse("\u{feff}Alice\r\nBob=W:保留\r\nalice=W:例外\r\nALICE=B:遮黑\r\nCarol=B:備註:續文=不讀\r\nDave=W\r\nEve==B:空分隔\r\n".as_bytes()).unwrap();
        assert_eq!(
            p.entries
                .iter()
                .map(|e| (e.id.as_str(), e.policy, e.note.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("alice", Policy::Allow, "例外"),
                ("Bob", Policy::Allow, "保留"),
                ("Carol", Policy::Mask, "備註:續文"),
                ("Dave", Policy::Mask, "W"),
                ("Eve", Policy::Mask, "空分隔")
            ]
        );
        assert_eq!(p.skipped, 0);
        let marked =
            parse(b"Alice=w13_:keep\nBob=C:mask\nCarol=N:note only\nDave=BW:ambiguous").unwrap();
        assert_eq!(marked.entries[0].policy, Policy::Allow);
        assert_eq!(marked.entries[0].color, Some(9));
        assert_eq!(marked.entries[1].policy, Policy::MaskRow);
        assert_eq!((marked.adjusted, marked.skipped), (2, 2));
        let p = parse(b"Alice=B:ok\nOther=X:unsupported\n123=bad\n").unwrap();
        assert_eq!(p.skipped, 2);
        assert_eq!(p.entries.len(), 1);
        assert!(parse(b"Other=X:unsupported").is_err());
        assert!(parse(b"\xffbad").is_err());
        assert!(parse(&vec![b'a'; MAX_BYTES + 1]).is_err());
        let full = (0..=MAX_ENTRIES)
            .map(|i| format!("User{i}\n"))
            .collect::<String>();
        assert!(parse(full.as_bytes()).is_err());
    }

    #[test]
    fn numbered_categories_become_visible_colours_without_changing_blocking_policies() {
        let parsed = parse(b"One=W1:red\nTwo=W2:green\nThree=W3:yellow\nFour=W4:blue\nFive=W5:magenta\nSix=W6:cyan\nSeven=W7:white\nEight=W8:teal\nNine=W9:purple\nShared=W:default\nMask=_B12:content\nRow=C9:row\nMarker=31:category only\nNeutral=N2:category only\n").unwrap();
        let colours: Vec<_> = parsed.entries[..9].iter().map(|e| e.color).collect();
        assert_eq!(
            colours,
            vec![
                Some(9),
                Some(10),
                Some(11),
                Some(12),
                Some(13),
                Some(14),
                Some(15),
                Some(6),
                Some(5)
            ]
        );
        assert_eq!(parsed.entries[9].color, None);
        assert_eq!(
            (parsed.entries[10].policy, parsed.entries[10].color),
            (Policy::Mask, None)
        );
        assert_eq!(
            (parsed.entries[11].policy, parsed.entries[11].color),
            (Policy::MaskRow, None)
        );
        assert_eq!(
            (parsed.entries[12].policy, parsed.entries[12].color),
            (Policy::Allow, Some(9))
        );
        assert_eq!(
            (parsed.entries[13].policy, parsed.entries[13].color),
            (Policy::Allow, Some(10))
        );
        assert_eq!((parsed.skipped, parsed.adjusted), (0, 13));
        Rules {
            entries: parsed.entries,
            ..Rules::default()
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn marked_import_keeps_notes_and_rejects_unknown_markers_without_guessing() {
        let parsed = parse("\u{feff} Alice = _w9311:中文備註:保留\r\nBob=B:一般\r\nALICE=C:不得蓋掉白單\r\nBad=W0:未知\r\nOther=BW1:混合\r\nEmpty=N:無分類\r\nOnly=_:無分類\r\nNope=WN3:混合\r\n".as_bytes()).unwrap();
        assert_eq!((parsed.skipped, parsed.adjusted), (5, 1));
        assert_eq!(
            parsed.entries,
            vec![
                Entry {
                    id: "Alice".into(),
                    note: "中文備註:保留".into(),
                    policy: Policy::Allow,
                    color: Some(9)
                },
                Entry {
                    id: "Bob".into(),
                    note: "一般".into(),
                    policy: Policy::Mask,
                    color: None
                }
            ]
        );
        assert_eq!(
            parse(b"Alice=W913:note").unwrap().entries,
            parse(b"Alice=W3319:note").unwrap().entries
        );
    }
}
