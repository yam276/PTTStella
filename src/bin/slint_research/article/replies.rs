//! UI-only placement and scrolling of the common reducer's filtered reply index.
use super::{ArticleSession, ArticleView};
use crate::ReplySideMark;
use pttstella::{reply_navigation::Index, surface_state::Identity};
use slint::{ModelRc, VecModel};
use std::{ops::Range, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Controls {
    valid: bool,
    retained: bool,
    ready: bool,
    source: Option<Identity>,
    group: usize,
    groups: [bool; 4],
    open: bool,
    current: usize,
    total: usize,
}
#[derive(Default)]
pub(super) struct Navigation {
    scope: Option<(u64, u64)>,
    source: Option<Identity>,
    index: Rc<Index>,
    group: usize,
    active: Option<usize>,
    open: bool,
    shown: Option<Controls>,
}
impl Navigation {
    fn rows(&self) -> &[usize] {
        match self.group {
            1 => &self.index.mine_rows,
            2 => &self.index.focused_rows,
            _ => &self.index.author_rows,
        }
    }
    fn groups(&self) -> [bool; 4] {
        [
            !self.index.author_rows.is_empty(),
            !self.index.mine_rows.is_empty(),
            !self.index.focused_rows.is_empty(),
            self.index.mine,
        ]
    }
    fn install(&mut self, scope: (u64, u64), source: Identity, index: &Index) {
        if self.scope != Some(scope) {
            *self = Self::default();
            self.scope = Some(scope);
        }
        if self.source != Some(source) {
            self.source = Some(source);
            self.index = Rc::new(index.clone());
            if self.rows().is_empty() {
                self.group = self.groups()[..3].iter().position(|b| *b).unwrap_or(0);
                self.active = None;
            }
        }
    }
    fn step(&mut self, visible: Range<usize>, back: bool) -> Option<usize> {
        let rows = self.rows();
        if rows.is_empty() {
            return None;
        }
        let active = self
            .active
            .filter(|row| visible.contains(row))
            .and_then(|row| rows.binary_search(&row).ok());
        let at = if let Some(at) = active {
            if back {
                (at + rows.len() - 1) % rows.len()
            } else {
                (at + 1) % rows.len()
            }
        } else if back {
            rows.partition_point(|row| *row < visible.start)
                .checked_sub(1)
                .unwrap_or(rows.len() - 1)
        } else {
            let at = rows.partition_point(|row| *row < visible.start);
            if at == rows.len() { 0 } else { at }
        };
        self.active = Some(rows[at]);
        self.active
    }
}
impl ArticleSession {
    pub(super) fn install_replies(self: &Rc<Self>, ui: &crate::TerminalWindow) {
        let weak = Rc::downgrade(self);
        ui.on_reply_navigation_action(move |action| {
            if let Some(owner) = weak.upgrade() {
                owner.reply_action(action);
            }
        });
    }
    pub(super) fn reply_close(&self, focus: bool) {
        self.reply_navigation.borrow_mut().open = false;
        if let Some(ui) = self.ui.upgrade() {
            ui.set_reply_navigation_open(false);
            if focus && ui.get_context_focus_restorable() {
                ui.invoke_focus_terminal();
            }
        }
    }
    fn reply_action(self: &Rc<Self>, action: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || ui.get_native_composing() || ui.get_find_composing() {
            return;
        }
        if action == 1 {
            self.reply_close(true);
            return;
        }
        if if action == 0 {
            ui.get_reply_navigation_opening_blocked()
        } else {
            ui.get_reply_navigation_blocked()
        } {
            return;
        }
        // Lease the acknowledged index before closing search: that close may
        // retire its painted decorations until the same source is repainted.
        self.reply_sync_ui();
        if !ui.get_reply_navigation_available() || !ui.get_reply_navigation_ready() {
            return;
        }
        if action == 0 {
            ui.invoke_context_dismiss(false);
            ui.invoke_tools_dismiss(false);
            ui.invoke_clear_selection();
            ui.invoke_reset_keyboard();
            if ui.get_find_open() {
                self.core.borrow_mut().find_finish();
                self.find_sync_ui();
            }
            self.reply_navigation.borrow_mut().open = true;
            ui.set_reply_navigation_open(true);
            ui.invoke_focus_reply_navigation();
            return;
        }
        if !self.reply_navigation.borrow().open {
            return;
        }
        if (4..=6).contains(&action) {
            let mut state = self.reply_navigation.borrow_mut();
            let group = (action - 4) as usize;
            if state.groups()[group] {
                state.group = group;
                state.active = None;
            }
            drop(state);
            self.reply_sync_ui();
        } else if matches!(action, 2 | 3) {
            let visible = self
                .view
                .borrow()
                .as_ref()
                .and_then(|view| view.reply_visible());
            if let Some(visible) = visible {
                let row = self
                    .reply_navigation
                    .borrow_mut()
                    .step(visible, action == 2);
                if let Some(row) = row
                    && let Some(view) = self.view.borrow().as_ref()
                {
                    view.reply_reveal(row);
                }
                self.reply_sync_ui();
            }
        }
    }
    pub(super) fn reply_sync_ui(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let view = self.connection.updates.borrow();
        let id = self.output_identity(&view);
        let native_ready = self.view.borrow().as_ref().is_some_and(|native| {
            native.geometry().is_some()
                && self.reply_navigation.borrow().source
                    == Some(Identity::of(&native.source.borrow()))
        });
        let (controls, index, changed) = {
            let core = self.core.borrow();
            let mut state = self.reply_navigation.borrow_mut();
            let scope = core
                .reading_scope()
                .map(|(generation, article, _)| (generation, article));
            let paint = id
                .and_then(|id| core.current(id, &view))
                .filter(|p| p.ptt_features && p.article.is_some());
            let valid = if let (Some(scope), Some(paint)) = (scope, paint) {
                state.install(scope, Identity::of(paint), &paint.reply_navigation);
                true
            } else {
                if state.scope.is_some_and(|old| Some(old) != scope) {
                    *state = Navigation::default();
                }
                false
            };
            let total = state.rows().len();
            let retained =
                state.source.is_some() && core.retained_reading_identity(&view) == state.source;
            let current = state
                .active
                .and_then(|row| state.rows().binary_search(&row).ok())
                .map_or(0, |at| at + 1);
            let controls = Controls {
                valid,
                retained,
                ready: valid || retained && native_ready,
                source: state.source,
                group: state.group,
                groups: state.groups(),
                open: state.open,
                current,
                total,
            };
            let changed = state.shown.replace(controls) != Some(controls);
            (controls, state.index.clone(), changed)
        };
        drop(view);
        if !changed {
            return;
        }
        let Controls {
            valid,
            retained,
            ready,
            source,
            group,
            groups,
            open,
            current,
            total,
        } = controls;
        // Keep chrome in place while the same article is repainting. Geometry
        // ACK controls permission, rather than repeatedly changing the layout.
        let available = source.is_some() && groups[..3].iter().any(|b| *b);
        ui.set_reply_navigation_ready(ready);
        ui.set_reply_navigation_available(available);
        ui.set_reply_navigation_groups(ModelRc::new(VecModel::from(groups.to_vec())));
        ui.set_reply_navigation_group(group as i32);
        ui.set_reply_navigation_count(if valid || retained {
            pttstella::msg!(
                "reply-navigation-count",
                current = (current as i64),
                total = (total as i64)
            )
            .into()
        } else {
            "".into()
        });
        ui.set_reply_navigation_open(open);
        if let Some(native) = self.view.borrow().as_ref() {
            native.set_reply_index(if valid || retained {
                source.map(|source| (source, index))
            } else {
                None
            });
        } else {
            ui.set_reply_side_marks(ModelRc::default());
        }
    }
    pub(super) fn reply_language_changed(&self) {
        self.reply_navigation.borrow_mut().shown = None;
        self.reply_sync_ui();
    }
    pub(super) fn reply_finish(&self) {
        self.reply_navigation.replace(Navigation::default());
        if let Some(ui) = self.ui.upgrade() {
            ui.set_reply_navigation_open(false);
            ui.set_reply_navigation_ready(false);
            ui.set_reply_navigation_available(false);
            ui.set_reply_navigation_count("".into());
            ui.set_reply_navigation_groups(ModelRc::default());
            ui.set_reply_side_marks(ModelRc::default());
        }
    }
    pub(super) fn replies_stopped(&self) -> bool {
        let state = self.reply_navigation.borrow();
        !state.open
            && state.scope.is_none()
            && state.source.is_none()
            && state.index.marks.is_empty()
            && state.active.is_none()
            && state.shown.is_none()
    }
}
impl ArticleView {
    fn reply_visible(&self) -> Option<Range<usize>> {
        self.geometry()?;
        let layout = self.layout.get()?;
        let flow = self.flow()?;
        let y = self.position(&flow);
        let mut rows = flow.visible(y, f64::from(layout.body_height * layout.dpi), 0);
        if rows.start < rows.end && flow.top(rows.start) as f64 + f64::from(layout.row) <= y {
            rows.start += 1;
        }
        Some(rows)
    }
    fn reply_reveal(self: &Rc<Self>, row: usize) {
        let Some(layout) = self.layout.get() else {
            return;
        };
        if let Some(flow) = self.flow() {
            self.scroll_pixels(
                flow.top(row) as f64 - f64::from(layout.body_height * layout.dpi) / 2.,
            );
        }
    }
    fn set_reply_index(&self, index: Option<(Identity, Rc<Index>)>) {
        let unchanged = match (self.reply_index.borrow().as_ref(), index.as_ref()) {
            (Some((a, x)), Some((b, y))) => a == b && Rc::ptr_eq(x, y),
            (None, None) => true,
            _ => false,
        };
        if unchanged {
            return;
        }
        self.reply_index.replace(index);
        self.reply_marks();
    }
    pub(crate) fn reply_marks(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let marks = (|| {
            let index = self.reply_index.borrow();
            let (identity, index) = index.as_ref()?;
            if Identity::of(&self.source.borrow()) != *identity {
                return None;
            }
            self.geometry()?;
            let layout = self.layout.get()?;
            let flow = self.flow()?;
            let y = self.position(&flow);
            let viewport = f64::from(layout.body_height * layout.dpi);
            Some(side_marks(
                index, &flow, y, viewport, layout.row, layout.dpi,
            ))
        })()
        .unwrap_or_default();
        ui.set_reply_side_marks(ModelRc::new(VecModel::from(marks)));
    }
}

fn side_marks(
    index: &Index,
    flow: &pttstella::article_flow::Flow,
    y: f64,
    viewport: f64,
    row: u32,
    dpi: f32,
) -> Vec<ReplySideMark> {
    let visible = flow.visible(y, viewport, 0);
    let start = index.marks.partition_point(|m| m.row < visible.start);
    index.marks[start..]
        .iter()
        .take_while(|m| m.row < visible.end)
        .filter_map(|m| {
            let top = flow.top(m.row) as f64 - y;
            (top >= 0. && top + f64::from(row) <= viewport).then_some(ReplySideMark {
                row: m.row as i32,
                y: (top / f64::from(dpi)) as f32,
                author: m.author,
                mine: m.mine,
                first: m.first,
            })
        })
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{article_flow::Flow, reply_navigation::Mark};
    #[test]
    fn visible_role_marks_skip_partial_rows_and_image_gaps_in_physical_pixels() {
        let index = Index {
            marks: (0..100)
                .map(|row| Mark {
                    row,
                    author: true,
                    mine: false,
                    first: row != 5,
                })
                .collect(),
            ..Index::default()
        };
        let flow = Flow::new(100, 20, [(2, 100)]).unwrap();
        assert!(side_marks(&index, &flow, 55., 100., 20, 2.).is_empty());
        let visible = side_marks(&index, &flow, 165., 100., 20, 2.);
        assert_eq!(
            visible.iter().map(|m| (m.row, m.y)).collect::<Vec<_>>(),
            [(4, 7.5), (5, 17.5), (6, 27.5), (7, 37.5)]
        );
        assert!(!visible[1].first);
        assert!(visible.iter().all(|m| m.author && !m.mine));
    }
    #[test]
    fn side_mark_model_has_an_independent_visible_budget() {
        let index = Index {
            marks: (0..4000)
                .map(|row| Mark {
                    row,
                    author: false,
                    mine: true,
                    first: true,
                })
                .collect(),
            ..Index::default()
        };
        let flow = Flow::new(4000, 10, []).unwrap();
        let visible = side_marks(&index, &flow, 1000., 1000., 10, 1.);
        assert_eq!(visible.len(), 64);
        assert_eq!(visible.first().unwrap().row, 100);
        assert_eq!(visible.last().unwrap().row, 163);
    }
    #[test]
    fn local_steps_follow_the_viewport_and_generation_cannot_keep_an_old_group() {
        let mut state = Navigation::default();
        let identity = Identity {
            generation: 1,
            revision: 4,
            presentation: 0,
        };
        let index = Index {
            author_rows: vec![3, 4, 10, 18],
            mine_rows: vec![6, 14],
            mine: true,
            ..Index::default()
        };
        state.install((1, 9), identity, &index);
        state.open = true;
        assert_eq!(state.step(8..16, false), Some(10));
        assert_eq!(state.step(8..16, false), Some(18));
        assert_eq!(state.step(12..23, false), Some(3));
        state.active = None;
        assert_eq!(state.step(12..23, true), Some(10));
        state.group = 1;
        state.active = None;
        assert_eq!(state.step(8..16, false), Some(14));
        state.install(
            (2, 1),
            Identity {
                generation: 2,
                ..identity
            },
            &index,
        );
        assert!(!state.open && state.active.is_none() && state.group == 0);
    }
}
