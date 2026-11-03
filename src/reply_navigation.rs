//! Reply metadata only; never changes ANSI glyphs, merged IDs or exported frames.
use crate::{
    filters::{self, Policy, Push, Rules},
    screen::Frame,
};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Index {
    pub author: Option<String>,
    pub mine: bool,
    pub marks: Vec<Mark>,
    pub author_rows: Vec<usize>,
    pub mine_rows: Vec<usize>,
    pub focused_rows: Vec<usize>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Mark {
    pub row: usize,
    pub author: bool,
    pub mine: bool,
    pub first: bool,
}
pub fn author(frame: &Frame) -> Option<String> {
    if frame.glyphs.iter().any(|g| g.row == 0 && g.unmapped) {
        return None;
    }
    let header: String = frame
        .glyphs
        .iter()
        .filter(|g| g.row == 0)
        .map(|g| g.text.as_str())
        .collect();
    crate::article_navigation::header_board(&header)?;
    let value = header
        .trim_start_matches(' ')
        .strip_prefix("作者")?
        .trim_start();
    let id = value.split([' ', '(']).next()?;
    filters::valid_id(id).then(|| id.to_owned())
}
pub fn index(
    replies: &[Push],
    rules: &Rules,
    revealed: bool,
    author: Option<String>,
    mine: Option<&str>,
    focused: Option<&str>,
) -> Index {
    let mut result = Index {
        author,
        mine: mine.is_some(),
        ..Index::default()
    };
    fn key(id: &str) -> Option<[u8; 12]> {
        if !filters::valid_id(id) {
            return None;
        }
        let mut value = [0; 12];
        for (to, from) in value.iter_mut().zip(id.bytes()) {
            *to = from.to_ascii_lowercase();
        }
        Some(value)
    }
    let hidden_ids: std::collections::HashSet<_> = rules
        .entries
        .iter()
        .filter(|e| e.policy == Policy::MaskRow)
        .filter_map(|e| key(&e.id))
        .collect();
    let mut previous: Option<&Push> = None;
    for reply in replies {
        let hidden = !revealed && key(&reply.id).is_some_and(|id| hidden_ids.contains(&id));
        if hidden {
            previous = None;
            continue;
        }
        let is_author = result
            .author
            .as_ref()
            .is_some_and(|id| id.eq_ignore_ascii_case(&reply.id));
        let is_mine = mine.is_some_and(|id| id.eq_ignore_ascii_case(&reply.id));
        if is_author {
            result.author_rows.push(reply.row);
        }
        if is_mine {
            result.mine_rows.push(reply.row);
        }
        if focused.is_some_and(|id| id.eq_ignore_ascii_case(&reply.id)) {
            result.focused_rows.push(reply.row);
        }
        if is_author || is_mine {
            result.marks.push(Mark {
                row: reply.row,
                author: is_author,
                mine: is_mine,
                first: !previous.is_some_and(|p| {
                    p.row + 1 == reply.row && p.id.eq_ignore_ascii_case(&reply.id)
                }),
            });
        }
        previous = Some(reply);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn push(row: usize, id: &str) -> Push {
        Push {
            row,
            id: id.into(),
            start: 9,
            end: 66,
        }
    }
    #[test]
    fn identity_roles_are_independent_of_colour_and_consecutive_id_merging() {
        let replies = vec![
            push(3, "Alice"),
            push(4, "alice"),
            push(5, "Bob"),
            push(7, "Alice"),
        ];
        let rules = Rules::default()
            .edit_color("Alice", "", Policy::Allow, Some(13), false)
            .unwrap();
        let value = index(
            &replies,
            &rules,
            false,
            Some("ALICE".into()),
            Some("alice"),
            Some("Bob"),
        );
        assert_eq!(value.author_rows, vec![3, 4, 7]);
        assert_eq!(value.mine_rows, vec![3, 4, 7]);
        assert_eq!(value.focused_rows, vec![5]);
        assert_eq!(
            value.marks.iter().map(|m| m.first).collect::<Vec<_>>(),
            vec![true, false, true]
        );
        assert!(value.marks.iter().all(|m| m.author && m.mine));
        assert_eq!(rules.entries[0].color, Some(13));
    }
    #[test]
    fn whole_row_masks_are_absent_even_from_counts_while_content_masks_keep_visible_identity() {
        let replies = vec![push(0, "Alice"), push(1, "Bob")];
        let rules = Rules::default()
            .edit_policy("Alice", "", Policy::MaskRow, false)
            .unwrap()
            .edit("Bob", "", false)
            .unwrap();
        let value = index(
            &replies,
            &rules,
            false,
            Some("Alice".into()),
            Some("Bob"),
            Some("Alice"),
        );
        assert!(value.author_rows.is_empty() && value.focused_rows.is_empty());
        assert_eq!(value.mine_rows, vec![1]);
        assert_eq!(value.marks.len(), 1);
        assert_eq!(
            index(&replies, &rules, true, Some("Alice".into()), None, None).author_rows,
            vec![0]
        );
    }
    #[test]
    fn padded_standard_headers_only_and_no_guessed_author_for_headerless_art() {
        let frame = |text: &str| {
            let mut s = crate::screen::Screen::new(80, 24).unwrap();
            s.feed(&crate::encoding::encode(text).unwrap());
            s.frame()
        };
        assert_eq!(
            author(&frame(" 作者  Alice (暱稱)                  看板  Stella")),
            Some("Alice".into())
        );
        for text in [
            "ANSI Alice 畫圖",
            "作者 Alice (暱稱)",
            " 作者 Alice! (暱稱) 看板 Stella",
        ] {
            assert_eq!(author(&frame(text)), None);
        }
    }
}
