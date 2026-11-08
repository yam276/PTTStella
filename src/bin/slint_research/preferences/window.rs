use super::*;
use crate::{
    SettingsMatch, SettingsWindow,
    renderer::{Renderer, Size},
};
use pttstella::settings::PortableProfile;
use slint::{
    ModelRc, VecModel,
    winit_030::{
        EventResult, WinitWindowAccessor,
        winit::event::{Ime, WindowEvent},
    },
};
#[path = "../../../fonts/catalog.rs"]
mod fonts;

fn strings(values: impl IntoIterator<Item = String>) -> ModelRc<slint::SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}
impl Preferences {
    pub(super) fn open(self: &Rc<Self>) {
        if self.stopped.get() {
            return;
        }
        if self.ui.upgrade().is_some_and(|u| {
            u.get_native_composing()
                || u.get_editor_composing()
                || u.get_login_open()
                || u.get_paste_open()
                || u.get_station_color_open()
        }) {
            return;
        }
        if self.window.borrow().is_none() {
            match self.create_window() {
                Ok(window) => {
                    self.window.replace(Some(window));
                }
                Err(error) => {
                    self.notice(error);
                    return;
                }
            }
        }
        self.closing.set(false);
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade());
        if let Some(window) = window {
            if !self.ui.upgrade().is_some_and(|u| u.get_settings_open()) {
                self.model.borrow_mut().open();
                let app = self.pending.borrow().clone().unwrap_or_else(|| self.app());
                form::populate(&window, &app);
                self.localize(&window);
                self.preview(&window);
            }
            if let Some(ui) = self.ui.upgrade() {
                ui.invoke_tools_dismiss(false);
                ui.invoke_context_dismiss(false);
                ui.invoke_reset_keyboard();
                ui.set_settings_open(true);
            }
            if let Err(error) = window.show() {
                self.notice(error.to_string());
                return;
            }
            window.window().with_winit_window(|w| w.focus_window());
            window.invoke_focus_search();
        }
    }
    fn create_window(self: &Rc<Self>) -> Result<SettingsWindow, String> {
        let window = SettingsWindow::new().map_err(|e| e.to_string())?;
        window.on_edit({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.edited();
                }
            }
        });
        window.on_custom_font({
            let weak = Rc::downgrade(self);
            let w = window.as_weak();
            move || {
                if let Some(w) = w.upgrade() {
                    w.set_preset(8);
                }
                if let Some(o) = weak.upgrade() {
                    o.edited();
                }
            }
        });
        window.on_font_preset({
            let weak = Rc::downgrade(self);
            let w = window.as_weak();
            move || {
                if let (Some(o), Some(w)) = (weak.upgrade(), w.upgrade()) {
                    if let Ok(draft) = form::read(&w, o.model.borrow().edit_base())
                        && let Ok(display) = draft.display.resolve()
                    {
                        w.set_ascii(display.ascii_font.into());
                        w.set_cjk(display.cjk_font.into());
                    }
                    o.edited();
                }
            }
        });
        window.on_search({
            let w = window.as_weak();
            move |query| {
                if let Some(w) = w.upgrade() {
                    w.set_matches(ModelRc::new(VecModel::from(
                        form::search(&query)
                            .into_iter()
                            .map(|(page, key)| SettingsMatch {
                                field: match key {
                                    "font-ascii" => 1,
                                    "font-cjk" => 2,
                                    "font-size" => 3,
                                    "font-baseline" => 4,
                                    "font-width" => 5,
                                    "settings-opacity" => 6,
                                    _ => -1,
                                },
                                page,
                                title: pttstella::i18n::application().loader().get(key).into(),
                            })
                            .collect::<Vec<_>>(),
                    )));
                }
            }
        });
        window.on_focus_result({
            let w = window.as_weak();
            move |field| {
                let w = w.clone();
                Timer::single_shot(Duration::from_millis(16), move || {
                    if let Some(w) = w.upgrade() {
                        w.invoke_focus_field(field);
                    }
                });
            }
        });
        window.on_reset({
            let weak = Rc::downgrade(self);
            let w = window.as_weak();
            move || {
                if let (Some(o), Some(w)) = (weak.upgrade(), w.upgrade()) {
                    let mut draft =
                        form::read(&w, o.model.borrow().edit_base()).unwrap_or_else(|_| o.app());
                    let defaults = AppSettings::default();
                    match w.get_page() {
                        0 => {
                            draft.language = defaults.language;
                            draft.article_mode = defaults.article_mode;
                            draft.article_boundary_navigation =
                                defaults.article_boundary_navigation;
                        }
                        1 => draft.display = defaults.display,
                        2 => {
                            draft.tools_opacity = defaults.tools_opacity;
                            draft.tools_access = defaults.tools_access;
                        }
                        3 => {
                            draft.hover = defaults.hover;
                            draft.auto_expand_images = defaults.auto_expand_images;
                        }
                        _ => return,
                    }
                    form::populate(&w, &draft);
                    o.edited();
                }
            }
        });
        window.on_retry({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.failed.set(false);
                    o.flush();
                }
            }
        });
        window.on_close_settings({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.request_close();
                }
            }
        });
        window.window().on_close_requested({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.request_close();
                }
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
        let modifiers = Cell::new(slint::winit_030::winit::keyboard::ModifiersState::empty());
        window.window().on_winit_window_event({
            let weak = Rc::downgrade(self);
            let window = window.as_weak();
            move |_, event| {
                use slint::winit_030::winit::event::ElementState;
                if let WindowEvent::ModifiersChanged(value) = event {
                    modifiers.set(value.state());
                }
                if let Some(o) = weak.upgrade() {
                    match event {
                        WindowEvent::KeyboardInput {
                            event,
                            is_synthetic: false,
                            ..
                        } if !o.composing.get()
                            && event.state == ElementState::Pressed
                            && !event.repeat =>
                        {
                            if let Some(code) = crate::native_input::text_edit_shortcut(
                                event.physical_key,
                                modifiers.get(),
                            ) && let Some(w) = window.upgrade()
                                && w.invoke_edit_key(code)
                            {
                                return EventResult::PreventDefault;
                            }
                        }
                        WindowEvent::Ime(Ime::Preedit(text, _)) => {
                            o.composing.set(!text.is_empty());
                            if !text.is_empty() {
                                o.timer.stop();
                            }
                        }
                        WindowEvent::Ime(Ime::Commit(_)) => {
                            o.composing.set(false);
                            let weak = Rc::downgrade(&o);
                            Timer::single_shot(Duration::ZERO, move || {
                                if let Some(o) = weak.upgrade() {
                                    o.edited();
                                }
                            });
                        }
                        WindowEvent::Focused(true) => {
                            if let Some(ui) = o.ui.upgrade() {
                                ui.invoke_native_menu_refresh();
                            }
                        }
                        WindowEvent::Focused(false) => {
                            modifiers.set(Default::default());
                            o.composing.set(false);
                        }
                        WindowEvent::Ime(Ime::Disabled) => {
                            o.composing.set(false);
                        }
                        _ => {}
                    }
                }
                EventResult::Propagate
            }
        });
        window.on_manage_account({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.account_handoff.set(true);
                    o.request_close();
                }
            }
        });
        window.on_catalog({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.catalog();
                }
            }
        });
        window.on_family({
            let weak = Rc::downgrade(self);
            let w = window.as_weak();
            move |cjk, name| {
                if let (Some(o), Some(w)) = (weak.upgrade(), w.upgrade()) {
                    if cjk {
                        w.set_cjk(name);
                    } else {
                        w.set_ascii(name);
                    }
                    w.set_preset(8);
                    o.edited();
                }
            }
        });
        window.on_profile({
            let weak = Rc::downgrade(self);
            move |export| {
                if let Some(o) = weak.upgrade() {
                    o.profile(export);
                }
            }
        });
        window.on_font_source({
            let weak = Rc::downgrade(self);
            move |id| {
                if let Some(o) = weak.upgrade()
                    && let Ok(source) = serde_json::from_value::<fonts::Source>(
                        serde_json::Value::String(id.to_string()),
                    )
                    && let Some(ui) = o.ui.upgrade()
                {
                    ui.invoke_link_activated(source.help().url.into());
                }
            }
        });
        Ok(window)
    }
    fn localize(&self, w: &SettingsWindow) {
        let t = |key: &str| pttstella::i18n::application().loader().get(key);
        let mut labels: Vec<String> = [
            "settings-center-title",
            "settings-search-placeholder",
            "settings-search-empty",
            "language-label",
            "language-restart",
            "article-mode",
            "article-mode-help",
            "article-boundary-navigation",
            "font-preset",
            "font-ascii",
            "font-cjk",
            "font-refresh",
            "font-size",
            "font-baseline",
            "font-width",
            "settings-baseline-help",
            "settings-import",
            "settings-export",
            "settings-profile-help",
            "settings-opacity",
            "settings-right-click",
            "settings-show-ball",
            "settings-tool-access-help",
            "hover-position",
            "auto-expand-images",
            "auto-expand-images-help",
            "settings-privacy-description",
            "login",
            "about-version",
            "font-license",
            "settings-defaults",
            "settings-retry",
            "close",
        ]
        .into_iter()
        .map(t)
        .collect();
        labels[28] = env!("CARGO_PKG_VERSION").into();
        labels[29] = format!(
            "{}\n{}\n{}\n{}",
            include_str!("../../../../assets/fonts/NOTICE.txt"),
            include_str!("../../../../assets/fonts/Cascadia-OFL.txt"),
            include_str!("../../../../assets/fonts/NotoSansCJK-OFL.txt"),
            include_str!(concat!(env!("OUT_DIR"), "/third-party-notices.txt"))
        );
        w.set_labels(strings(labels));
        w.set_categories(strings(
            [
                "settings-general",
                "settings-appearance",
                "settings-input",
                "settings-media",
                "settings-account",
                "settings-about",
            ]
            .into_iter()
            .map(t),
        ));
        w.set_presets(strings(
            [
                "font-platform",
                "font-windows-modern",
                "font-windows-classic",
                "font-macos-bbs",
                "font-macos-sans",
                "linux",
                "font-macos-serif",
                "font-bundled",
                "font-custom",
            ]
            .into_iter()
            .map(|key| {
                if key == "linux" {
                    "Linux".into()
                } else {
                    t(key)
                }
            }),
        ));
        w.set_font_sources(ModelRc::new(VecModel::from(
            fonts::Source::ALL
                .into_iter()
                .map(|s| {
                    let help = s.help();
                    crate::FontSource {
                        id: help.id.into(),
                        title: format!("{} · {}", help.label, help.families.join(" / ")).into(),
                    }
                })
                .collect::<Vec<_>>(),
        )));
        w.set_languages(strings(["繁體中文".into(), "English".into()]));
        w.set_modes(strings([t("article-mode-web"), t("article-mode-terminal")]));
        w.set_positions(strings([
            t("hover-follow"),
            t("hover-top-right"),
            t("hover-disabled"),
        ]));
    }
    pub(crate) fn refresh_labels(&self) {
        if let Some(w) = self.window.borrow().as_ref() {
            self.localize(w);
        }
    }
    fn edited(self: &Rc<Self>) {
        self.timer.stop();
        if self.composing.get() {
            return;
        }
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade());
        if let Some(w) = window {
            match form::read(&w, self.model.borrow().edit_base()) {
                Ok(draft) => {
                    w.set_failed(false);
                    self.preview(&w);
                    self.queue(draft, Duration::from_millis(300));
                }
                Err(error) => {
                    self.valid.set(false);
                    w.set_notice(error.into());
                }
            }
        }
    }
    fn preview(&self, w: &SettingsWindow) {
        let result = (|| {
            let draft = form::read(w, self.model.borrow().edit_base())?;
            let display = draft.display.resolve()?;
            let mut preview = self.preview_renderer.borrow_mut();
            if let Some(renderer) = preview.as_mut() {
                renderer.configure(&display).map_err(str::to_owned)?;
            } else {
                *preview = Some(
                    Renderer::new(pttstella::presentation::font_sample()?, &display)
                        .map_err(str::to_owned)?,
                );
            }
            let renderer = preview.as_ref().ok_or("font preview unavailable")?;
            let image = renderer
                .paint(
                    Size {
                        width: 576,
                        height: 192,
                    },
                    false,
                )
                .map_err(str::to_owned)?;
            w.set_preview(slint::Image::from_rgba8_premultiplied(image));
            w.set_font_status(renderer.font_status(&display).into());
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            w.set_font_status(error.into());
        }
    }
    fn request_close(self: &Rc<Self>) {
        if self.composing.get() {
            return;
        }
        self.export_pending.set(false);
        self.closing.set(true);
        self.due.set(Some(Instant::now()));
        self.timer.stop();
        self.flush();
    }
    pub(super) fn hide(&self) {
        self.timer.stop();
        self.export_pending.set(false);
        self.closing.set(false);
        self.composing.set(false);
        if let Some(w) = self.window.borrow().as_ref()
            && let Err(error) = w.hide()
        {
            self.notice(error.to_string());
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_settings_open(false);
            if self.account_handoff.replace(false) && !self.stopped.get() {
                ui.invoke_login_show();
            }
        }
    }
    fn catalog(self: &Rc<Self>) {
        if self.output.busy() || self.inflight.borrow().is_some() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let weak = Rc::downgrade(self);
        self.output
            .work(&ui, fonts::installed, move |_, _, result| {
                if let Some(o) = weak.upgrade() {
                    match result {
                        Ok(catalog) => {
                            let count = catalog.families.len();
                            if let Some(w) = o.window.borrow().as_ref() {
                                w.set_families(strings(catalog.families));
                            }
                            o.notice(catalog.notice.unwrap_or_else(|| {
                                msg!("font-local-count", count = count.to_string())
                            }));
                        }
                        Err(error) => o.notice(error),
                    }
                }
            });
    }
    pub(super) fn profile(self: &Rc<Self>, export: bool) {
        if export && (self.composing.get() || !self.valid.get()) {
            self.notice(msg!("settings-autosave-invalid"));
            return;
        }
        if self.output.busy()
            || self.inflight.borrow().is_some()
            || export && self.pending.borrow().is_some()
        {
            if export {
                self.export_pending.set(true);
                self.flush();
            } else {
                self.notice(msg!("file-busy"));
            }
            return;
        }
        let Some(w) = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
        else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if !self.output.begin_picker(&ui) {
            self.notice(msg!("file-busy"));
            return;
        }
        let path = w
            .window()
            .with_winit_window(|window| {
                let picker = rfd::FileDialog::new()
                    .set_parent(window)
                    .add_filter(msg!("profile-file-type"), &["json"]);
                if export {
                    picker
                        .set_title(msg!("profile-export-title"))
                        .set_file_name("PTTStella.pttstella.json")
                        .save_file()
                } else {
                    picker.set_title(msg!("profile-import-title")).pick_file()
                }
            })
            .flatten();
        self.output.end_picker(&ui);
        let Some(path) = path else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let display = self.app().display;
        let weak = Rc::downgrade(self);
        self.output.work(
            &ui,
            move || {
                if export {
                    if !path.file_name().is_some_and(|name| {
                        name.to_string_lossy()
                            .to_ascii_lowercase()
                            .ends_with(".pttstella.json")
                    }) {
                        return Err(msg!("profile-extension"));
                    }
                    let profile = PortableProfile::new(msg!("profile-default-name"), display)?;
                    pttstella::storage::replace(&path, &profile.encode()?)?;
                    Ok(None)
                } else {
                    let bytes = pttstella::storage::read_bounded(
                        &path,
                        pttstella::settings::MAX_SETTINGS_BYTES,
                    )
                    .map_err(|error| match error {
                        pttstella::storage::ReadError::Io(e) => e.to_string(),
                        pttstella::storage::ReadError::TooLarge => msg!("settings-too-large"),
                    })?
                    .ok_or_else(|| msg!("profile-missing"))?;
                    Ok(Some(PortableProfile::decode(&bytes)?.display))
                }
            },
            move |_, _, result| {
                if let Some(o) = weak.upgrade() {
                    match result {
                        Ok(Some(display)) => {
                            if let Some(w) = o.window.borrow().as_ref() {
                                let mut draft =
                                    o.pending.borrow().clone().unwrap_or_else(|| o.app());
                                draft.display = display;
                                form::populate(w, &draft);
                                o.preview(w);
                                o.queue(draft, Duration::from_millis(300));
                            }
                        }
                        Ok(None) => o.notice(msg!("profile-exported")),
                        Err(error) => o.notice(error),
                    }
                }
            },
        );
    }
}
