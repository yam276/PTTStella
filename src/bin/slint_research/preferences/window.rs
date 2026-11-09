use super::*;
use crate::{
    SettingsMatch, SettingsWindow,
    renderer::{Renderer, Size},
};
use pttstella::settings::PortableProfile;
use slint::{
    Model, ModelRc, VecModel,
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
            u.get_app_closing()
                || u.get_manager_open()
                || u.get_native_composing()
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
        window.on_release_check({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.check_release();
                }
            }
        });
        window.on_release_open({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.open_release(false);
                }
            }
        });
        window.on_repository_open({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.open_release(true);
                }
            }
        });
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
        window.set_profile_name(msg!("profile-default-name").into());
        window.on_search({
            let owner = Rc::downgrade(self);
            let w = window.as_weak();
            move |query| {
                if let (Some(owner), Some(w)) = (owner.upgrade(), w.upgrade()) {
                    owner.search_focus.stop();
                    let bounded = pttstella::settings::catalog::bounded_query(&query);
                    if bounded != w.get_query().as_str() {
                        w.set_query(bounded.clone().into());
                    }
                    let found = owner
                        .search_index
                        .borrow()
                        .as_ref()
                        .map(|i| i.search(&bounded))
                        .unwrap_or_default();
                    w.set_matches(ModelRc::new(VecModel::from(
                        found
                            .into_iter()
                            .filter_map(|entry| {
                                form::destination(entry)
                                    .filter(|(_, field)| match *field {
                                        18 => w.get_sites_available(),
                                        26 => w.get_filters_available(),
                                        21 => w.get_runtime_available().row_data(1) == Some(true),
                                        22 => w.get_runtime_available().row_data(2) == Some(true),
                                        23 => w.get_runtime_available().row_data(3) == Some(true),
                                        _ => true,
                                    })
                                    .map(|(page, field)| SettingsMatch {
                                        field,
                                        page,
                                        title: pttstella::i18n::application()
                                            .loader()
                                            .get(entry.2)
                                            .into(),
                                    })
                            })
                            .collect::<Vec<_>>(),
                    )));
                }
            }
        });
        window.on_cancel_result_focus({
            let owner = Rc::downgrade(self);
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.search_focus.stop();
                }
            }
        });
        window.on_focus_result({
            let owner = Rc::downgrade(self);
            let w = window.as_weak();
            move |field| {
                if let (Some(owner), Some(w)) = (owner.upgrade(), w.upgrade()) {
                    let expected_page = w.get_page();
                    let w = w.as_weak();
                    let owner_weak = Rc::downgrade(&owner);
                    owner.search_focus.start(
                        TimerMode::SingleShot,
                        Duration::from_millis(16),
                        move || {
                            if let (Some(owner), Some(w)) = (owner_weak.upgrade(), w.upgrade())
                                && !owner.stopped.get()
                                && !owner.closing.get()
                                && !owner.composing.get()
                                && w.get_query().is_empty()
                                && w.get_page() == expected_page
                                && owner
                                    .ui
                                    .upgrade()
                                    .is_some_and(|u| u.get_settings_open() && !u.get_app_closing())
                            {
                                w.invoke_focus_field(field);
                            }
                        },
                    );
                }
            }
        });
        window.on_runtime_action({
            let owner = Rc::downgrade(self);
            move |code| {
                if let Some(owner) = owner.upgrade()
                    && !owner.composing.get()
                    && owner
                        .ui
                        .upgrade()
                        .is_some_and(|u| u.get_settings_open() && !u.get_app_closing())
                {
                    let handoff = match code {
                        0 => Handoff::Fit,
                        1 => Handoff::Vim,
                        2 => Handoff::Mouse,
                        3 => Handoff::KeepAlive,
                        _ => return,
                    };
                    owner.handoff.set(Some(handoff));
                    owner.request_close();
                }
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
        window.on_manage_sites({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.handoff.set(Some(Handoff::Sites));
                    o.request_close();
                }
            }
        });
        window.on_manage_filters({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.handoff.set(Some(Handoff::Filters));
                    o.request_close();
                }
            }
        });
        window.on_manage_account({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(o) = weak.upgrade() {
                    o.handoff.set(Some(Handoff::Account));
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
    pub(super) fn refresh_runtime(&self, w: &SettingsWindow) {
        if let Some(ui) = self.ui.upgrade() {
            w.set_runtime_labels(strings([
                msg!("zoom-fit"),
                ui.get_vim_label().to_string(),
                ui.get_station_mouse_label().to_string(),
                ui.get_keep_alive_label().to_string(),
            ]));
            w.set_runtime_states(ModelRc::new(VecModel::from(vec![
                false,
                ui.get_vim_enabled(),
                ui.get_station_mouse_enabled(),
                ui.get_keep_alive_enabled(),
            ])));
            let unblocked = !ui.get_app_closing()
                && !ui.get_editor_open()
                && !ui.get_station_color_open()
                && !ui.get_login_open()
                && !ui.get_paste_open()
                && !ui.get_switching_station();
            let ptt = ui.get_keep_alive_available() && unblocked;
            w.set_runtime_available(ModelRc::new(VecModel::from(vec![
                true,
                ptt,
                ui.get_station_mouse_available() && unblocked,
                ptt,
            ])));
            if !w.get_query().is_empty() {
                w.invoke_search(w.get_query());
            }
        }
    }
    fn localize(&self, w: &SettingsWindow) {
        w.set_filters_label(msg!("filters-title").into());
        w.set_filters_available(self.ui.upgrade().is_some_and(|u| u.get_filters_available()));
        w.set_sites_label(msg!("sites-title").into());
        w.set_sites_available(self.ui.upgrade().is_some_and(|u| u.get_sites_available()));
        let t = |key: &str| pttstella::i18n::application().loader().get(key);
        match pttstella::settings::catalog::Index::new(t) {
            Ok(index) => {
                self.search_index.replace(Some(index));
            }
            Err(error) => {
                self.search_index.borrow_mut().take();
                self.notice(error);
            }
        }
        self.refresh_runtime(w);
        self.refresh_releases(w);
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
            "profile-name",
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
                "sites-title",
                "filters-title",
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
            w.invoke_search(w.get_query());
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
        self.search_focus.stop();
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
            if let Some(handoff) = self.handoff.replace(None)
                && !self.stopped.get()
            {
                match handoff {
                    Handoff::Account => ui.invoke_login_show(),
                    Handoff::Sites => ui.invoke_sites_show(),
                    Handoff::Filters => ui.invoke_filters_show(),
                    Handoff::Fit => {
                        ui.set_zoom(1.);
                        ui.invoke_focus_terminal();
                    }
                    Handoff::Vim => {
                        ui.invoke_keyboard_updated();
                        ui.invoke_toggle_vim();
                    }
                    Handoff::Mouse => ui.invoke_toggle_station_mouse(),
                    Handoff::KeepAlive => ui.invoke_toggle_keep_alive(),
                }
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
    pub(super) fn imported(self: &Rc<Self>, profile: PortableProfile) {
        if let Some(w) = self.window.borrow().as_ref() {
            let mut draft = self.pending.borrow().clone().unwrap_or_else(|| self.app());
            draft.display = profile.display;
            w.set_profile_name(profile.name.into());
            form::populate(w, &draft);
            self.preview(w);
            self.queue(draft, Duration::from_millis(300));
            self.inline_notice(msg!("slint-profile-imported"));
        }
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
        let profile = if export {
            match PortableProfile::new(w.get_profile_name().to_string(), self.app().display) {
                Ok(profile) => Some(profile),
                Err(error) => {
                    self.notice(error);
                    return;
                }
            }
        } else {
            None
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
                    let profile = profile.ok_or_else(|| msg!("profile-missing"))?;
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
                    Ok(Some(PortableProfile::decode(&bytes)?))
                }
            },
            move |_, _, result| {
                if let Some(o) = weak.upgrade() {
                    match result {
                        Ok(Some(profile)) => o.imported(profile),
                        Ok(None) => {
                            o.inline_notice(msg!("profile-exported"));
                            if let Some(ui) = o.ui.upgrade() {
                                crate::notice::success(&ui, msg!("profile-exported"));
                            }
                        }
                        Err(error) => o.notice(error),
                    }
                }
            },
        );
    }
}
