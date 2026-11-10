//! The native output adapter consumes the same acknowledged Rust presentation.
use super::ArticleSession;
use pttstella::{
    connection::View,
    presentation::Paint,
    screen::Frame,
    selection::{SelectedText, Selection},
    surface_state::{CopyTarget, Identity, OutputLease},
};

impl ArticleSession {
    pub fn lease_media_output(&self, view: &View) -> Option<OutputLease> {
        if self.closed.get() {
            return None;
        }
        let mut core = self.core.borrow_mut();
        let id = Identity::of(core.last_displayed()?);
        core.current(id, view)?;
        Some(core.lease_picture(id, view.input_epoch))
    }
    /// An already-authorized preview survives geometry-only repainting. New
    /// gestures still require output_identity and its installed geometry ACK.
    pub fn renew_media(&self, view: &View, lease: &mut pttstella::media::hover::Lease) -> bool {
        if self.closed.get() {
            return false;
        }
        let core = self.core.borrow();
        let Some(displayed) = core.last_displayed() else {
            return false;
        };
        let Some(paint) = core.current(Identity::of(displayed), view) else {
            return false;
        };
        lease.renew(
            view.stamp(),
            view.phase == pttstella::connection::Phase::Connected
                && !view.output_pending
                && !view.input_pending
                && view.frame.control_complete,
            &paint.links,
        )
    }
    pub fn settings(&self) -> pttstella::settings::DisplaySettings {
        self.display.borrow().clone()
    }
    pub fn rules(&self) -> pttstella::filters::Rules {
        self.core.borrow().rules().clone()
    }
    pub fn replace_rules(&self, rules: pttstella::filters::Rules) {
        let view = self.connection.updates.borrow().clone();
        self.core.borrow_mut().replace_rules(rules);
        self.presentation_changed(&view);
    }
    pub fn output_identity(&self, view: &View) -> Option<Identity> {
        let ui = self.ui.upgrade()?;
        if ui.get_frame_paint_pending() {
            return None;
        }
        let id = self.retained_output_identity(view)?;
        let core = self.core.borrow();
        let paint = core.current(id, view)?;
        if ui.get_article_active() {
            self.view.borrow().as_ref()?.geometry()?;
            paint.article.as_ref()?;
        } else if paint.article.is_some()
            || self.terminal_shown.get() != Some(id)
            || self.terminal_size.get().is_none()
            || ui.get_frame_paint_pending()
            || self.terminal_presented.get()
                != Some(crate::session::FrameId {
                    generation: id.generation,
                    revision: id.revision,
                })
        {
            return None;
        }
        (paint.input_epoch == view.input_epoch && !view.input_pending).then_some(id)
    }
    /// Keep an already-admitted preview across geometry-only repainting. This
    /// does not authorize a new gesture or send: those require output_identity.
    pub fn retained_output_identity(&self, view: &View) -> Option<Identity> {
        if self.closed.get() || self.composition_handoff.get().is_some() {
            return None;
        }
        let core = self.core.borrow();
        let id = Identity::of(core.last_displayed()?);
        let paint = core.current(id, view)?;
        (paint.input_epoch == view.input_epoch && !view.input_pending).then_some(id)
    }
    pub fn with_output<R>(&self, view: &View, read: impl FnOnce(&Paint) -> R) -> Option<R> {
        let id = self.output_identity(view)?;
        let core = self.core.borrow();
        Some(read(core.current(id, view)?))
    }
    pub fn output_geometry(&self) -> Option<crate::pointer::SurfaceGeometry> {
        self.view.borrow().as_ref()?.geometry()
    }
    pub fn select_output(
        &self,
        view: &View,
        request: u64,
        selection: &Selection,
    ) -> Option<(Identity, SelectedText)> {
        let id = self.output_identity(view)?;
        self.core
            .borrow_mut()
            .select(id, request, selection, view)
            .map(|text| (id, text))
    }
    pub fn copied_output(&self, id: Identity, request: u64, view: &View) -> bool {
        self.output_identity(view) == Some(id) && self.core.borrow().copied(id, request, view)
    }
    pub fn clear_output_selection(&self, id: Identity, request: u64) {
        self.core.borrow_mut().clear_selection(id, request);
    }
    pub fn copy_output(&self, view: &View, target: &CopyTarget) -> Option<String> {
        let id = self.output_identity(view)?;
        self.core.borrow().copy_text(id, target, view)
    }
    pub fn lease_output(&self, view: &View, request: Option<u64>) -> Option<OutputLease> {
        let id = self.output_identity(view)?;
        Some(
            self.core
                .borrow_mut()
                .lease_output(id, request, view.input_epoch),
        )
    }
    pub fn revoke_output(&self) {
        self.core.borrow_mut().revoke_output();
    }
    pub fn export_output(
        &self,
        view: &View,
        request: Option<u64>,
    ) -> Result<(Frame, std::ops::Range<usize>), String> {
        let id = self
            .output_identity(view)
            .ok_or_else(|| pttstella::msg!("export-screen-changed"))?;
        let core = self.core.borrow();
        let rows = core.export_rows(id, request, view)?;
        let paint = core
            .current(id, view)
            .ok_or_else(|| pttstella::msg!("export-screen-changed"))?;
        if request.is_none() {
            return Ok((
                paint.source_frame.as_ref().unwrap_or(&paint.frame).clone(),
                rows,
            ));
        }
        let frame = crate::output::crop_rows(&paint.frame, rows.clone())?;
        Ok((frame, 0..rows.len()))
    }
}
