//! Native scheduling over the shared recent-board state and one pending queue.
//! The file thread and its result outlive a cancelled UI waiter and are joined.
use crate::{TerminalWindow, file_job::ResultSlot};
use pttstella::recent_boards::{
    BoardVisit,
    session::{Snapshot, State},
    store::Store,
    writer::{Completed, Prepared, Queue, Ticket},
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    thread::JoinHandle,
};
use tokio::sync::{Notify, oneshot};

struct Flight {
    thread: JoinHandle<Result<(), String>>,
    slot: ResultSlot<Completed>,
    ticket: Ticket,
}
pub struct Recent {
    ui: slint::Weak<TerminalWindow>,
    state: RefCell<State>,
    queue: RefCell<Queue>,
    worker: RefCell<Option<Flight>>,
    waiter: RefCell<Option<slint::JoinHandle<()>>>,
    closing: Cell<bool>,
    changed: Notify,
    error: RefCell<Option<String>>,
}
impl Recent {
    /// Called before starting the UI event loop. Disk loading stays on an owned
    /// blocking worker and is joined even if a document is invalid.
    pub fn mount(ui: &TerminalWindow, store: Result<Store, String>) -> Rc<Self> {
        let state = std::thread::Builder::new()
            .name("pttstella-recent-load".into())
            .spawn(move || State::with_store(store));
        let state = match state {
            Ok(worker) => worker.join().unwrap_or_else(|_| {
                State::with_store(Err(pttstella::msg!(
                    "file-worker-failed",
                    e = "recent load worker panicked"
                )))
            }),
            Err(e) => State::with_store(Err(e.to_string())),
        };
        Rc::new(Self {
            ui: ui.as_weak(),
            state: RefCell::new(state),
            queue: RefCell::new(Queue::default()),
            worker: RefCell::new(None),
            waiter: RefCell::new(None),
            closing: Cell::new(false),
            changed: Notify::new(),
            error: RefCell::new(None),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().packet()
    }
    pub fn observe(self: &Rc<Self>, visit: Option<&BoardVisit>) -> bool {
        if self.closing.get() {
            return false;
        }
        let changed = self.state.borrow_mut().observe_visit(visit, |write| {
            self.queue.borrow_mut().enqueue(write);
            Ok(())
        });
        if changed {
            self.kick();
        }
        changed
    }
    pub fn clear(self: &Rc<Self>) {
        if self.closing.get() {
            return;
        }
        self.state.borrow_mut().clear(|write| {
            self.queue.borrow_mut().enqueue(write);
            Ok(())
        });
        self.kick();
        self.publish();
    }
    fn publish(&self) {
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_destination_updated();
        }
    }
    fn spawn(prepared: Prepared) -> Result<(Flight, oneshot::Receiver<()>), (Ticket, String)> {
        let ticket = prepared.ticket();
        let slot = ResultSlot::new();
        let result = slot.clone();
        let (send, receive) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("pttstella-recent-save".into())
            .spawn(move || {
                // Dropping the sender also wakes the waiter after a panic. The
                // persistent Flight owns the native join and result collection.
                let _completion = send;
                result.publish(Ok(prepared.run()))
            })
            .map_err(|e| (ticket, e.to_string()))?;
        Ok((
            Flight {
                thread,
                slot,
                ticket,
            },
            receive,
        ))
    }
    fn apply(&self, done: Completed) {
        self.error.replace(done.result.as_ref().err().cloned());
        self.queue.borrow_mut().complete(&done);
        self.state.borrow_mut().complete(done);
    }
    fn collect(&self) {
        let Some(flight) = self.worker.borrow_mut().take() else {
            return;
        };
        let done = match flight.thread.join() {
            Ok(Ok(())) => flight
                .slot
                .take()
                .unwrap_or_else(|e| flight.ticket.complete(Err(e))),
            Ok(Err(e)) => flight.ticket.complete(Err(e)),
            Err(_) => flight.ticket.complete(Err(pttstella::msg!(
                "file-worker-failed",
                e = "recent save worker panicked"
            ))),
        };
        self.apply(done);
    }
    fn kick(self: &Rc<Self>) {
        if self.worker.borrow().is_some() {
            return;
        }
        let Some(prepared) = self.queue.borrow_mut().begin() else {
            self.changed.notify_one();
            return;
        };
        match Self::spawn(prepared) {
            Ok((flight, receive)) => {
                self.worker.replace(Some(flight));
                let weak = Rc::downgrade(self);
                match slint::spawn_local(async move {
                    let _ = receive.await;
                    if let Some(owner) = weak.upgrade() {
                        owner.collect();
                        owner.publish();
                        owner.kick();
                        owner.changed.notify_one();
                    }
                }) {
                    Ok(task) => {
                        self.waiter.replace(Some(task));
                    }
                    Err(e) => {
                        self.collect();
                        self.error.replace(Some(e.to_string()));
                        self.changed.notify_one();
                        self.publish();
                    }
                }
            }
            Err((ticket, e)) => {
                self.apply(ticket.complete(Err(e)));
                self.changed.notify_one();
                self.publish();
            }
        }
    }
    pub(super) fn pressure(&self) -> (bool, bool) {
        (
            self.worker.borrow().is_some(),
            self.queue.borrow().pending(),
        )
    }
    pub(super) fn stopped(&self) -> bool {
        self.closing.get() && !self.busy() && self.waiter.borrow().is_none()
    }
    pub(super) fn last_error(&self) -> Option<String> {
        self.error.borrow().clone()
    }
    pub fn busy(&self) -> bool {
        self.worker.borrow().is_some() || self.queue.borrow().pending()
    }
    pub async fn shutdown(self: &Rc<Self>) {
        self.closing.set(true);
        self.kick();
        while self.busy() {
            self.changed.notified().await;
        }
    }
    pub fn finish(&self) -> Result<(), String> {
        self.closing.set(true);
        if let Some(waiter) = self.waiter.borrow_mut().take() {
            waiter.abort();
        }
        // Also join on a backend/event-loop failure, without needing more UI
        // callbacks. No accepted pending write is detached or silently lost.
        loop {
            self.collect();
            let Some(prepared) = self.queue.borrow_mut().begin() else {
                break;
            };
            match Self::spawn(prepared) {
                Ok((flight, _)) => {
                    self.worker.replace(Some(flight));
                }
                Err((ticket, e)) => self.apply(ticket.complete(Err(e))),
            }
        }
        self.error.borrow().clone().map_or(Ok(()), Err)
    }
}
