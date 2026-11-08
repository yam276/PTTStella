//! Content menu lifetime, source snapshot and typed action dispatch.
mod picture;
pub mod probe;
use crate::{
    MenuEntry, TerminalWindow, output::Output, pointer::SurfaceGeometry, renderer::Renderer,
};
pub use picture::Picture;
use pttstella::{
    connection::{Connection, Phase},
    context_menu::{self, Action, Capabilities, Entry, Flag},
    mouse::input::{Route, Snapshot},
    navigation::{self, Context, Stamp},
    selection::Span,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::{Rc, Weak},
};
#[derive(Clone, PartialEq)]
struct Source {
    stamp: Stamp,
    policy: u64,
    geometry: SurfaceGeometry,
    selected: Option<(Range<usize>, Vec<Span>)>,
    phase: Phase,
    mouse: Snapshot,
    pending: bool,
}
struct Press {
    shift: bool,
    source: Source,
    point: (f32, f32),
    picture: Option<Picture>,
}
struct Menu {
    source: Source,
    link: Option<usize>,
    entries: Vec<Entry>,
    selected: usize,
    picture: Option<Picture>,
}
pub struct ContextMenu {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    renderer: Rc<RefCell<Renderer>>,
    output: Rc<Output>,
    press: RefCell<Option<Press>>,
    menu: RefCell<Option<Menu>>,
    pending: RefCell<Option<Rc<Source>>>,
    job: RefCell<Option<slint::JoinHandle<()>>>,
    closed: Cell<bool>,
    media: RefCell<Weak<crate::media::Media>>,
    trace_left: Cell<u8>,
}
impl ContextMenu {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        renderer: Rc<RefCell<Renderer>>,
        output: Rc<Output>,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            renderer,
            output,
            press: RefCell::new(None),
            menu: RefCell::new(None),
            pending: RefCell::new(None),
            job: RefCell::new(None),
            closed: Cell::new(false),
            media: RefCell::new(Weak::new()),
            trace_left: Cell::new(if std::env::var_os("PTTSTELLA_INLINE_TRACE").is_some() {
                8
            } else {
                0
            }),
        });
        ui.on_context_pointer({
            let owner = owner.clone();
            move |kind, x, y| owner.pointer(kind, (x, y))
        });
        ui.on_context_dismiss({
            let owner = owner.clone();
            move |focus| owner.dismiss(focus)
        });
        ui.on_context_updated({
            let owner = owner.clone();
            move || owner.update()
        });
        ui.on_context_key({
            let owner = owner.clone();
            move |key, repeat| owner.key(key, repeat)
        });
        ui.on_context_choose({
            let owner = owner.clone();
            move |index| owner.choose(index)
        });
        ui.on_inline_context({
            let owner = owner.clone();
            move |kind, index, tag, x, y| owner.picture_pointer(kind, index, tag.as_str(), (x, y))
        });
        owner
    }
    pub fn attach_media(&self, media: &Rc<crate::media::Media>) {
        self.media.replace(Rc::downgrade(media));
    }
    fn source(&self, ui: &TerminalWindow) -> Option<Source> {
        if self.closed.get() || ui.get_context_source_blocked() {
            return None;
        }
        let stamp = self.output.current(ui)?;
        let geometry = self.output.geometry(ui)?;
        let selected = self.output.selection_key();
        let policy = self.output.policy(ui);
        let view = self.connection.updates.borrow();
        (view.stamp() == stamp).then_some(Source {
            stamp,
            policy,
            geometry,
            selected,
            phase: view.phase,
            mouse: view.mouse,
            pending: view.input_pending,
        })
    }
    fn pointer(self: &Rc<Self>, kind: i32, point: (f32, f32)) {
        if kind == 3 {
            self.dismiss(false);
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        if ui.get_context_open() || ui.get_context_pending() {
            self.dismiss(true);
            return;
        }
        let Some(source) = self.source(&ui) else {
            self.press.borrow_mut().take();
            return;
        };
        if kind == 0 {
            let pointer_shift = ui.get_pointer_shift_held();
            let native_shift = ui.invoke_native_shift_held();
            if let Some(left) = self.trace_left.get().checked_sub(1) {
                self.trace_left.set(left);
                eprintln!(
                    "context modifier pointer_shift={pointer_shift} native_shift={native_shift}"
                );
            }
            self.press.replace(Some(Press {
                source,
                point,
                shift: pointer_shift || native_shift,
                picture: None,
            }));
            return;
        }
        if kind == 1 {
            if self
                .press
                .borrow()
                .as_ref()
                .is_some_and(|p| !press_matches(&source, p, point))
            {
                self.press.take();
            }
            return;
        }
        let Some(press) = self.press.borrow_mut().take() else {
            return;
        };
        if kind != 2 || press.picture.is_some() || !press_matches(&source, &press, point) {
            return;
        }
        let editor = self.connection.updates.borrow().site.ptt_features()
            && navigation::article_editor(self.renderer.borrow().frame());
        if !press.shift
            && ui.get_tools_available()
            && ui.get_tools_right_click()
            && source.selected.is_none()
            && source.mouse.route == Route::Local
            && !editor
            && self.link_at(&ui, &source, point).is_none()
            && ui.invoke_tools_show_at(
                ui.get_terminal_origin_x() + point.0,
                ui.get_terminal_origin_y() + point.1,
            )
        {
            return;
        }
        self.output.prepare_context(&ui);
        ui.invoke_reset_keyboard();
        let Some(source) = self.source(&ui) else {
            return;
        };
        let request = Rc::new(source.clone());
        self.pending.replace(Some(request.clone()));
        ui.set_context_x(point.0);
        ui.set_context_y(point.1);
        ui.set_context_pending(true);
        if source.mouse.route == Route::Local {
            self.open(&ui, point);
            return;
        }
        let reply = match self
            .connection
            .request_mouse_revoke(source.stamp.generation)
        {
            Ok(reply) => reply,
            Err(error) => {
                self.dismiss(true);
                ui.set_input_notice(error.into());
                return;
            }
        };
        let weak = Rc::downgrade(self);
        match slint::spawn_local(async move {
            let result = reply.wait().await;
            if let Some(owner) = weak.upgrade()
                && let Some(ui) = owner.ui.upgrade()
            {
                let current = owner
                    .pending
                    .borrow()
                    .as_ref()
                    .is_some_and(|pending| Rc::ptr_eq(pending, &request));
                if current {
                    match result {
                        Ok(()) => owner.open(&ui, point),
                        Err(error) => {
                            owner.dismiss(ui.get_context_focus_restorable());
                            ui.set_input_notice(error.into());
                        }
                    }
                }
            }
        }) {
            Ok(job) => {
                self.job.replace(Some(job));
            }
            Err(e) => {
                self.dismiss(true);
                ui.set_input_notice(e.to_string().into());
            }
        }
    }
    fn capabilities(
        &self,
        ui: &TerminalWindow,
        source: &Source,
        link: Option<usize>,
        picture: Option<&Picture>,
    ) -> Capabilities {
        let view = self.connection.updates.borrow();
        let context = view.context();
        let editor =
            view.site.ptt_features() && navigation::article_editor(self.renderer.borrow().frame());
        let selected = source.selected.is_some();
        let ready = !editor || source.phase == Phase::Connected;
        let mut flags = Capabilities::default();
        for (flag, value) in [
            (Flag::Link, link.is_some()),
            (Flag::Selection, selected),
            (Flag::SelectionReady, selected),
            (
                Flag::RowsReady,
                source
                    .selected
                    .as_ref()
                    .is_some_and(|(rows, _)| rows.len() <= 24),
            ),
            (Flag::Ready, ready),
            (Flag::ExportReady, true),
            (Flag::Editor, editor),
            (Flag::Local, source.mouse.route == Route::Local),
            (Flag::Ptt, view.site.ptt_features()),
            (
                Flag::Reading,
                matches!(context, Context::ArticleReading { .. }),
            ),
            (
                Flag::First,
                matches!(context, Context::ArticleReading { first_line: 1, .. }),
            ),
            (
                Flag::Last,
                matches!(context, Context::ArticleReading { percent: 100, .. }),
            ),
            (
                Flag::Navigate,
                source.phase == Phase::Connected
                    && !source.pending
                    && source.mouse.route == Route::Local,
            ),
            (Flag::Pending, source.pending),
            (Flag::Background, !selected && link.is_none()),
        ] {
            flags.set(flag, value);
        }
        if editor && ui.get_keyboard_read_only() {
            flags.set(Flag::Ready, false);
        }
        let inline = self.media.borrow().upgrade().and_then(|m| m.inline_owner());
        flags.set(
            Flag::Images,
            ui.get_article_active() && inline.as_ref().is_some_and(|i| i.has_images()),
        );
        flags.set(
            Flag::InlineReady,
            inline.as_ref().is_some_and(|i| i.menu_ready()),
        );
        flags.set(Flag::Picture, picture.is_some());
        flags.set(
            Flag::PictureReady,
            picture.is_some_and(|p| p.image.is_some()),
        );
        flags
    }
    fn entries(
        &self,
        ui: &TerminalWindow,
        source: &Source,
        link: Option<usize>,
        picture: Option<&Picture>,
    ) -> Vec<Entry> {
        match context_menu::items(self.capabilities(ui, source, link, picture)) {
            Ok(entries) => entries
                .into_iter()
                .filter(|entry| supported(entry.action))
                .collect(),
            Err(error) => {
                ui.set_input_notice(error.into());
                Vec::new()
            }
        }
    }
    fn link_at(&self, ui: &TerminalWindow, source: &Source, point: (f32, f32)) -> Option<usize> {
        let dpi = ui.window().scale_factor();
        if self.output.policy(ui) != source.policy {
            return None;
        }
        self.output.link_at(ui, (point.0 * dpi, point.1 * dpi))
    }
    fn open(&self, ui: &TerminalWindow, point: (f32, f32)) {
        let expected = self.pending.borrow_mut().take();
        let Some(source) = self.source(ui) else {
            self.dismiss(false);
            return;
        };
        if expected.as_ref().is_none_or(|old| {
            old.stamp != source.stamp
                || old.policy != source.policy
                || old.geometry != source.geometry
                || old.selected != source.selected
                || old.mouse.modes != source.mouse.modes
                || old.mouse.route != source.mouse.route
        }) {
            self.dismiss(false);
            return;
        }
        let link = self.link_at(ui, &source, point);
        let entries = self.entries(ui, &source, link, None);
        self.install_menu(ui, source, link, None, entries);
    }
    fn install_menu(
        &self,
        ui: &TerminalWindow,
        source: Source,
        link: Option<usize>,
        picture: Option<Picture>,
        entries: Vec<Entry>,
    ) {
        if entries.is_empty() {
            self.dismiss(true);
            return;
        }
        let selected = entries.iter().position(|e| e.enabled).unwrap_or(0);
        ui.set_context_entries(ModelRc::new(VecModel::from(
            entries
                .iter()
                .map(|e| MenuEntry {
                    label: label(e.action).into(),
                    enabled: e.enabled,
                })
                .collect::<Vec<_>>(),
        )));
        self.menu.replace(Some(Menu {
            source,
            link,
            entries,
            selected,
            picture,
        }));
        ui.set_context_selected(selected as i32);
        ui.set_context_open(true);
        ui.set_context_pending(false);
        ui.invoke_focus_context();
    }
    fn update(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() {
            return;
        }
        if self.menu.borrow().is_none()
            && self.pending.borrow().is_none()
            && self.press.borrow().is_none()
        {
            return;
        }
        if self.source(&ui).is_none() {
            let active = self.menu.borrow().is_some() || self.pending.borrow().is_some();
            self.dismiss(active && ui.get_context_focus_restorable());
            return;
        }
        let pending = self.pending.borrow().clone();
        if let Some(expected) = pending.as_ref() {
            let valid = self.source(&ui).is_some_and(|s| {
                s.stamp == expected.stamp
                    && s.geometry == expected.geometry
                    && s.selected == expected.selected
                    && s.mouse.modes == expected.mouse.modes
                    && s.mouse.route == expected.mouse.route
            });
            if !valid {
                self.dismiss(ui.get_context_focus_restorable());
            }
            return;
        }
        let valid = self.menu.borrow().as_ref().is_none_or(|menu| {
            self.source(&ui).as_ref() == Some(&menu.source)
                && menu
                    .picture
                    .as_ref()
                    .is_none_or(|p| self.picture_current(p))
        });
        if !valid {
            self.dismiss(ui.get_context_focus_restorable());
        }
    }
    fn dismiss(&self, focus: bool) {
        self.menu.borrow_mut().take();
        self.pending.borrow_mut().take();
        self.press.borrow_mut().take();
        if let Some(job) = self.job.borrow_mut().take() {
            job.abort();
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_context_open(false);
            ui.set_context_pending(false);
            if focus {
                ui.invoke_focus_terminal();
            }
        }
    }
    fn key(&self, key: i32, repeat: bool) {
        if matches!(key, 0 | 1) {
            self.dismiss(true);
            return;
        }
        if key == 6 {
            let index = self.menu.borrow().as_ref().map(|m| m.selected);
            if !repeat && let Some(index) = index {
                self.choose(index as i32);
            }
            return;
        }
        let selected = {
            let mut menu = self.menu.borrow_mut();
            let Some(menu) = menu.as_mut() else { return };
            let enabled = menu
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.enabled)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if let Some(index) = crate::chrome::move_selection(menu.selected, &enabled, key) {
                menu.selected = index;
            }
            menu.selected
        };
        if let Some(ui) = self.ui.upgrade() {
            ui.set_context_selected(selected as i32);
        }
    }
    fn choose(&self, index: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let action = {
            let menu = self.menu.borrow();
            menu.as_ref().and_then(|menu| {
                usize::try_from(index)
                    .ok()
                    .and_then(|i| menu.entries.get(i))
                    .filter(|entry| entry.enabled)
                    .map(|e| (menu.source.clone(), menu.link, *e, menu.picture.clone()))
            })
        };
        let valid = action
            .as_ref()
            .is_some_and(|(source, link, entry, picture)| {
                self.source(&ui).as_ref() == Some(source)
                    && picture.as_ref().is_none_or(|p| self.picture_current(p))
                    && self
                        .entries(&ui, source, *link, picture.as_ref())
                        .contains(entry)
            });
        self.dismiss(true);
        if valid && let Some((_, link, entry, picture)) = action {
            if let Some(picture) = picture {
                self.picture_action(&ui, picture, entry.action);
            } else if entry.action == Action::Inline {
                if let Some(inline) = self.media.borrow().upgrade().and_then(|m| m.inline_owner()) {
                    inline.expand_all();
                }
            } else {
                self.output.context_action(entry.action, link);
            }
        }
    }
    pub fn finish(&self) {
        self.closed.set(true);
        self.dismiss(false);
    }
}
fn supported(action: Action) -> bool {
    matches!(
        action,
        Action::Copy
            | Action::CopyPicture
            | Action::SavePicture
            | Action::Inline
            | Action::CopyAnsi
            | Action::CopyLink
            | Action::OpenLink
            | Action::SelectLink
            | Action::SelectAll
            | Action::ExportRows
            | Action::ExportPage
            | Action::CopyImageRows
            | Action::CopyImagePage
            | Action::Back
            | Action::Previous
            | Action::Next
            | Action::Paste
            | Action::EditorColor
    )
}
pub(super) fn label(action: Action) -> String {
    use Action as A;
    match action {
        A::Copy => pttstella::msg!("context-copy"),
        A::CopyPicture => pttstella::msg!("context-copy-picture"),
        A::SavePicture => pttstella::msg!("context-save-picture"),
        A::Inline => pttstella::msg!("context-inline"),
        A::CopyAnsi => pttstella::msg!("context-copy-ansi"),
        A::CopyLink => pttstella::msg!("context-copy-link"),
        A::OpenLink => pttstella::msg!("context-open-link-external"),
        A::SelectLink => pttstella::msg!("context-select-link"),
        A::SelectAll => pttstella::msg!("context-select-all"),
        A::ExportRows => pttstella::msg!("context-export-rows"),
        A::ExportPage => pttstella::msg!("context-export-page"),
        A::Back => pttstella::msg!("context-back"),
        A::Previous => pttstella::msg!("context-previous"),
        A::Next => pttstella::msg!("context-next"),
        A::EditorColor => pttstella::msg!("editor-color-title"),
        A::Paste => pttstella::msg!("menu-paste"),
        A::CopyImageRows => pttstella::msg!("copy-rows-png"),
        A::CopyImagePage => pttstella::msg!("copy-png"),
        _ => String::new(),
    }
}
fn press_matches(source: &Source, press: &Press, point: (f32, f32)) -> bool {
    source.stamp == press.source.stamp
        && source.policy == press.source.policy
        && source.geometry == press.source.geometry
        && source.selected == press.source.selected
        && source.mouse.modes == press.source.mouse.modes
        && source.mouse.route == press.source.mouse.route
        && (point.0 - press.point.0).hypot(point.1 - press.point.1) < 3.
}
