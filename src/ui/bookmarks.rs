//! Desktop-only command submission; persistence and acquisition are shared.
pub(super) use pttstella::bookmarks::session::*;

pub(super) fn start_capture(
    screen: &mut super::surface_state::SurfaceState,
    commands: &mut super::commands::Pending,
    connection: &pttstella::connection::Connection,
    view: &pttstella::connection::View,
    request: u64,
    stamp: pttstella::navigation::Stamp,
    id: u64,
) -> Result<Acquisition, String> {
    let (frame, destination) = screen.bookmark_current(stamp, id, view)?;
    if let Err(e) = commands.start(super::commands::Kind::Bookmark, view.generation, || {
        connection
            .request_go_to(stamp, destination)
            .map(|reply| reply.wait())
    }) {
        screen.fail_updates();
        return Err(e);
    }
    Ok(Acquisition::new(request, stamp, frame))
}
