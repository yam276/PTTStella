//! Native tool chrome; terminal, output, keyboard and session owners admit actions.
pub mod probe;
use crate::{EditorState, SelectionRect, TerminalWindow, ToolItem, output::Output};
use pttstella::floating_tools::{
    State,
    geometry::{Point, Rect, Viewport},
};
use pttstella::{
    connection::{Connection, Phase},
    context_menu::Action,
    mouse::input::Route,
    navigation::{self, Intent},
    settings::{ToolAccess, ToolOpacity},
};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, PartialEq)]
enum Command {
    Content(Action),
    Reconnect,
    Disconnect,
    Login,
    CancelLogin,
    Sites,
    Reveal,
    ReplyNavigation,
    Detail,
    Destination,
    StopDestination,
    Drafts,
}
struct PressedAction {
    index: i32,
    category: usize,
    command: Command,
    stamp: pttstella::navigation::Stamp,
    viewport: Viewport,
}
pub struct Tools {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    state: RefCell<State>,
    pressed_action: RefCell<Option<PressedAction>>,
    timer: Timer,
    generation: Cell<u64>,
    closed: Cell<bool>,
}
impl Tools {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        output: Rc<Output>,
        opacity: ToolOpacity,
        access: ToolAccess,
    ) -> Rc<Self> {
        let viewport = Viewport::new(
            ui.window().size().width as f32 / ui.window().scale_factor(),
            ui.window().size().height as f32 / ui.window().scale_factor(),
        )
        .unwrap_or(Viewport {
            width: 1000.,
            height: 690.,
        });
        let generation = connection.updates.borrow().generation;
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            output,
            state: RefCell::new(State::new(viewport)),
            pressed_action: RefCell::new(None),
            timer: Timer::default(),
            generation: Cell::new(generation),
            closed: Cell::new(false),
        });
        ui.set_tools_available(true);
        ui.set_tools_opacity(opacity.percent() as f32 / 100.);
        ui.set_tools_visible(access.show_floating_button);
        ui.set_tools_right_click(access.right_click_menu);
        ui.on_tools_pointer({
            let owner = owner.clone();
            move |kind, x, y| owner.pointer(kind, Point { x, y })
        });
        ui.on_tools_toggle({
            let owner = owner.clone();
            move || owner.toggle()
        });
        ui.on_tools_dismiss({
            let owner = owner.clone();
            move |focus| owner.dismiss(focus)
        });
        ui.on_tools_category_choose({
            let owner = owner.clone();
            move |index| owner.category(index)
        });
        ui.on_tools_action_pointer({
            let owner = owner.clone();
            move |kind, index| owner.action_pointer(kind, index)
        });
        ui.on_tools_action_choose({
            let owner = owner.clone();
            move |index| owner.action(index)
        });
        ui.on_tools_key({
            let owner = owner.clone();
            move |key, repeat| owner.key(key, repeat)
        });
        ui.on_tools_updated({
            let owner = owner.clone();
            move || owner.update()
        });
        ui.on_tools_cancel_gesture({
            let owner = owner.clone();
            move || {
                owner.timer.stop();
                owner.pressed_action.borrow_mut().take();
                owner.state.borrow_mut().stop_motion();
                owner.publish();
            }
        });
        ui.on_tools_resized({
            let owner = owner.clone();
            move || owner.resize()
        });
        ui.on_tools_show_at({
            let owner = owner.clone();
            move |x, y| {
                let Some(ui) = owner.ui.upgrade() else {
                    return false;
                };
                if owner.closed.get()
                    || ui.get_tools_source_blocked()
                    || owner.state.borrow().dragging()
                    || !x.is_finite()
                    || !y.is_finite()
                {
                    return false;
                }
                owner.prepare(&ui);
                owner.timer.stop();
                owner.state.borrow_mut().show_at(Point { x, y });
                owner.publish();
                ui.invoke_focus_tools();
                true
            }
        });
        owner.publish();
        owner
    }
    fn prepare(&self, ui: &TerminalWindow) {
        ui.invoke_context_dismiss(false);
        self.output.prepare_context(ui);
        ui.invoke_reset_keyboard();
        let generation = self.connection.updates.borrow().generation;
        self.connection.revoke_mouse(generation);
    }
    fn toggle(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || ui.get_tools_source_blocked() {
            return;
        }
        self.prepare(&ui);
        self.timer.stop();
        self.state.borrow_mut().toggle();
        self.publish();
        if self.state.borrow().open {
            ui.invoke_focus_tools();
        } else if ui.get_context_focus_restorable() {
            ui.invoke_focus_terminal();
        }
    }
    fn pointer(self: &Rc<Self>, kind: i32, point: Point) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || ui.get_tools_source_blocked() {
            self.dismiss(false);
            return;
        }
        if kind == 0 {
            self.prepare(&ui);
            self.timer.stop();
        }
        if !point.x.is_finite() || !point.y.is_finite() {
            self.dismiss(false);
            return;
        }
        match kind {
            0 => self.state.borrow_mut().down(point, Instant::now()),
            1 => {
                if self.state.borrow().dragging() {
                    if ui.get_native_pointer_buttons() & 1 == 0 {
                        // The backend can flush the final move after the native
                        // release updates the button mask, immediately before
                        // delivering the toolkit release to the grab owner.
                        let weak = Rc::downgrade(self);
                        self.timer
                            .start(TimerMode::SingleShot, Duration::ZERO, move || {
                                if let Some(owner) = weak.upgrade()
                                    && let Some(ui) = owner.ui.upgrade()
                                    && owner.state.borrow().dragging()
                                    && ui.get_native_pointer_buttons() & 1 == 0
                                {
                                    owner.dismiss(false);
                                }
                            });
                        return;
                    }
                    self.state.borrow_mut().motion(point, Instant::now(), true);
                }
            }
            2 => {
                self.state
                    .borrow_mut()
                    .up(point, Instant::now(), ui.get_tools_reduced_motion());
            }
            3 => self.state.borrow_mut().cancel(),
            _ => return,
        }
        self.publish();
        if kind == 2 {
            if self.state.borrow().open {
                ui.invoke_focus_tools();
            } else if ui.get_context_focus_restorable() {
                ui.invoke_focus_terminal();
            }
            self.schedule();
        }
    }
    fn schedule(self: &Rc<Self>) {
        if self.closed.get() || !self.state.borrow().animating() {
            self.timer.stop();
            return;
        }
        let weak = Rc::downgrade(self);
        self.timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(16),
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.state.borrow_mut().tick(Instant::now());
                    owner.publish();
                    owner.schedule();
                }
            },
        );
    }
    fn labels() -> [String; 5] {
        [
            pttstella::msg!("tools-navigate"),
            pttstella::msg!("tools-bookmarks"),
            pttstella::msg!("tools-capture"),
            pttstella::msg!("tools-session"),
            pttstella::msg!("settings-center-title"),
        ]
    }
    fn commands(&self, ui: &TerminalWindow, category: usize) -> Vec<(Command, String, bool)> {
        let ready = self.output.current(ui).is_some() && !ui.get_output_busy();
        let view = self.connection.updates.borrow();
        let context = view.context();
        let navigate = ready
            && view.phase == Phase::Connected
            && !view.input_pending
            && !view.login_pending
            && view.mouse.route == Route::Local;
        let mut result = Vec::new();
        match category {
            0 => {
                if ui.get_filter_count() > 0 {
                    result.extend([
                        (
                            Command::Reveal,
                            ui.get_filter_label().into(),
                            ready && ui.get_filter_ready(),
                        ),
                        (Command::Detail, ui.get_filter_status().into(), false),
                    ]);
                }
                result.extend([
                    (
                        Command::Drafts,
                        pttstella::msg!("draft-open"),
                        ui.get_editor_mounted(),
                    ),
                    (
                        Command::Destination,
                        pttstella::msg!("destination-open"),
                        ui.get_destination_available() && view.site.ptt_features(),
                    ),
                ]);
                if ui.get_destination_active() {
                    result.push((
                        Command::StopDestination,
                        pttstella::msg!("destination-stop"),
                        true,
                    ));
                }
                if matches!(context, navigation::Context::ArticleReading { .. }) {
                    for (action, intent, label) in [
                        (Action::Back, Intent::Back, pttstella::msg!("browse-back")),
                        (
                            Action::Previous,
                            Intent::PreviousPage,
                            pttstella::msg!("browse-previous"),
                        ),
                        (
                            Action::Next,
                            Intent::NextPage,
                            pttstella::msg!("browse-next"),
                        ),
                    ] {
                        let enabled = navigate
                            && (ui.get_article_active() && action != Action::Back
                                || navigation::plan(&context, intent)
                                    .is_ok_and(|bytes| !bytes.is_empty()));
                        result.push((Command::Content(action), label, enabled));
                    }
                    if ui.get_reply_navigation_available() {
                        result.push((
                            Command::ReplyNavigation,
                            pttstella::msg!("reply-navigation-title"),
                            ready && ui.get_reply_navigation_ready(),
                        ));
                    }
                }
                if view.phase == Phase::Connected && view.mouse.route == Route::Local {
                    let hint = if view.input_pending {
                        pttstella::msg!("navigation-wait")
                    } else {
                        match context {
                            navigation::Context::ArticleList { .. } => {
                                pttstella::msg!("article-list-hint")
                            }
                            navigation::Context::ArticleReading { .. } => {
                                pttstella::msg!("article-reading-hint")
                            }
                            _ => String::new(),
                        }
                    };
                    result.push((Command::Detail, hint, false));
                }
            }
            2 => {
                for (action, label) in [
                    (Action::SelectAll, pttstella::msg!("select-all")),
                    (Action::ExportPage, pttstella::msg!("export-png")),
                    (Action::ExportRows, pttstella::msg!("export-rows")),
                    (Action::CopyImagePage, pttstella::msg!("copy-png")),
                    (Action::CopyImageRows, pttstella::msg!("copy-rows-png")),
                ] {
                    let rows = matches!(action, Action::ExportRows | Action::CopyImageRows);
                    result.push((
                        Command::Content(action),
                        label,
                        ready && (!rows || ui.get_selection_rows_ready()),
                    ));
                }
            }
            3 => {
                result.extend([
                    (
                        Command::Reconnect,
                        pttstella::msg!("connect"),
                        ui.get_can_reconnect(),
                    ),
                    (
                        Command::Disconnect,
                        pttstella::msg!("disconnect"),
                        matches!(view.phase, Phase::Connecting | Phase::Connected),
                    ),
                ]);
                if ui.get_sites_available() {
                    result.push((Command::Sites, pttstella::msg!("sites-title"), true));
                }
                result.extend([
                    (
                        Command::Detail,
                        crate::sites_manager::describe(&view.site),
                        false,
                    ),
                    (
                        Command::Login,
                        pttstella::msg!("login"),
                        ui.get_login_available() && !ui.get_login_busy(),
                    ),
                ]);
                if ui.get_login_pending() {
                    result.push((
                        Command::CancelLogin,
                        ui.get_login_cancel_attempt_label().to_string(),
                        true,
                    ));
                }
            }
            _ => {}
        }
        result.retain(|(_, label, _)| !label.is_empty());
        result
    }
    fn category_available(ui: &TerminalWindow, index: usize) -> bool {
        match index {
            0 | 2 | 3 => true,
            1 => ui.get_bookmarks_available() && !ui.get_bookmarks_capturing(),
            4 => ui.get_settings_available(),
            _ => false,
        }
    }
    fn publish(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let (viewport, point, open, busy, dragging, category, selected) = {
            let state = self.state.borrow();
            (
                state.viewport,
                state.point,
                state.open,
                state.busy(),
                state.dragging(),
                state.category,
                state.selected,
            )
        };
        let layout = viewport.layout(point);
        ui.set_tools_x(layout.ball.x);
        ui.set_tools_y(layout.ball.y);
        ui.set_tools_scale(layout.scale);
        ui.set_tools_open(open);
        ui.set_tools_busy(busy);
        ui.set_tools_dragging(dragging);
        ui.set_tools_category(category.map_or(-1, |index| index as i32));
        ui.set_tools_selected(selected as i32);
        ui.set_tools_ball_label(
            if category.is_some() {
                pttstella::msg!("tools-back")
            } else {
                pttstella::msg!("tools-toggle")
            }
            .into(),
        );
        if !open {
            return;
        }
        let labels = Self::labels();
        ui.set_tools_compact(layout.compact);
        ui.set_tools_panel(SelectionRect {
            x: layout.panel.x,
            y: layout.panel.y,
            width: layout.panel.width,
            height: layout.panel.height,
        });
        ui.set_tools_grid_height(
            layout
                .petals
                .iter()
                .map(|r| r.y + r.height - layout.panel.y + 8. * layout.scale)
                .fold(0., f32::max),
        );
        ui.set_tools_categories(ModelRc::new(VecModel::from(
            layout
                .petals
                .iter()
                .enumerate()
                .map(|(index, rect)| {
                    item(
                        index,
                        &labels[index],
                        Self::category_available(&ui, index),
                        *rect,
                    )
                })
                .collect::<Vec<_>>(),
        )));
        if let Some(category) = category {
            let actions = self.commands(&ui, category);
            if !actions
                .get(selected)
                .is_some_and(|(_, _, enabled)| *enabled)
            {
                let selected = actions
                    .iter()
                    .position(|(_, _, enabled)| *enabled)
                    .unwrap_or(0);
                self.state.borrow_mut().selected = selected;
                ui.set_tools_selected(selected as i32);
            }
            let scale = layout.scale;
            let gap = 8. * scale;
            let available = layout.panel.width - 28.;
            let mut x = 0.;
            let mut y = 0.;
            let mut row_height = 0_f32;
            let items = actions
                .iter()
                .enumerate()
                .map(|(index, (command, label, enabled))| {
                    let detail = *command == Command::Detail;
                    let text = label
                        .chars()
                        .map(|ch| if ch.is_ascii() { 7. } else { 13. })
                        .sum::<f32>();
                    let icon = if *command == Command::Sites { 24. } else { 0. };
                    let natural_width = if detail {
                        text * (12. / 13.) + 4.
                    } else {
                        text + 22. + icon
                    };
                    let width = (natural_width * scale).min(available);
                    if x > 0. && (x + width > available || *command == Command::Sites) {
                        x = 0.;
                        y += row_height + gap;
                        row_height = 0.;
                    }
                    let height = if detail {
                        ((natural_width * scale / width).ceil() * 16. + 2.).max(34.) * scale
                    } else {
                        34. * scale
                    };
                    let rect = Rect {
                        x,
                        y,
                        width,
                        height,
                    };
                    x += width + gap;
                    row_height = row_height.max(height);
                    let mut item = item(index, label, *enabled, rect);
                    item.detail = detail;
                    item.icon = if *command == Command::Sites { 6 } else { -1 };
                    item
                })
                .collect::<Vec<_>>();
            let content_height = y + row_height;
            ui.set_tools_action_height(content_height);
            ui.set_tools_panel(SelectionRect {
                x: layout.panel.x,
                y: layout.panel.y,
                width: layout.panel.width,
                height: layout.panel.height.min(28. + 50. * scale + content_height),
            });
            ui.set_tools_actions(ModelRc::new(VecModel::from(items)));
        }
    }
    fn category(&self, index: i32) {
        self.pressed_action.borrow_mut().take();
        let Some(ui) = self.ui.upgrade() else { return };
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if self.closed.get()
            || ui.get_tools_source_blocked()
            || !Self::category_available(&ui, index)
            || !self.state.borrow().open
        {
            return;
        }
        // These petals are dialog shortcuts in Dioxus.
        if matches!(index, 1 | 4) {
            self.dismiss(true);
            if index == 1 {
                ui.invoke_bookmarks_show();
            } else {
                ui.invoke_settings_show();
            }
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.category = Some(index);
            state.selected = 0;
        }
        self.publish();
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_focus_tools();
        }
    }
    fn action_pointer(&self, kind: i32, index: i32) {
        if kind == 3 {
            self.pressed_action.borrow_mut().take();
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let Some(category) = self.state.borrow().category else {
            return;
        };
        let command = usize::try_from(index)
            .ok()
            .and_then(|i| self.commands(&ui, category).get(i).cloned())
            .filter(|(_, _, enabled)| *enabled)
            .map(|(command, _, _)| command);
        if kind == 0 {
            let source = command.and_then(|command| {
                let stamp = if matches!(
                    command,
                    Command::Content(_) | Command::Reveal | Command::ReplyNavigation
                ) {
                    self.output.current(&ui)?
                } else {
                    self.connection.updates.borrow().stamp()
                };
                Some(PressedAction {
                    index,
                    category,
                    command,
                    stamp,
                    viewport: self.state.borrow().viewport,
                })
            });
            self.pressed_action.replace(source);
        } else if kind == 2 {
            let press = self.pressed_action.borrow_mut().take();
            if press.is_some_and(|press| {
                press.index == index
                    && press.category == category
                    && Some(press.command) == command
                    && press.stamp == self.connection.updates.borrow().stamp()
                    && press.viewport == self.state.borrow().viewport
            }) {
                self.action(index);
            }
        }
    }
    fn action(&self, index: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || ui.get_tools_source_blocked() || !self.state.borrow().open {
            return;
        }
        let Some(category) = self.state.borrow().category else {
            return;
        };
        let command = usize::try_from(index)
            .ok()
            .and_then(|index| self.commands(&ui, category).get(index).cloned())
            .filter(|(_, _, enabled)| *enabled)
            .map(|(command, _, _)| command);
        let Some(command) = command else { return };
        if command == Command::Content(Action::SelectAll) {
            self.output.context_action(Action::SelectAll, None);
            self.publish();
            return;
        }
        self.dismiss(true);
        match command {
            Command::Content(action) => self.output.context_action(action, None),
            Command::Reconnect => ui.invoke_reconnect(),
            Command::Disconnect => {
                if let Err(error) = self.connection.disconnect() {
                    crate::notice::error(&ui, error);
                }
            }
            Command::Login => ui.invoke_login_show(),
            Command::CancelLogin => ui.invoke_login_cancel(),
            Command::Sites => ui.invoke_sites_show(),
            Command::Reveal => ui.invoke_filter_reveal(),
            Command::ReplyNavigation => ui.invoke_reply_navigation_action(0),
            Command::Detail => {}
            Command::Destination => ui.invoke_destination_show(),
            Command::StopDestination => ui.invoke_destination_stop(),
            Command::Drafts => ui.global::<EditorState>().invoke_action(0),
        }
    }
    fn key(&self, key: i32, repeat: bool) {
        if key == 0 {
            self.dismiss(true);
            return;
        }
        if key == 7 {
            self.pressed_action.borrow_mut().take();
            let previous = self.state.borrow().category;
            if let Some(category) = previous {
                {
                    let mut state = self.state.borrow_mut();
                    state.category = None;
                    state.selected = category;
                }
                self.publish();
            }
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let category = self.state.borrow().category;
        if key == 6 {
            if !repeat {
                let selected = self.state.borrow().selected as i32;
                if category.is_some() {
                    self.action(selected);
                } else {
                    self.category(selected);
                }
            }
            return;
        }
        let enabled = if let Some(category) = category {
            self.commands(&ui, category)
                .iter()
                .enumerate()
                .filter_map(|(index, (_, _, enabled))| enabled.then_some(index))
                .collect::<Vec<_>>()
        } else {
            (0..5)
                .filter(|index| Self::category_available(&ui, *index))
                .collect()
        };
        let current = self.state.borrow().selected;
        if let Some(index) = crate::chrome::move_selection(current, &enabled, key) {
            self.state.borrow_mut().selected = index;
        }
        self.publish();
    }
    fn update(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() {
            return;
        }
        let generation = self.connection.updates.borrow().generation;
        if self.generation.replace(generation) != generation || ui.get_tools_source_blocked() {
            self.dismiss(false);
        } else if self.state.borrow().open {
            self.publish();
        }
    }
    fn resize(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let dpi = ui.window().scale_factor();
        self.pressed_action.borrow_mut().take();
        if let Some(viewport) = Viewport::new(
            ui.window().size().width as f32 / dpi,
            ui.window().size().height as f32 / dpi,
        ) {
            self.timer.stop();
            self.state.borrow_mut().resize(viewport);
            self.publish();
        }
    }
    fn dismiss(&self, focus: bool) {
        self.timer.stop();
        self.pressed_action.borrow_mut().take();
        self.state.borrow_mut().cancel();
        self.publish();
        if focus
            && let Some(ui) = self.ui.upgrade()
            && ui.get_context_focus_restorable()
        {
            ui.invoke_focus_terminal();
        }
    }
    pub fn finish(&self) {
        self.closed.set(true);
        self.dismiss(false);
    }
}
fn item(id: usize, label: &str, enabled: bool, rect: Rect) -> ToolItem {
    ToolItem {
        id: id as i32,
        label: label.into(),
        detail: false,
        icon: -1,
        enabled,
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}
