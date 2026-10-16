//! UI projection ownership: one paint in flight, one acknowledged snapshot and
//! interactions that can only consume a current acknowledged presentation.
#[cfg(test)]
#[path = "surface_state_tests.rs"]
mod tests;
use super::presentation::{Paint, current_presentation, paint};
use pttstella::{
    connection::{Phase, View},
    export, filters, links,
    media::hover::Lease,
    navigation::{Context, Stamp},
    selection::{self, SelectedText, Selection},
    settings::DisplaySettings,
};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A snapshot may be saved only while its UI policy and source still match.
#[derive(Clone)]
pub(super) struct ExportLease {
    identity: Identity,
    revoked: Arc<AtomicBool>,
    input_epoch: Option<u64>,
}
impl ExportLease {
    pub(super) fn check(&self, view: &View) -> Result<(), String> {
        if self
            .input_epoch
            .is_some_and(|epoch| epoch != view.input_epoch)
            || self.revoked.load(Ordering::Acquire)
            || (view.generation, view.frame.revision)
                != (self.identity.generation, self.identity.revision)
        {
            Err("畫面、規則或選取已變更；PNG 未儲存，請重新匯出".into())
        } else {
            Ok(())
        }
    }
}
struct ActiveExport {
    lease: ExportLease,
    selection: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub presentation: u64,
    pub generation: u64,
    pub revision: u64,
}
impl Identity {
    fn of(paint: &Paint) -> Self {
        Self {
            presentation: paint.presentation,
            generation: paint.generation,
            revision: paint.frame.revision,
        }
    }
}
struct SelectedRows {
    identity: Identity,
    request: u64,
    rows: Range<usize>,
    input_epoch: u64,
}
pub(super) struct SurfaceState {
    filters: filters::State,
    in_flight: Option<Paint>,
    displayed: Option<Paint>,
    dirty: bool,
    selected: Option<SelectedRows>,
    hover: Option<Lease>,
    export: Option<ActiveExport>,
}
impl SurfaceState {
    pub(super) fn new(rules: filters::Rules) -> Self {
        Self {
            filters: filters::State::new(rules),
            in_flight: None,
            displayed: None,
            dirty: true,
            selected: None,
            hover: None,
            export: None,
        }
    }
    pub(super) fn rules(&self) -> &filters::Rules {
        self.filters.rules()
    }
    pub(super) fn has_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }
    pub(super) fn dirty(&self) -> bool {
        self.dirty
    }
    pub(super) fn display_changed(&mut self) {
        self.revoke_export();
        self.dirty = true;
    }
    pub(super) fn queue(
        &mut self,
        view: &View,
        probe: bool,
        settings: &DisplaySettings,
    ) -> Option<&Paint> {
        if !self.dirty || self.in_flight.is_some() {
            return None;
        }
        let mut packet = paint(view, probe, settings, &self.filters);
        packet.hover = self
            .hover
            .as_ref()
            .filter(|lease| lease.stamp == view.stamp())
            .cloned();
        self.in_flight = Some(packet);
        self.dirty = false;
        self.in_flight.as_ref()
    }
    pub(super) fn acknowledge(&mut self, identity: Identity) -> Result<(), String> {
        if self.in_flight.as_ref().map(Identity::of) != Some(identity) {
            return Err("畫面回報未知版本".into());
        }
        self.displayed = self.in_flight.take();
        Ok(())
    }
    pub(super) fn painted(&self, view: &View) -> bool {
        self.displayed.as_ref().is_some_and(|p| {
            (p.generation, p.frame.revision) == (view.generation, view.frame.revision)
        })
    }
    pub(super) fn current(&self, identity: Identity, view: &View) -> Option<&Paint> {
        self.displayed.as_ref().filter(|p| {
            current_presentation(
                p,
                identity.presentation,
                identity.generation,
                identity.revision,
                &self.filters,
                view,
            )
        })
    }
    pub(super) fn navigation_shown(&self, stamp: Stamp) -> bool {
        self.displayed.as_ref().is_some_and(|p| {
            p.generation == stamp.generation
                && p.frame.revision == stamp.revision
                && p.input_epoch == stamp.input_epoch
        })
    }
    /// Vim may advance its logical cursor before the next terminal redraw.
    pub(super) fn browse_shown(&self, generation: u64, context: &Context) -> bool {
        self.displayed
            .as_ref()
            .is_some_and(|p| p.generation == generation && p.navigation.kind() == context.kind())
    }
    fn policy_changed(&mut self) {
        self.revoke_export();
        self.selected = None;
        self.hover = None;
        self.dirty = true;
    }
    pub(super) fn replace_rules(&mut self, rules: filters::Rules) {
        self.filters.replace(rules);
        self.policy_changed();
    }
    pub(super) fn reveal(&mut self, identity: Identity, reveal: bool, view: &View) -> bool {
        if self.filters.version() != identity.presentation
            || (view.generation, view.frame.revision) != (identity.generation, identity.revision)
        {
            return false;
        }
        self.filters
            .reveal(identity.generation, identity.revision, reveal);
        self.policy_changed();
        true
    }
    /// Even an old in-flight ACK must be drained before queuing the newest view.
    /// Its snapshot stays ineligible for current interactions after this update.
    pub(super) fn update(&mut self, view: &View) -> bool {
        let version = self.filters.version();
        self.filters.sync(view.generation, view.frame.revision);
        if self.export.as_ref().is_some_and(|active| {
            active.lease.identity.presentation != self.filters.version()
                || active.lease.check(view).is_err()
        }) {
            self.revoke_export();
        }
        if self.filters.version() != version
            || self.selected.as_ref().is_some_and(|s| {
                s.input_epoch != view.input_epoch
                    || (s.identity.generation, s.identity.revision)
                        != (view.generation, view.frame.revision)
            })
        {
            self.selected = None;
        }
        let filtered = self.filters.project(view.generation, &view.frame);
        let revoked = self.hover.as_mut().is_some_and(|lease| {
            !lease.renew(
                view.stamp(),
                view.phase == Phase::Connected && view.frame.control_complete,
                &links::detect(&filtered.frame),
            )
        });
        if revoked {
            self.hover = None;
        }
        self.dirty = true;
        revoked
    }
    pub(super) fn select(
        &mut self,
        identity: Identity,
        request: u64,
        selection: &Selection,
        view: &View,
    ) -> Option<SelectedText> {
        let selected = self
            .current(identity, view)
            .and_then(|p| selection::extract(&p.frame, selection).ok());
        if self.export.as_ref().is_some_and(|active| {
            active.selection.is_some_and(|old| {
                old != request || active.lease.identity != identity || selected.is_none()
            })
        }) {
            self.revoke_export();
        }
        self.selected = selected.as_ref().map(|s| SelectedRows {
            identity,
            request,
            rows: s.rows.clone(),
            input_epoch: view.input_epoch,
        });
        selected
    }
    pub(super) fn copied(&self, identity: Identity, request: u64, view: &View) -> bool {
        self.selected.as_ref().is_some_and(|s| {
            s.identity == identity && s.request == request && s.input_epoch == view.input_epoch
        }) && self.current(identity, view).is_some()
    }
    pub(super) fn export_rows(
        &self,
        identity: Identity,
        request: Option<u64>,
        view: &View,
    ) -> Result<Range<usize>, String> {
        let shown = self
            .current(identity, view)
            .ok_or("畫面已更新，請重新選取後匯出")?;
        if (shown.frame.cols, shown.frame.rows) != (80, export::ROWS) {
            return Err("此畫面尺寸不支援 PNG 匯出".into());
        }
        match request {
            None => Ok(0..export::ROWS),
            Some(request) => self
                .selected
                .as_ref()
                .filter(|s| {
                    s.identity == identity
                        && s.request == request
                        && s.input_epoch == view.input_epoch
                })
                .map(|s| s.rows.clone())
                .ok_or_else(|| "選取已變更，請重新選取後匯出".into()),
        }
    }
    pub(super) fn clear_selection(&mut self, identity: Identity, request: u64) {
        if self
            .selected
            .as_ref()
            .is_some_and(|s| s.identity == identity && s.request == request)
        {
            self.selected = None;
            if self.export.as_ref().is_some_and(|active| {
                active.selection == Some(request) && active.lease.identity == identity
            }) {
                self.revoke_export();
            }
        }
    }
    pub(super) fn begin_hover(&mut self, lease: Lease) {
        self.hover = Some(lease);
    }
    fn revoke_export(&mut self) {
        if let Some(active) = self.export.take() {
            active.lease.revoked.store(true, Ordering::Release);
        }
    }
    /// Called inside Files' admitted factory, so a busy rejection never revokes
    /// the already accepted export. export_rows validates identity first.
    pub(super) fn lease_export(
        &mut self,
        identity: Identity,
        selection: Option<u64>,
        input_epoch: u64,
    ) -> ExportLease {
        self.revoke_export();
        let lease = ExportLease {
            identity,
            revoked: Arc::new(AtomicBool::new(false)),
            input_epoch: selection.map(|_| input_epoch),
        };
        self.export = Some(ActiveExport {
            lease: lease.clone(),
            selection,
        });
        lease
    }
    pub(super) fn clear_hover(&mut self) -> bool {
        self.hover.take().is_some()
    }
    pub(super) fn cancel_hover(&mut self, sequence: u64) -> bool {
        if self
            .hover
            .as_ref()
            .is_some_and(|lease| lease.sequence == sequence)
        {
            self.hover = None;
            true
        } else {
            false
        }
    }
}
impl Drop for SurfaceState {
    fn drop(&mut self) {
        self.revoke_export();
    }
}
