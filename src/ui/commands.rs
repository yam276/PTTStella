//! One cancelable UI wait; the Connection remains the only writer of station bytes.
use std::{future::Future, pin::Pin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Login,
    Navigate,
    Browse,
    Board,
}
pub(super) struct Completion {
    pub(super) kind: Kind,
    pub(super) generation: u64,
    pub(super) result: Result<(), String>,
}
type Reply = Pin<Box<dyn Future<Output = Completion>>>;

#[derive(Default)]
pub(super) struct Pending {
    reply: Option<Reply>,
    kind: Option<Kind>,
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
            }
        }));
        self.kind = Some(kind);
        Ok(())
    }
    pub(super) async fn next(&mut self) -> Completion {
        let result = match &mut self.reply {
            Some(reply) => reply.await,
            None => std::future::pending().await,
        };
        self.reply = None;
        self.kind = None;
        result
    }
    /// Drops the Connection reply receiver, revoking commands still in its queue.
    /// An in-progress write cannot be rolled back and is never retried here.
    pub(super) fn cancel(&mut self) {
        self.reply = None;
        self.kind = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

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
        pending.cancel();
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
                    if event == "close" { pending.cancel(); }
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
