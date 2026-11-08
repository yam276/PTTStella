//! Frontend enqueue adapter; confirmation, revisions and clear semantics are shared.
use super::{files::Files, files_recent};
pub(super) struct State(pttstella::recent_boards::session::State);
impl State {
    pub(super) fn load(probe: Option<&std::path::Path>) -> Self {
        Self(pttstella::recent_boards::session::State::load(probe))
    }
    pub(super) fn packet(&self) -> super::outbound::RecentBoardsPacket {
        self.0.packet()
    }
    pub(super) fn observe(
        &mut self,
        view: &pttstella::connection::View,
        files: &mut Files,
    ) -> bool {
        self.0.observe(view, |write| files.save_recent(write))
    }
    pub(super) fn clear(&mut self, files: &mut Files) {
        self.0.clear(|write| files.save_recent(write));
    }
    pub(super) fn complete(&mut self, done: files_recent::Completed) {
        self.0.complete(done);
    }
}
