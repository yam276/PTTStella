use super::{Audit, Preference};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::watch;
use windows::{
    Foundation::TypedEventHandler,
    UI::ViewManagement::{UISettings, UISettingsAnimationsEnabledChangedEventArgs},
};

pub struct Provider {
    settings: UISettings,
    token: Option<i64>,
    stopped: Arc<AtomicBool>,
}
fn refresh(
    settings: &UISettings,
    sender: &watch::Sender<Preference>,
    gate: &Mutex<()>,
    stopped: &AtomicBool,
) {
    let Ok(_serial) = gate.lock() else {
        sender.send_replace(Preference::failed(
            "serialize",
            "motion reader lock poisoned",
        ));
        return;
    };
    if stopped.load(Ordering::Acquire) {
        return;
    }
    let reading = match settings.AnimationsEnabled() {
        Ok(enabled) => Preference::Known {
            reduced: !enabled,
            source: "windows_uisettings",
            raw_value: None,
        },
        Err(error) => Preference::failed("read", error),
    };
    sender.send_replace(reading);
}
impl Provider {
    pub fn start(sender: watch::Sender<Preference>) -> Result<Self, String> {
        let settings = UISettings::new().map_err(|e| e.to_string())?;
        let gate = Arc::new(Mutex::new(()));
        let stopped = Arc::new(AtomicBool::new(false));
        let (serial, guard, sink) = (gate.clone(), stopped.clone(), sender.clone());
        let handler =
            TypedEventHandler::<UISettings, UISettingsAnimationsEnabledChangedEventArgs>::new(
                move |settings, _| {
                    if let Some(settings) = settings.as_ref() {
                        refresh(settings, &sink, &serial, &guard);
                    }
                    Ok(())
                },
            );
        let token = settings
            .AnimationsEnabledChanged(&handler)
            .map_err(|e| e.to_string())?;
        // The callback and initial read+publish share a lock, so an older
        // initial read cannot overwrite a concurrent newer OS event.
        refresh(&settings, &sender, &gate, &stopped);
        Ok(Self {
            settings,
            token: Some(token),
            stopped,
        })
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.stopped.store(true, Ordering::Release);
        if let Some(token) = self.token {
            self.settings
                .RemoveAnimationsEnabledChanged(token)
                .map_err(|e| e.to_string())?;
            self.token = None;
        }
        Ok(())
    }
    pub async fn shutdown(&mut self) -> Result<Audit, String> {
        self.cancel()?;
        Ok(Audit {
            listener_removed: self.token.is_none(),
            worker_joined: None,
        })
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        if let Err(error) = self.cancel() {
            eprintln!("motion observer removal unproven: {error}");
        }
    }
}
