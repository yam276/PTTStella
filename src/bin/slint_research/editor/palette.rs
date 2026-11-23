//! A popup owns the exact selected raw ranges, source mode and document version.
use super::viewport::Viewport;
use pttstella::{
    composer::document::{Document, Mode, Version},
    editor_color::Color,
};
use std::ops::Range;
pub struct Lease {
    version: Version,
    mode: Mode,
    ranges: Vec<Range<usize>>,
}
impl Lease {
    pub fn new(document: &Document, viewport: &Viewport) -> Result<Self, String> {
        let ranges = viewport.ranges(document)?;
        if ranges.iter().all(Range::is_empty) {
            return Err(pttstella::msg!("editor-color-empty"));
        }
        Ok(Self {
            version: document.version(),
            mode: document.mode(),
            ranges,
        })
    }
    pub fn apply(
        &self,
        document: &mut Document,
        viewport: &Viewport,
        color: Color,
    ) -> Result<(), String> {
        if self.version != document.version()
            || self.mode != document.mode()
            || self.ranges != viewport.ranges(document)?
        {
            return Err(pttstella::msg!("draft-color-stale"));
        }
        document
            .color(self.version, &self.ranges, color)
            .map(|_| ())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::composer::document::Selection;
    #[test]
    fn popup_lease_rejects_selection_changes_without_revision_and_cross_draft_reuse() {
        let mut d = Document::new(1, "title".into(), "一二三".into()).unwrap();
        let view = Viewport::default();
        d.select(d.version(), Selection { anchor: 0, head: 3 })
            .unwrap();
        let lease = Lease::new(&d, &view).unwrap();
        d.select(d.version(), Selection { anchor: 3, head: 6 })
            .unwrap();
        let before = d.source().to_owned();
        assert!(
            lease
                .apply(
                    &mut d,
                    &view,
                    Color {
                        foreground: 9,
                        background: 0
                    }
                )
                .is_err()
        );
        assert_eq!(d.source(), before);
        let mut other = Document::new(2, "title".into(), "一二三".into()).unwrap();
        other
            .select(other.version(), Selection { anchor: 0, head: 3 })
            .unwrap();
        assert!(
            lease
                .apply(
                    &mut other,
                    &view,
                    Color {
                        foreground: 9,
                        background: 0
                    }
                )
                .is_err()
        );
    }
    #[test]
    fn rectangle_popup_is_one_transaction_and_mode_changes_are_stale() {
        let mut d = Document::new(1, "title".into(), "A中B\nC文D\n".into()).unwrap();
        let mut view = Viewport::default();
        view.pointer(&mut d, 0, 0, 2, false, true).unwrap();
        view.pointer(&mut d, 2, 1, 3, false, true).unwrap();
        let lease = Lease::new(&d, &view).unwrap();
        let before = d.source().to_owned();
        lease
            .apply(
                &mut d,
                &view,
                Color {
                    foreground: 10,
                    background: 0,
                },
            )
            .unwrap();
        assert!(d.source().contains("A"));
        d.undo(d.version()).unwrap();
        assert_eq!(d.source(), before);
        let lease = Lease::new(&d, &view).unwrap();
        d.set_mode(Mode::Source);
        assert!(
            lease
                .apply(
                    &mut d,
                    &view,
                    Color {
                        foreground: 9,
                        background: 0
                    }
                )
                .is_err()
        );
    }
}
