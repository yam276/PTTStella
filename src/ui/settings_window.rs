//! A single auxiliary window. Preferences and all I/O remain with the surface owners.
use super::{outbound::Packet, protocol::SurfaceEvent};
use dioxus::prelude::*;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc;

#[derive(Default)]
struct State {
    creating: bool,
    owner: Option<dioxus::desktop::WeakDesktopContext>,
    window: Option<dioxus::desktop::WeakDesktopContext>,
    eval: Option<document::Eval>,
}
#[derive(Clone)]
struct Shared {
    state: Rc<RefCell<State>>,
    sender: mpsc::Sender<SurfaceEvent>,
}
impl PartialEq for Shared {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}
type Opening = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<dioxus::desktop::DesktopContext, String>>>,
>;

pub(super) struct SettingsWindow {
    shared: Shared,
    receiver: mpsc::Receiver<SurfaceEvent>,
    directory: Option<std::path::PathBuf>,
    debug_port: Option<u16>,
    pending: Option<Opening>,
    store: Option<Result<pttstella::settings::store::Store, String>>,
}
impl Default for SettingsWindow {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel(32);
        Self {
            shared: Shared {
                state: Rc::new(RefCell::new(State::default())),
                sender,
            },
            receiver,
            directory: None,
            debug_port: None,
            pending: None,
            store: None,
        }
    }
}
impl SettingsWindow {
    pub(super) fn configure(
        &mut self,
        path: Option<&std::path::Path>,
        debug_port: Option<u16>,
        reload: bool,
    ) {
        self.shared.state.borrow_mut().owner = Some(Rc::downgrade(&dioxus::desktop::window()));
        self.debug_port = debug_port;
        self.directory = path.map(|path| {
            path.join(if debug_port.is_some() {
                if reload { "webview-reload" } else { "webview" }
            } else {
                "settings-webview"
            })
        });
    }
    pub(super) fn open(&mut self, store: Result<pttstella::settings::store::Store, String>) {
        self.store = Some(store);
        let mut state = self.shared.state.borrow_mut();
        if let Some(window) = state.window.as_ref().and_then(std::rc::Weak::upgrade) {
            window.set_visible(true);
            window.set_focus();
            return;
        }
        if state.creating {
            return;
        }
        state.creating = true;
        let dom = VirtualDom::new_with_props(
            view,
            ViewProps {
                shared: self.shared.clone(),
            },
        );
        let config = dioxus::desktop::Config::new()
            .with_disable_context_menu(true)
            .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::WindowHides)
            .with_window(
                dioxus::desktop::WindowBuilder::new()
                    .with_title(pttstella::msg!("settings-center-title"))
                    .with_inner_size(dioxus::desktop::LogicalSize::new(900.0, 700.0))
                    .with_min_inner_size(dioxus::desktop::LogicalSize::new(620.0, 440.0)),
            );
        let config = if let Some(path) = &self.directory {
            config.with_data_directory(path)
        } else {
            config
        };
        let config = config.with_menu(None);
        let config = if let Some(port) = self.debug_port {
            config.with_windows_browser_args(format!(
                "--remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"
            ))
        } else {
            config
        };
        // Creation is queued by Dioxus; the component registers its own weak handle.
        let Some(owner) = state.owner.as_ref().and_then(std::rc::Weak::upgrade) else {
            state.creating = false;
            return;
        };
        let opening = owner.new_window(dom, config);
        self.pending = Some(Box::pin(async move {
            opening.try_resolve().await.map_err(|e| e.to_string())
        }));
    }
    pub(super) fn send(&self, packet: &Packet<'_>) -> Result<bool, document::EvalError> {
        let state = self.shared.state.borrow();
        if let Some(eval) = state.eval {
            if matches!(
                packet,
                Packet::SettingsSaved(super::outbound::Feedback {
                    status: super::outbound::SaveStatus::Rejected,
                    ..
                })
            ) && let Some(window) = state.window.as_ref().and_then(std::rc::Weak::upgrade)
            {
                window.set_visible(true);
                window.set_focus();
            }
            eval.send(packet)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) async fn recv(&mut self) -> SurfaceEvent {
        loop {
            tokio::select! {
                ready=async {match self.pending.as_mut(){Some(opening)=>opening.await,None=>std::future::pending().await}}=>{
                    self.pending=None;
                    match ready {
                        Ok(window)=>{window.set_visible(true);window.set_focus();self.shared.state.borrow_mut().window=Some(Rc::downgrade(&window));},
                        Err(message)=>return SurfaceEvent::Error{message},
                    }
                },
                event=self.receiver.recv()=>return match event{Some(event)=>event,None=>std::future::pending().await},
            }
        }
    }
    pub(super) fn focus_main(&self) {
        if let Some(window) = self
            .shared
            .state
            .borrow()
            .owner
            .as_ref()
            .and_then(std::rc::Weak::upgrade)
        {
            window.set_visible(true);
            window.set_focus();
        }
    }
    /// Stop timers/listeners, retain the last authorized unsent edit, then close.
    /// Disk draining happens after the interactive file owner has finished.
    pub(super) async fn finish(&mut self) -> Result<(), String> {
        let result = async {
            if let Some(pending) = self.pending.take() {
                let window = pending.await?;
                self.shared.state.borrow_mut().window = Some(Rc::downgrade(&window));
            }
            let eval = self.shared.state.borrow_mut().eval.take();
            if let Some(eval) = eval {
                eval.send(Packet::Stop).map_err(|e| e.to_string())?;
                eval.join::<Snapshot>().await.map_err(|e| e.to_string())
            } else {
                Ok(Snapshot {
                    last: None,
                    error: None,
                })
            }
        }
        .await;
        if let Some(window) = self
            .shared
            .state
            .borrow_mut()
            .window
            .take()
            .and_then(|w| w.upgrade())
        {
            window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
            window.close();
        }
        self.shared.state.borrow_mut().creating = false;
        let snapshot = result?;
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        if let Some(edit) = snapshot.last {
            let store = self.store.clone().ok_or("settings store unavailable")??;
            super::files::flush_settings(store, final_change(edit)?).await?;
        }
        Ok(())
    }
}
#[derive(serde::Deserialize)]
struct Snapshot {
    last: Option<SurfaceEvent>,
    error: Option<String>,
}
fn final_change(edit: SurfaceEvent) -> Result<super::preferences::Change, String> {
    match edit {
        SurfaceEvent::SettingsSave {
            change,
            trusted: true,
        } => Ok(change),
        _ => Err("invalid settings snapshot".into()),
    }
}

#[derive(Clone, PartialEq, Props)]
struct ViewProps {
    shared: Shared,
}
fn view(props: ViewProps) -> Element {
    dioxus::desktop::use_asset_handler("pttstella-fonts", |request, responder| {
        responder.respond(crate::fonts::response(&request))
    });
    let shared = props.shared.clone();
    use_hook(move || {
        let mut state = shared.state.borrow_mut();
        state.creating = false;
        state.window = Some(Rc::downgrade(&dioxus::desktop::window()));
    });
    let close_shared = props.shared.clone();
    dioxus::desktop::use_wry_event_handler(move |event, _| {
        if matches!(
            event,
            dioxus::desktop::tao::event::Event::WindowEvent {
                event: dioxus::desktop::WindowEvent::CloseRequested,
                ..
            }
        ) && let Some(eval) = close_shared.state.borrow().eval
        {
            let _ = eval.send(serde_json::json!({"kind":"settings_close"}));
        }
    });
    rsx! {
        style {{include_str!("../../assets/app.css")}}
        div { class:"settings-host", onmounted:move |_|{
            let shared=props.shared.clone();
            spawn(async move {
                let script=["const fluent=(()=>{const exports={};const module={exports};\n",include_str!("../../assets/vendor/fluent/index.js"),"\nreturn module.exports;})();\n",
                    include_str!("../../assets/i18n.js"),include_str!("../../assets/surface-fonts.js"),include_str!("../../assets/surface-renderer.js"),
                    include_str!("../../assets/settings-catalog.js"),include_str!("../../assets/surface-settings.js"),include_str!("../../assets/settings-window.js")].concat();
                let mut eval=document::eval(&script);shared.state.borrow_mut().eval=Some(eval);
                let language=pttstella::i18n::application().language();
                if eval.send(Packet::Localization{language:language.tag(),resource:language.resource(),fallback:pttstella::i18n::Language::TraditionalChinese.resource()}).is_err(){return;}
                // Internal request for the current effective snapshot, never a send to the station.
                if shared.sender.send(SurfaceEvent::SettingsOpen{trusted:true}).await.is_err(){return;}
                while let Ok(value)=eval.recv::<serde_json::Value>().await {
                    if value["kind"]=="settings_attention" {if let Some(window)=shared.state.borrow().window.as_ref().and_then(std::rc::Weak::upgrade){window.set_visible(true);window.set_focus();}continue;}
                    if value["kind"]=="settings_hide" {
                        {
                            let state=shared.state.borrow();
                            if let Some(window)=state.window.as_ref().and_then(std::rc::Weak::upgrade){window.set_visible(false);}
                            if let Some(window)=state.owner.as_ref().and_then(std::rc::Weak::upgrade){window.set_visible(true);window.set_focus();}
                        }
                        if shared.sender.send(SurfaceEvent::SettingsVisibility{open:false}).await.is_err(){break;}
                        continue;
                    }
                    let Ok(event)=serde_json::from_value::<SurfaceEvent>(value) else {continue;};
                    if !allowed(&event){continue;}
                    if shared.sender.send(event).await.is_err(){break;}
                }
            });
        },super::app::SettingsForm{}}
    }
}
fn allowed(event: &SurfaceEvent) -> bool {
    matches!(
        event,
        SurfaceEvent::SettingsOpen { .. }
            | SurfaceEvent::SettingsSave { .. }
            | SurfaceEvent::SettingsPreview { .. }
            | SurfaceEvent::SettingsFonts { .. }
            | SurfaceEvent::SettingsProfile { .. }
            | SurfaceEvent::FontSource { .. }
            | SurfaceEvent::SettingsLaunch { .. }
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn auxiliary_window_cannot_submit_station_input_and_final_edit_preserves_other_preferences()
     {
        let event = serde_json::from_value(
            serde_json::json!({"kind":"settings_launch","feature":"sites","trusted":true}),
        )
        .unwrap();
        assert!(allowed(&event));
        assert!(!allowed(&SurfaceEvent::StationMouseEnabled {
            generation: 0,
            enabled: true,
            trusted: true
        }));
        let directory = tempfile::tempdir().unwrap();
        let store = pttstella::settings::store::Store::at(directory.path().join("settings.json"));
        let app = pttstella::settings::AppSettings {
            hover: pttstella::settings::HoverPosition::Off,
            ..Default::default()
        };
        store.save(app).unwrap();
        let event=serde_json::from_value(serde_json::json!({"kind":"settings_save","values":{"fonts":{"kind":"platform_default"},"font_size":18,"baseline":19},"language":"en-US","hover":null,"tools_opacity":40,"tools_access":{"right_click_menu":false,"show_floating_button":false},"trusted":true})).unwrap();
        super::super::files::flush_settings(store.clone(), final_change(event).unwrap())
            .await
            .unwrap();
        let saved = store.load().unwrap();
        assert_eq!(saved.display.font_size, 18.0);
        assert_eq!(saved.hover, pttstella::settings::HoverPosition::Off);
        assert!(!saved.tools_access.show_floating_button);
        let untrusted = serde_json::from_value(
            serde_json::json!({"kind":"settings_launch","feature":"sites","trusted":false}),
        )
        .unwrap();
        assert!(final_change(untrusted).is_err());
    }
    #[tokio::test]
    async fn cancelling_receive_retains_the_pending_window_creation() {
        let mut window = SettingsWindow::default();
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        window.pending = Some(Box::pin(async move {
            receiver.await.map_err(|e| e.to_string())?;
            Err("synthetic creation failure".into())
        }));
        tokio::select! {_ = window.recv()=>panic!("creation should still be pending"),_ = tokio::time::sleep(std::time::Duration::from_millis(5))=>{}}
        assert!(window.pending.is_some());
        sender.send(()).unwrap();
        assert!(
            matches!(window.recv().await,SurfaceEvent::Error{message} if message=="synthetic creation failure")
        );
        assert!(window.pending.is_none());
    }
}
