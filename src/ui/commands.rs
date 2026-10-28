//! One cancelable UI wait; the Connection remains the only writer of station bytes.
use std::{future::Future, pin::Pin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Login,
    Navigate,
    Preload,
    Browse,
    Destination,
}
pub(super) struct Completion {
    pub(super) kind: Kind,
    pub(super) generation: u64,
    pub(super) result: Result<(), String>,
    pub(super) navigation: Option<super::protocol::NavigationTicket>,
}
type Reply = Pin<Box<dyn Future<Output = Completion>>>;

#[derive(Default)]
pub(super) struct Pending {
    reply: Option<Reply>,
    kind: Option<Kind>,
    navigation: Option<super::protocol::NavigationTicket>,
}
impl Pending {
    pub(super) fn busy(&self) -> bool {
        self.reply.is_some()
    }
    pub(super) fn is(&self, kind: Kind) -> bool {
        self.kind == Some(kind)
    }
    /// Never replace a live request or grow an additional UI command queue.
    pub(super) fn start<F: Future<Output = Result<(), String>> + 'static>(
        &mut self,
        kind: Kind,
        generation: u64,
        begin: impl FnOnce() -> Result<F, String>,
    ) -> Result<(), String> {
        if self.busy() {
            return Err(pttstella::msg!("command-busy"));
        }
        // Admit and enqueue synchronously, before a later native input event
        // can overtake this command. Only the result wait is deferred.
        let work = begin()?;
        self.reply = Some(Box::pin(async move {
            Completion {
                kind,
                generation,
                result: work.await,
                navigation: None,
            }
        }));
        self.kind = Some(kind);
        Ok(())
    }
    pub(super) fn start_navigation<F: Future<Output = Result<(), String>> + 'static>(
        &mut self,
        ticket: super::protocol::NavigationTicket,
        begin: impl FnOnce() -> Result<F, String>,
    ) -> Result<(), String> {
        self.start(Kind::Navigate, ticket.generation, begin)?;
        self.navigation = Some(ticket);
        Ok(())
    }
    pub(super) fn navigation(&self) -> Option<super::protocol::NavigationTicket> {
        self.navigation
    }
    pub(super) async fn next(&mut self) -> Completion {
        let mut result = match &mut self.reply {
            Some(reply) => reply.await,
            None => std::future::pending().await,
        };
        self.reply = None;
        self.kind = None;
        result.navigation = self.navigation.take();
        result
    }
    /// Drops the Connection reply receiver, revoking commands still in its queue.
    /// An in-progress write cannot be rolled back and is never retried here.
    pub(super) fn cancel(&mut self) -> Option<super::protocol::NavigationTicket> {
        self.reply = None;
        self.kind = None;
        self.navigation.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn navigation_ticket_survives_success_and_error_without_a_retry() {
        for fail in [false, true] {
            let mut pending = Pending::default();
            let ticket = super::super::protocol::NavigationTicket {
                request: 21,
                generation: 7,
            };
            let (answer, receiver) = oneshot::channel();
            pending
                .start_navigation(ticket, || {
                    Ok(async move { receiver.await.map_err(|e| e.to_string())? })
                })
                .unwrap();
            assert_eq!(pending.navigation(), Some(ticket));
            answer
                .send(if fail {
                    Err("navigation deadline expired".into())
                } else {
                    Ok(())
                })
                .unwrap();
            let completed = pending.next().await;
            assert_eq!(completed.navigation, Some(ticket));
            assert_eq!(completed.result.is_err(), fail);
            assert!(!pending.busy());
            assert!(pending.navigation().is_none());
            assert!(pending.cancel().is_none());
        }
    }

    #[tokio::test]
    async fn cancellation_releases_only_the_old_wait_and_closes_its_queue_ticket() {
        let mut pending = Pending::default();
        let old = super::super::protocol::NavigationTicket {
            request: 21,
            generation: 7,
        };
        let new = super::super::protocol::NavigationTicket {
            request: 22,
            generation: 7,
        };
        let (old_answer, old_receiver) = oneshot::channel::<Result<(), String>>();
        pending
            .start_navigation(old, || {
                Ok(async move { old_receiver.await.map_err(|e| e.to_string())? })
            })
            .unwrap();
        assert_eq!(pending.cancel(), Some(old));
        assert!(old_answer.is_closed());
        assert!(pending.cancel().is_none());
        pending
            .start_navigation(new, || Ok(async { Ok(()) }))
            .unwrap();
        assert_eq!(pending.next().await.navigation, Some(new));
    }

    #[tokio::test]
    async fn acceptance_enqueues_now_and_busy_rejection_never_calls_the_factory() {
        let mut pending = Pending::default();
        let mut order = Vec::new();
        pending
            .start(Kind::Browse, 7, || {
                order.push("browse");
                Ok(std::future::pending())
            })
            .unwrap();
        assert_eq!(order, ["browse"]); // No next() poll needed for admission.
        assert!(
            pending
                .start(Kind::Login, 7, || {
                    order.push("must not enqueue");
                    Ok(async { Ok(()) })
                })
                .is_err()
        );
        order.push("native");
        assert_eq!(order, ["browse", "native"]);
        let _ = pending.cancel();
    }

    #[tokio::test]
    async fn slow_reply_leaves_frame_and_close_events_pollable_and_cancels_queue_ticket() {
        let mut pending = Pending::default();
        let (answer, receiver) = oneshot::channel::<()>();
        pending
            .start(Kind::Navigate, 7, || {
                Ok(async move { receiver.await.map_err(|e| e.to_string()) })
            })
            .unwrap();
        for event in ["frame", "media", "close"] {
            tokio::select! {
                biased;
                _ = pending.next() => panic!("slow station reply unexpectedly completed"),
                event = async { event } => {
                    if event == "close" { let _ = pending.cancel(); }
                }
            }
        }
        assert!(answer.is_closed());
        assert!(!pending.busy());
    }

    #[tokio::test]
    async fn bounded_wait_keeps_original_result_and_does_not_start_second_work() {
        let mut pending = Pending::default();
        let (answer, receiver) = oneshot::channel();
        pending
            .start(Kind::Login, 8, || {
                Ok(async move { receiver.await.map_err(|e| e.to_string())? })
            })
            .unwrap();
        assert!(
            pending
                .start(Kind::Browse, 8, || Ok(async { panic!("second work ran") }))
                .is_err()
        );
        answer.send(Err("write failed".into())).unwrap();
        let done = pending.next().await;
        assert_eq!((done.kind, done.generation), (Kind::Login, 8));
        assert_eq!(done.result, Err("write failed".into()));
        assert!(!pending.busy());
    }
}
