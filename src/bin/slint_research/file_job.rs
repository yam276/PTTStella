//! A single owned file result survives event-loop shutdown. Output still owns
//! and joins the worker; both settings and station editors consume this slot.
use std::sync::{Arc, Mutex};
pub struct ResultSlot<T>(Arc<Mutex<Option<Result<T, String>>>>);
impl<T> Clone for ResultSlot<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> ResultSlot<T> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
    pub fn publish(&self, result: Result<T, String>) -> Result<(), String> {
        *self
            .0
            .lock()
            .map_err(|_| pttstella::msg!("file-worker-failed", e = "result lock poisoned"))? =
            Some(result);
        Ok(())
    }
    pub fn take(&self) -> Result<T, String> {
        self.0
            .lock()
            .map_err(|_| pttstella::msg!("file-worker-failed", e = "result lock poisoned"))?
            .take()
            .ok_or_else(|| {
                pttstella::msg!("file-worker-failed", e = "worker finished without a result")
            })?
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joined_worker_result_survives_without_a_ui_consumer_and_is_taken_once() {
        let slot = ResultSlot::new();
        let worker = slot.clone();
        std::thread::spawn(move || worker.publish(Ok(vec![1, 2, 3])))
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(slot.take().unwrap(), vec![1, 2, 3]);
        assert!(slot.take().is_err());
        let failed = ResultSlot::<()>::new();
        failed.publish(Err("disk failure".into())).unwrap();
        assert_eq!(failed.take(), Err("disk failure".into()));
    }
}
