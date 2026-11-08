//! The window grants permission for the current presentation. Connection owns the timer.
use crate::{TerminalWindow, session::FrameId};
use pttstella::{
    connection::{Connection, Phase, View},
    msg,
    navigation::Stamp,
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};
#[path = "article_keepalive_probe.rs"]
pub mod probe;

#[derive(Default)]
struct Admission {
    last: Option<(Stamp, bool)>,
    acknowledged: bool,
}

impl Admission {
    fn update(
        &mut self,
        view: &View,
        presented: Option<FrameId>,
        surface_ready: bool,
    ) -> Option<(Stamp, bool)> {
        let stamp = view.stamp();
        let ready = surface_ready
            && view.keep_alive
            && view.phase == Phase::Connected
            && !view.input_pending
            && !view.output_pending
            && !view.login_pending
            && presented
                == Some(FrameId {
                    generation: stamp.generation,
                    revision: stamp.revision,
                })
            && view.context().allows_keepalive();
        let signal = (stamp, ready);
        // A clock gap can revoke an acknowledged permit while the window stays focused.
        let renew = ready && self.acknowledged && !view.keep_alive_ready;
        self.acknowledged = view.keep_alive_ready;
        let previous = self.last.replace(signal);
        let changed = if ready {
            previous != Some(signal)
        } else {
            previous.is_none_or(|(_, was_ready)| was_ready)
        };
        (changed || renew).then_some(signal)
    }
}

pub fn mount(
    ui: &TerminalWindow,
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    article: Option<Rc<crate::article::session::ArticleSession>>,
) {
    ui.on_keep_alive_changed({
        let weak = ui.as_weak();
        let mut admission = Admission::default();
        let connection = connection.clone();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let surface_ready = ui.get_keep_alive_surface_ready();
            // Drop the watch guard before asking native geometry/properties.
            // The source page is always the bounded terminal View (80x24).
            let shown = if surface_ready && ui.get_article_active() {
                let view = connection.updates.borrow().clone();
                article
                    .as_ref()
                    .and_then(|article| article.output_identity(&view))
                    .map(|id| FrameId {
                        generation: id.generation,
                        revision: id.revision,
                    })
            } else {
                presented.get()
            };
            let (signal, available, enabled) = {
                let view = connection.updates.borrow();
                (
                    admission.update(&view, shown, surface_ready),
                    view.phase == Phase::Connected && view.site.ptt_features(),
                    view.keep_alive,
                )
            };
            if let Some((stamp, ready)) = signal {
                connection.keep_alive_gate(stamp, ready);
            }
            ui.set_keep_alive_available(available);
            ui.set_keep_alive_label(
                if enabled {
                    msg!("keepalive-on")
                } else {
                    msg!("keepalive-off")
                }
                .into(),
            );
        }
    });
    ui.on_toggle_keep_alive({
        let weak = ui.as_weak();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let (stamp, enabled) = {
                let view = connection.updates.borrow();
                if ui.get_switching_station()
                    || view.phase != Phase::Connected
                    || !view.site.ptt_features()
                {
                    return;
                }
                (view.stamp(), view.keep_alive)
            };
            connection.keep_alive_gate(stamp, false);
            if let Err(error) = connection.keep_alive(!enabled) {
                ui.set_input_notice(error.into());
            }
            ui.invoke_focus_terminal();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::connection::CloseOutcome;
    use std::time::Duration;

    #[tokio::test]
    async fn foreground_pause_and_coalesced_resume_restart_the_real_idle_interval() {
        let connection = Connection::start_keep_alive_probe().unwrap();
        let mut updates = connection.updates.clone();
        connection.connect().unwrap();
        let list = tokio::time::timeout(
            Duration::from_secs(4),
            updates.wait_for(|view| view.context().allows_keepalive()),
        )
        .await
        .unwrap()
        .unwrap()
        .clone();
        let id = FrameId {
            generation: list.generation,
            revision: list.frame.revision,
        };
        let stamp = list.stamp();
        let mut admission = Admission::default();
        let (s, ready) = admission.update(&list, Some(id), false).unwrap();
        assert_eq!((s, ready), (stamp, false));
        connection.keep_alive_gate(s, ready);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(3150),
                updates.wait_for(|v| v.keep_alive_sent != 0)
            )
            .await
            .is_err()
        );
        assert_eq!(admission.update(&list, None, true), None);

        let (s, ready) = admission.update(&list, Some(id), true).unwrap();
        assert!(ready);
        connection.keep_alive_gate(s, ready);
        let ready_view = tokio::time::timeout(
            Duration::from_secs(2),
            updates.wait_for(|view| view.keep_alive_ready),
        )
        .await
        .unwrap()
        .unwrap()
        .clone();
        assert_eq!(admission.update(&ready_view, Some(id), true), None);
        let mut resumed = ready_view.clone();
        resumed.keep_alive_ready = false;
        assert_eq!(
            admission.update(&resumed, Some(id), true),
            Some((stamp, true))
        );
        assert_eq!(admission.update(&resumed, Some(id), true), None);

        tokio::time::sleep(Duration::from_millis(1500)).await;
        for surface_ready in [false, true] {
            let (s, ready) = admission
                .update(&ready_view, Some(id), surface_ready)
                .unwrap();
            connection.keep_alive_gate(s, ready);
        }
        assert!(
            tokio::time::timeout(
                Duration::from_millis(1850),
                updates.wait_for(|v| v.keep_alive_sent != 0)
            )
            .await
            .is_err()
        );
        let refreshed = tokio::time::timeout(
            Duration::from_secs(3),
            updates.wait_for(|v| v.keep_alive_sent == 1 && !v.context().allows_keepalive()),
        )
        .await
        .unwrap()
        .unwrap()
        .clone();
        assert_eq!(
            admission.update(&refreshed, Some(id), true),
            Some((refreshed.stamp(), false))
        );
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
    }
}
