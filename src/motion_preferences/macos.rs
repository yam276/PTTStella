use super::{Audit, Preference};
use block2::RcBlock;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSWorkspace, NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue,
};
use std::{
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;

pub struct Provider {
    center: Retained<NSNotificationCenter>,
    observer: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    stopped: Arc<AtomicBool>,
}
fn read() -> Preference {
    Preference::Known {
        reduced: NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion(),
        source: "macos_nsworkspace",
        raw_value: None,
    }
}
impl Provider {
    pub fn start(sender: watch::Sender<Preference>) -> Result<Self, String> {
        let _main = MainThreadMarker::new().ok_or("motion observer requires AppKit main thread")?;
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let stopped = Arc::new(AtomicBool::new(false));
        let guard = stopped.clone();
        let sink = sender.clone();
        // Compile-time proof for the unsafe Objective-C registration below.
        fn sendable<F: Fn(NonNull<NSNotification>) + Send + Sync + 'static>(f: F) -> F {
            f
        }
        let block = RcBlock::new(sendable(move |_notification| {
            if !guard.load(Ordering::Acquire) {
                sink.send_replace(read());
            }
        }));
        // SAFETY: This is the documented workspace notification center/name,
        // no filtered object, and the main operation queue. The block owns only
        // Send+Sync channel/atomic state and never borrows the provider or UI.
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification),
                None,
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        // Register before reading; queued callbacks reread current state on the
        // same main queue rather than publishing a captured, older bool.
        sender.send_replace(read());
        Ok(Self {
            center,
            observer: Some(observer),
            stopped,
        })
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.stopped.store(true, Ordering::Release);
        if let Some(observer) = self.observer.take() {
            // SAFETY: The retained token was returned by this exact center's
            // addObserver call and has not previously been removed.
            unsafe {
                self.center.removeObserver((*observer).as_ref());
            }
        }
        Ok(())
    }
    pub async fn shutdown(&mut self) -> Result<Audit, String> {
        self.cancel()?;
        Ok(Audit {
            listener_removed: self.observer.is_none(),
            worker_joined: None,
        })
    }
    pub fn verify_notification(&self) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("motion observer stopped".into());
        }
        // SAFETY: The NSString is Apple's static notification name; the object
        // is intentionally nil, as in the corresponding observer registration.
        unsafe {
            self.center.postNotificationName_object(
                NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
                None,
            );
        }
        Ok(())
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}
