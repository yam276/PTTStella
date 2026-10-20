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
        let mut fields = line.split('=').filter(|part| !part.is_empty());
        let Some(id) = fields.next() else {
            skipped += 1;
            continue;
        };
        let value = fields.next().unwrap_or("");
        let (policy, note, adapted) = match value.split_once(':') {
            None => (Policy::Mask, value, false),
            Some((prefix, note)) => {
                let classes: Vec<_> = prefix.chars().filter(|c| c.is_ascii_alphabetic()).collect();
                if classes.len() != 1
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
                match classes[0].to_ascii_uppercase() {
                    'B' => (Policy::Mask, note, prefix != "B"),
                    'C' => (Policy::MaskRow, note, true),
                    'W' => (Policy::Allow, note, prefix != "W"),
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
        rules = rules.edit_policy(id, note, policy, false)?;
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
}
