//! Native keyboard and IME events feed the existing session writer.
use crate::{TerminalWindow, clipboard::Clipboard, paste::Paste, session::FrameId};
use pttstella::{
    connection::{Connection, Phase, View},
    encoding::TextEncoding,
    input,
    vim::{self, Effect, Keyboard},
};
use slint::{
    ComponentHandle,
    winit_030::{
        EventResult, WinitWindowAccessor,
        winit::{
            event::{ElementState, Ime, KeyEvent, MouseButton, WindowEvent},
            keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
        },
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct Input {
    keyboard: Keyboard,
    modifiers: ModifiersState,
    window_focused: bool,
    occluded: bool,
    pressed_buttons: u8,
    composition: Option<Composition>,
    generation: Option<u64>,
    guidance: pttstella::login::Guidance,
    ime_allowed: Option<bool>,
}

struct Composition {
    generation: Option<u64>,
    active: bool,
}

impl Input {
    fn observe(&mut self, view: &View) -> pttstella::navigation::Context {
        let context = view.context();
        if self.generation != Some(view.generation) {
            self.composition = None;
            self.keyboard.new_session();
            self.generation = Some(view.generation);
            self.guidance = pttstella::login::Guidance::Idle;
        }
        if view.phase != Phase::Connected {
            self.reset();
        }
        if !view.site.ptt_features() && self.keyboard.status(&context).enabled {
            self.keyboard.toggle();
        }
        if self.guidance != view.login_guidance && view.login_guidance.needs_input() {
            self.keyboard.use_native_input();
        }
        self.guidance = view.login_guidance;
        self.keyboard.context(&context);
        context
    }

    fn reset(&mut self) {
        self.composition = None;
        self.keyboard.reset();
    }

    fn preedit(&mut self, generation: Option<u64>, active: bool) {
        if active
            && self
                .composition
                .as_ref()
                .is_none_or(|previous| !previous.active)
        {
            self.keyboard.composition_start();
            self.composition = Some(Composition { generation, active });
        } else if let Some(composition) = &mut self.composition {
            // Winit clears preedit immediately before Commit. Keep the lease
            // until Commit, the next edit, or a focus/IME reset.
            composition.active = active;
        }
    }

    fn take_commit(&mut self, generation: u64) -> bool {
        let Some(composition) = self.composition.take() else {
            return false;
        };
        let accepted =
            composition.generation == Some(generation) && self.keyboard.accept_text(true);
        self.keyboard.composition_end();
        accepted
    }

    fn ime_disabled(&mut self) {
        self.composition = None;
        // Disabling IME for Vim normal mode is our own mode transition. Keep
        // the physical-key hold until keyup so native-once cannot repeat as Vim.
        if self.ime_allowed != Some(false) {
            self.keyboard.reset();
        }
    }
}

pub fn mount(
    ui: &TerminalWindow,
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    clipboard: Clipboard,
) -> Rc<Paste> {
    mount_window(ui, connection, presented, clipboard, None)
}
/// The article and terminal share this observer and the same Keyboard reducer.
pub fn mount_article(
    ui: &TerminalWindow,
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    clipboard: Clipboard,
    article: Rc<crate::article::session::ArticleSession>,
) -> Rc<Paste> {
    mount_window(ui, connection, presented, clipboard, Some(article))
}
fn mount_window(
    ui: &TerminalWindow,
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    clipboard: Clipboard,
    article: Option<Rc<crate::article::session::ArticleSession>>,
) -> Rc<Paste> {
    ui.set_terminal_input(true);
    let input = Rc::new(RefCell::new(Input::default()));
    let paste = Paste::mount(
        ui,
        connection.clone(),
        presented.clone(),
        clipboard,
        article.clone(),
    );
    ui.on_paste_request({
        let weak = ui.as_weak();
        let paste = paste.clone();
        move || {
            if let Some(ui) = weak.upgrade() {
                paste.request(&ui);
            }
        }
    });
    ui.on_reset_keyboard({
        let (weak, input) = (ui.as_weak(), input.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                input.borrow_mut().reset();
                ui.set_native_composing(false);
                ui.invoke_keyboard_updated();
            }
        }
    });
    ui.on_keyboard_updated({
        let (weak, input, connection) = (ui.as_weak(), input.clone(), connection.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                refresh(&ui, &input, &connection);
            }
        }
    });
    for once in [false, true] {
        let (weak, input, connection, paste) = (
            ui.as_weak(),
            input.clone(),
            connection.clone(),
            paste.clone(),
        );
        let action = move || {
            let Some(ui) = weak.upgrade() else { return };
            if !ui.get_vim_available() || once && !ui.get_vim_once_available() {
                return;
            }
            {
                let mut input = input.borrow_mut();
                let view = connection.updates.borrow();
                let context = input.observe(&view);
                if !actions_available(&ui, &view)
                    || once && !matches!(input.keyboard.status(&context).mode, "normal" | "once")
                {
                    return;
                }
                input.composition = None;
                if once {
                    input.keyboard.arm();
                } else {
                    input.keyboard.toggle();
                }
            }
            ui.set_native_composing(false);
            paste.cancel_read();
            ui.invoke_clear_selection();
            ui.invoke_station_mouse_cancel();
            refresh(&ui, &input, &connection);
            ui.invoke_focus_terminal();
        };
        if once {
            ui.on_vim_once(action);
        } else {
            ui.on_toggle_vim(action);
        }
    }
    ui.on_terminal_focus_changed({
        let (weak, input, paste) = (ui.as_weak(), input.clone(), paste.clone());
        move || {
            if let Some(ui) = weak.upgrade() {
                if !ui.get_find_focused()
                    && !ui.get_terminal_input_focused()
                    && !ui.get_article_focused()
                {
                    input.borrow_mut().reset();
                    if !ui.get_context_open() && !ui.get_context_pending() && !ui.get_tools_busy() {
                        ui.invoke_station_mouse_cancel();
                    }
                    ui.set_native_composing(false);
                    paste.cancel_read();
                }
                ui.invoke_keyboard_updated();
                ui.invoke_keep_alive_changed();
            }
        }
    });
    let weak = ui.as_weak();
    let paste_owner = paste.clone();
    ui.window().on_winit_window_event(move |_, event| {
        use EventResult::{PreventDefault, Propagate};
        let Some(ui) = weak.upgrade() else {
            return Propagate;
        };
        if ui.get_article_active()
            && ui.get_article_focused()
            && matches!(event, WindowEvent::Ime(Ime::Preedit(text, _)) if !text.is_empty())
            && let Some(article) = &article
        {
            article.begin_composition();
        }
        let result = (|| {
            let mut input = input.borrow_mut();
            if matches!(
                event,
                WindowEvent::KeyboardInput { .. } | WindowEvent::Ime(_)
            ) {
                input.observe(&connection.updates.borrow());
            }
            match event {
                WindowEvent::Focused(focused) => {
                    input.window_focused = *focused;
                    let foreground = *focused && !input.occluded;
                    if !focused {
                        input.reset();
                        input.modifiers = ModifiersState::empty();
                        input.pressed_buttons = 0;
                    }
                    drop(input);
                    ui.set_native_foreground(foreground);
                    if !focused {
                        if let Some(article) = &article {
                            article.end_composition();
                        }
                        ui.set_native_composing(false);
                        ui.set_find_composing(false);
                        ui.set_native_pointer_down(false);
                        ui.set_native_pointer_buttons(0);
                        ui.invoke_station_mouse_cancel();
                        paste.cancel(&ui, false);
                        ui.invoke_clear_login_password();
                        ui.invoke_clear_selection();
                    }
                    ui.invoke_keep_alive_changed();
                    return Propagate;
                }
                WindowEvent::Occluded(occluded) => {
                    input.occluded = *occluded;
                    let foreground = input.window_focused && !occluded;
                    if *occluded {
                        input.reset();
                        input.pressed_buttons = 0;
                    }
                    drop(input);
                    ui.set_native_foreground(foreground);
                    if *occluded {
                        if let Some(article) = &article {
                            article.end_composition();
                        }
                        ui.set_native_composing(false);
                        ui.set_find_composing(false);
                        ui.set_native_pointer_down(false);
                        ui.set_native_pointer_buttons(0);
                        ui.invoke_station_mouse_cancel();
                        paste.cancel(&ui, false);
                        ui.invoke_clear_selection();
                    }
                    ui.invoke_keep_alive_changed();
                    return Propagate;
                }
                WindowEvent::CloseRequested => {
                    drop(input);
                    ui.set_native_foreground(false);
                    ui.set_native_pointer_buttons(0);
                    ui.invoke_station_mouse_cancel();
                    ui.set_login_open(false);
                    ui.invoke_clear_login_password();
                    ui.invoke_clear_selection();
                    ui.invoke_keep_alive_changed();
                    paste.cancel(&ui, false);
                    return Propagate;
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    drop(input);
                    paste.cancel(&ui, true);
                    ui.invoke_station_mouse_cancel();
                    ui.invoke_clear_selection();
                    return Propagate;
                }
                WindowEvent::CursorLeft { .. } if input.pressed_buttons != 0 => {
                    drop(input);
                    ui.invoke_station_mouse_cancel();
                    ui.invoke_clear_selection();
                    return Propagate;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let mask = match button {
                        MouseButton::Left => 1,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 4,
                        _ => return Propagate,
                    };
                    match state {
                        ElementState::Pressed => input.pressed_buttons |= mask,
                        ElementState::Released => input.pressed_buttons &= !mask,
                    }
                    let buttons = input.pressed_buttons;
                    let pressed = buttons != 0;
                    drop(input);
                    ui.set_native_pointer_down(pressed);
                    ui.set_native_pointer_buttons(i32::from(buttons));
                    ui.invoke_keep_alive_changed();
                    return Propagate;
                }
                WindowEvent::MouseWheel { .. } if input.pressed_buttons == 0 => {
                    drop(input);
                    // A wheel gesture can have no press/release edges on desktop.
                    ui.set_native_pointer_down(true);
                    ui.invoke_keep_alive_changed();
                    ui.set_native_pointer_down(false);
                    ui.invoke_keep_alive_changed();
                    return Propagate;
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    input.modifiers = modifiers.state();
                    return Propagate;
                }
                WindowEvent::Ime(Ime::Disabled) => {
                    input.ime_disabled();
                    drop(input);
                    if let Some(article) = &article {
                        article.end_composition();
                    }
                    ui.set_native_composing(false);
                    ui.set_find_composing(false);
                    ui.invoke_keep_alive_changed();
                    return Propagate;
                }
                WindowEvent::Ime(Ime::Preedit(..) | Ime::Commit(_))
                | WindowEvent::KeyboardInput {
                    is_synthetic: false,
                    ..
                } => {}
                _ => return Propagate,
            }
            // Local search owns its native text field, including IME and paste.
            // Route it before the BBS focus/keyboard gates (Vim may be normal).
            if let WindowEvent::KeyboardInput { event, .. } = event
                && ui.get_find_available()
                && !ui.get_find_blocked()
                && !ui.get_find_composing()
                && !input.composition.as_ref().is_some_and(|c| c.active)
                && find_shortcut(event.physical_key, input.modifiers)
            {
                let activate = event.state == ElementState::Pressed && !event.repeat;
                drop(input);
                if activate {
                    ui.invoke_find_action(0);
                }
                return PreventDefault;
            }
            if ui.get_find_open() {
                let composing = ui.get_find_composing();
                let back = input.modifiers.shift_key();
                if let WindowEvent::KeyboardInput { event, .. } = event
                    && ui.get_find_focused()
                    && !composing
                    && !ui.get_find_blocked()
                    && let Some(code) = find_edit_shortcut(event.physical_key, input.modifiers)
                {
                    let activate = event.state == ElementState::Pressed && !event.repeat;
                    drop(input);
                    if activate {
                        ui.invoke_find_edit_key(code);
                    }
                    return PreventDefault;
                }
                drop(input);
                match event {
                    WindowEvent::Ime(Ime::Preedit(text, _)) => {
                        ui.set_find_composing(!text.is_empty())
                    }
                    WindowEvent::Ime(Ime::Commit(_)) => ui.set_find_composing(false),
                    WindowEvent::KeyboardInput { event, .. }
                        if !composing && !ui.get_find_blocked() =>
                    {
                        let action = match event.logical_key {
                            Key::Named(NamedKey::Enter) => Some(if back { 1 } else { 2 }),
                            Key::Named(NamedKey::Escape) => Some(3),
                            _ => None,
                        };
                        if let Some(action) = action {
                            if event.state == ElementState::Pressed && !event.repeat {
                                ui.invoke_find_action(action);
                            }
                            return PreventDefault;
                        }
                    }
                    _ => {}
                }
                return Propagate;
            }
            let article_focused =
                ui.get_article_active() && ui.get_article_focused() && article.is_some();
            if ui.get_media_open()
                || ui.get_paste_open()
                || ui.get_login_open()
                || ui.get_context_open()
                || ui.get_context_pending()
                || ui.get_tools_busy()
                || !input.window_focused
                || input.occluded
                || (!ui.get_terminal_input_focused() && !article_focused)
            {
                return Propagate;
            }
            if let WindowEvent::KeyboardInput { event, .. } = event
                && !input
                    .composition
                    .as_ref()
                    .is_some_and(|composition| composition.active)
            {
                if ui.get_selection_active() && event.logical_key == Key::Named(NamedKey::Escape) {
                    if event.state == ElementState::Pressed && !event.repeat {
                        ui.invoke_clear_selection();
                    }
                    return PreventDefault;
                }
                let copy_key = matches!(event.logical_key.as_ref(), Key::Character("c" | "C"))
                    || event.physical_key == PhysicalKey::Code(KeyCode::KeyC);
                if ui.get_selection_active()
                    && copy_key
                    && !input.modifiers.alt_key()
                    && (input.modifiers.control_key() || input.modifiers.super_key())
                {
                    if event.state == ElementState::Pressed && !event.repeat {
                        ui.invoke_copy_selection(input.modifiers.shift_key());
                    }
                    return PreventDefault;
                }
            }
            let (stamp, context, encoding, station_paste_key) = {
                let view = connection.updates.borrow();
                let reading_context = article.as_ref().and_then(|a| a.input_context(&view));
                if ui.get_switching_station()
                    || view.phase != Phase::Connected
                    || reading_context.is_none()
                        && presented
                            .get()
                            .is_none_or(|id| id.generation != view.generation)
                {
                    if let WindowEvent::Ime(Ime::Preedit(text, _)) = event {
                        input.preedit(None, !text.is_empty());
                        ui.set_native_composing(!text.is_empty());
                    } else if matches!(event, WindowEvent::Ime(Ime::Commit(_))) {
                        input.reset();
                        ui.set_native_composing(false);
                    }
                    drop(view);
                    ui.invoke_keep_alive_changed();
                    return PreventDefault;
                }
                let station_paste_key = !view.output_pending
                    && presented
                        .get()
                        .is_some_and(|id| id.revision == view.frame.revision)
                    && view.site.ptt_features()
                    && pttstella::navigation::article_editor(&view.frame);
                (
                    view.stamp(),
                    reading_context.unwrap_or_else(|| view.context()),
                    view.site.encoding,
                    station_paste_key,
                )
            };
            input.keyboard.context(&context);
            match event {
                WindowEvent::Ime(Ime::Preedit(text, _)) => {
                    if matches!(input.keyboard.status(&context).mode, "normal" | "suspended") {
                        input.composition = None;
                        ui.set_native_composing(false);
                        return PreventDefault;
                    }
                    if !text.is_empty() {
                        paste.cancel_read();
                    }
                    input.preedit(Some(stamp.generation), !text.is_empty());
                    ui.set_native_composing(!text.is_empty());
                    ui.invoke_keep_alive_changed();
                    Propagate
                }
                WindowEvent::Ime(Ime::Commit(text)) => {
                    let accepted = input.take_commit(stamp.generation);
                    drop(input);
                    if accepted {
                        ui.invoke_station_mouse_cancel();
                        send_text(
                            &ui,
                            &connection,
                            article.as_deref(),
                            stamp.generation,
                            text,
                            encoding,
                        );
                    }
                    if let Some(article) = &article {
                        article.end_composition();
                    }
                    ui.set_native_composing(false);
                    ui.invoke_keep_alive_changed();
                    PreventDefault
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if input
                        .composition
                        .as_ref()
                        .is_some_and(|composition| composition.active)
                    {
                        return PreventDefault;
                    }
                    if input.composition.take().is_some() {
                        input.keyboard.composition_end();
                        if let Some(article) = &article {
                            article.end_composition();
                        }
                    }
                    if clipboard_shortcut(
                        event.logical_key.as_ref(),
                        event.physical_key,
                        input.modifiers,
                        station_paste_key,
                    ) {
                        if event.state == ElementState::Pressed && !event.repeat {
                            drop(input);
                            paste.request(&ui);
                        }
                        return PreventDefault;
                    }
                    if event.state == ElementState::Pressed {
                        paste.cancel_read();
                    }
                    if input.modifiers.super_key() {
                        return Propagate;
                    }
                    let read_only =
                        matches!(input.keyboard.status(&context).mode, "normal" | "suspended");
                    let Some(key) = terminal_key(event, input.modifiers, read_only) else {
                        return Propagate;
                    };
                    if event.state == ElementState::Released {
                        input.keyboard.release(&key.code);
                        return PreventDefault;
                    }
                    if key.key == "Escape" && ui.invoke_leave_fullscreen() {
                        return PreventDefault;
                    }
                    ui.invoke_clear_selection();
                    ui.invoke_station_mouse_cancel();
                    match input.keyboard.key(&key, &context) {
                        Effect::Native(bytes) => {
                            drop(input);
                            let result = if let Some(article) = &article {
                                article.route_input(stamp.generation, bytes)
                            } else {
                                connection.input(stamp.generation, bytes)
                            };
                            if let Err(error) = result {
                                ui.set_result(error.into());
                            }
                        }
                        Effect::None if input.keyboard.accept_text(false) => {
                            if let Some(text) = &event.text {
                                drop(input);
                                send_text(
                                    &ui,
                                    &connection,
                                    article.as_deref(),
                                    stamp.generation,
                                    text,
                                    encoding,
                                );
                            }
                        }
                        Effect::Browse(intent)
                            if article.as_ref().is_some_and(|a| {
                                a.input_context(&connection.updates.borrow()).is_some()
                            }) || presented
                                .get()
                                .is_some_and(|id| id.revision == stamp.revision) =>
                        {
                            drop(input);
                            if article
                                .as_ref()
                                .is_some_and(|article| article.local_intent(intent))
                            {
                                return PreventDefault;
                            }
                            if let Some(article) = &article {
                                article.station_handoff();
                            }
                            match connection.request_browse_key(stamp, intent) {
                                Ok(reply) => {
                                    let (weak, connection) = (weak.clone(), connection.clone());
                                    if let Err(error) = slint::spawn_local(async move {
                                        if let Err(error) = reply.wait().await
                                            && connection.updates.borrow().generation
                                                == stamp.generation
                                            && let Some(ui) = weak.upgrade()
                                        {
                                            ui.set_result(error.into());
                                        }
                                    }) {
                                        ui.set_result(error.to_string().into());
                                    }
                                }
                                Err(error) => ui.set_result(error.into()),
                            }
                        }
                        Effect::Notice(notice) => ui.set_result(notice.into()),
                        Effect::Browse(_) | Effect::None => {}
                    }
                    PreventDefault
                }
                _ => Propagate,
            }
        })();
        if matches!(
            event,
            WindowEvent::KeyboardInput { .. }
                | WindowEvent::Ime(_)
                | WindowEvent::Focused(_)
                | WindowEvent::Occluded(_)
                | WindowEvent::CloseRequested
        ) {
            refresh(&ui, &input, &connection);
        }
        result
    });
    ui.invoke_keyboard_updated();
    paste_owner
}

fn refresh(ui: &TerminalWindow, input: &RefCell<Input>, connection: &Connection) {
    let (status, available, ime_update) = {
        let mut input = input.borrow_mut();
        let view = connection.updates.borrow();
        let context = input.observe(&view);
        let status = input.keyboard.status(&context);
        let available = actions_available(ui, &view);
        let allowed = ui.get_find_focused() || matches!(status.mode, "input" | "once");
        let ime_update = if !ui.get_find_focused()
            && !ui.get_terminal_input_focused()
            && !ui.get_article_focused()
        {
            input.ime_allowed = None;
            None
        } else if input.ime_allowed != Some(allowed) {
            input.ime_allowed = Some(allowed);
            Some(allowed)
        } else {
            None
        };
        (status, available, ime_update)
    };
    // Changing read-only on a focused Slint TextInput does not update the OS IME.
    // Apply the same mode to Winit, only while this field owns text input.
    ui.set_keyboard_read_only(matches!(status.mode, "normal" | "suspended"));
    ui.set_keyboard_pending(status.pending);
    ui.set_vim_enabled(status.enabled);
    ui.set_vim_available(available);
    ui.set_vim_once_available(available && matches!(status.mode, "normal" | "once"));
    ui.set_vim_label(
        if status.enabled {
            pttstella::msg!("vim-on")
        } else {
            pttstella::msg!("vim-off")
        }
        .into(),
    );
    ui.set_vim_once_label(pttstella::msg!("vim-once-button").into());
    ui.set_keyboard_label(status.label().into());
    ui.invoke_article_keyboard_mode(status.enabled && matches!(status.mode, "input" | "once"));
    if let Some(allowed) = ime_update {
        ui.window()
            .with_winit_window(|window| window.set_ime_allowed(allowed));
    }
    ui.invoke_keep_alive_changed();
}

fn actions_available(ui: &TerminalWindow, view: &View) -> bool {
    view.site.ptt_features()
        && view.phase == Phase::Connected
        && !ui.get_switching_station()
        && !ui.get_paste_open()
        && !ui.get_media_open()
        && !ui.get_login_open()
        && !ui.get_login_busy()
        && !ui.get_output_busy()
}

fn find_shortcut(physical: PhysicalKey, modifiers: ModifiersState) -> bool {
    physical == PhysicalKey::Code(KeyCode::KeyF)
        && modifiers.control_key() != modifiers.super_key()
        && !modifiers.alt_key()
        && !modifiers.shift_key()
}
fn find_edit_shortcut(physical: PhysicalKey, modifiers: ModifiersState) -> Option<i32> {
    let command = if cfg!(target_os = "macos") {
        modifiers.super_key() && !modifiers.control_key()
    } else {
        modifiers.control_key() && !modifiers.super_key()
    };
    if !command || modifiers.alt_key() {
        return None;
    }
    match physical {
        PhysicalKey::Code(KeyCode::KeyZ) => Some(if modifiers.shift_key() { 5 } else { 4 }),
        PhysicalKey::Code(KeyCode::KeyA) if !modifiers.shift_key() => Some(0),
        PhysicalKey::Code(KeyCode::KeyC) if !modifiers.shift_key() => Some(1),
        PhysicalKey::Code(KeyCode::KeyX) if !modifiers.shift_key() => Some(2),
        PhysicalKey::Code(KeyCode::KeyV) if !modifiers.shift_key() => Some(3),
        _ => None,
    }
}

fn clipboard_shortcut(
    letter: Key<&str>,
    physical_key: PhysicalKey,
    modifiers: ModifiersState,
    station_paste_key: bool,
) -> bool {
    let paste_key = matches!(letter, Key::Character("v" | "V"))
        || physical_key == PhysicalKey::Code(KeyCode::KeyV);
    paste_key
        && !modifiers.alt_key()
        && (modifiers.super_key()
            || modifiers.control_key() && (modifiers.shift_key() || !station_paste_key))
}

fn send_text(
    ui: &TerminalWindow,
    connection: &Connection,
    article: Option<&crate::article::session::ArticleSession>,
    generation: u64,
    text: &str,
    encoding: TextEncoding,
) {
    if text.is_empty() || text.chars().any(char::is_control) {
        return;
    }
    let result = input::encode_with(text, encoding)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            if let Some(article) = article {
                article.route_input(generation, bytes)
            } else {
                connection.input(generation, bytes)
            }
        });
    if let Err(error) = result {
        ui.set_result(error.into());
    }
}

fn terminal_key(event: &KeyEvent, modifiers: ModifiersState, read_only: bool) -> Option<vim::Key> {
    terminal_key_parts(
        event.logical_key.as_ref(),
        event.physical_key,
        modifiers,
        read_only,
        event.repeat,
    )
}

fn terminal_key_parts(
    logical_key: Key<&str>,
    physical_key: PhysicalKey,
    modifiers: ModifiersState,
    read_only: bool,
    repeat: bool,
) -> Option<vim::Key> {
    use NamedKey as N;
    let key = match logical_key {
        Key::Character(text) => text.chars().next()?.to_string(),
        Key::Unidentified(_) if modifiers.control_key() || read_only => "Process".into(),
        Key::Named(named) => match named {
            N::Enter => "Enter",
            N::Backspace => "Backspace",
            N::Tab => "Tab",
            N::Escape => "Escape",
            N::ArrowUp => "ArrowUp",
            N::ArrowDown => "ArrowDown",
            N::ArrowLeft => "ArrowLeft",
            N::ArrowRight => "ArrowRight",
            N::Home => "Home",
            N::End => "End",
            N::PageUp => "PageUp",
            N::PageDown => "PageDown",
            N::Delete => "Delete",
            N::Space => " ",
            _ => return None,
        }
        .into(),
        _ => return None,
    };
    let code = match physical_key {
        PhysicalKey::Code(code) => physical_code(code).unwrap_or_else(|| key.clone()),
        PhysicalKey::Unidentified(_) => key.clone(),
    };
    Some(vim::Key {
        key,
        code,
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        shift: modifiers.shift_key(),
        repeat,
    })
}

fn physical_code(code: KeyCode) -> Option<String> {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ];
    if let Some(index) = LETTERS.iter().position(|&letter| letter == code) {
        return Some(format!("Key{}", char::from(b'A' + index as u8)));
    }
    DIGITS
        .iter()
        .position(|&digit| digit == code)
        .map(|index| format!("Digit{index}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_uses_physical_f_and_preserves_fullscreen_and_composition_modifiers() {
        let f = PhysicalKey::Code(KeyCode::KeyF);
        assert!(find_shortcut(f, ModifiersState::CONTROL));
        assert!(find_shortcut(f, ModifiersState::SUPER));
        for modifiers in [
            ModifiersState::empty(),
            ModifiersState::CONTROL | ModifiersState::SUPER,
            ModifiersState::CONTROL | ModifiersState::ALT,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
        ] {
            assert!(!find_shortcut(f, modifiers));
        }
        assert!(!find_shortcut(
            PhysicalKey::Code(KeyCode::Slash),
            ModifiersState::CONTROL
        ));
        let command = if cfg!(target_os = "macos") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        };
        for (key, code) in [
            (KeyCode::KeyA, 0),
            (KeyCode::KeyC, 1),
            (KeyCode::KeyX, 2),
            (KeyCode::KeyV, 3),
            (KeyCode::KeyZ, 4),
        ] {
            assert_eq!(
                find_edit_shortcut(PhysicalKey::Code(key), command),
                Some(code)
            );
            assert_eq!(
                find_edit_shortcut(PhysicalKey::Code(key), command | ModifiersState::ALT),
                None
            );
        }
        assert_eq!(
            find_edit_shortcut(
                PhysicalKey::Code(KeyCode::KeyZ),
                command | ModifiersState::SHIFT
            ),
            Some(5)
        );
    }
    #[test]
    fn native_adapter_uses_cjk_physical_keys_only_for_browsing_and_control_shortcuts() {
        let context = pttstella::navigation::Context::ArticleReading {
            first_line: 1,
            last_line: 23,
            percent: 50,
        };
        let mut keyboard = Keyboard::default();
        keyboard.toggle();
        let mapped = terminal_key_parts(
            Key::Character("注"),
            PhysicalKey::Code(KeyCode::KeyJ),
            ModifiersState::empty(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            keyboard.key(&mapped, &context),
            Effect::Browse(pttstella::navigation::Intent::Lines {
                down: true,
                count: 1
            })
        );
        let unidentified =
            Key::Unidentified(slint::winit_030::winit::keyboard::NativeKey::Unidentified);
        assert!(
            terminal_key_parts(
                unidentified.clone(),
                PhysicalKey::Code(KeyCode::KeyJ),
                ModifiersState::empty(),
                false,
                false
            )
            .is_none()
        );
        let mapped = terminal_key_parts(
            unidentified,
            PhysicalKey::Code(KeyCode::KeyJ),
            ModifiersState::empty(),
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            keyboard.key(&mapped, &context),
            Effect::Browse(pttstella::navigation::Intent::Lines {
                down: true,
                count: 1
            })
        );
        keyboard.toggle();
        let mapped = terminal_key_parts(
            Key::Character("注"),
            PhysicalKey::Code(KeyCode::KeyX),
            ModifiersState::CONTROL,
            false,
            false,
        )
        .unwrap();
        assert_eq!(keyboard.key(&mapped, &context), Effect::Native(vec![24]));
        let mapped = terminal_key_parts(
            Key::Character("注"),
            PhysicalKey::Code(KeyCode::KeyJ),
            ModifiersState::empty(),
            false,
            false,
        )
        .unwrap();
        assert_eq!(mapped.key, "注");
        assert_eq!(keyboard.key(&mapped, &context), Effect::None);
        assert!(keyboard.accept_text(false));
    }

    fn key(text: &str) -> vim::Key {
        vim::Key {
            key: text.into(),
            code: format!("Key{}", text.to_uppercase()),
            ctrl: false,
            alt: false,
            shift: text == "G",
            repeat: false,
        }
    }

    #[test]
    fn native_once_survives_own_ime_disabled_and_rejects_unleased_commits() {
        let context = pttstella::navigation::Context::ArticleReading {
            first_line: 1,
            last_line: 23,
            percent: 50,
        };
        let mut input = Input::default();
        input.keyboard.toggle();
        input.keyboard.context(&context);
        input.keyboard.arm();
        assert!(!input.take_commit(1));
        assert_eq!(input.keyboard.status(&context).mode, "once");
        assert_eq!(input.keyboard.key(&key("j"), &context), Effect::None);
        assert!(input.keyboard.accept_text(false));
        input.ime_allowed = Some(false);
        input.ime_disabled();
        let mut repeat = key("j");
        repeat.repeat = true;
        assert_eq!(input.keyboard.key(&repeat, &context), Effect::None);
        input.keyboard.release("KeyJ");
        assert_eq!(
            input.keyboard.key(&key("j"), &context),
            Effect::Browse(pttstella::navigation::Intent::Lines {
                down: true,
                count: 1
            })
        );
        input.keyboard.arm();
        input.preedit(Some(1), true);
        input.preedit(Some(1), false);
        assert!(input.take_commit(1));
        assert!(!input.take_commit(1));
        assert_eq!(input.keyboard.status(&context).mode, "normal");
    }

    async fn page(connection: &Connection, stage: usize) -> View {
        let mut updates = connection.updates.clone();
        let marker = format!("test{stage:02}");
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            updates.wait_for(|view| {
                !view.output_pending
                    && view
                        .frame
                        .glyphs
                        .iter()
                        .filter(|glyph| glyph.row == 0)
                        .map(|glyph| glyph.text.as_str())
                        .collect::<String>()
                        .contains(&marker)
            }),
        )
        .await
        .unwrap()
        .unwrap()
        .clone()
    }

    #[tokio::test]
    async fn native_keyboard_policy_passes_strict_46_step_peer_and_guidance_lifecycle() {
        use pttstella::connection::CloseOutcome;
        let connection = Connection::start_window_probe().unwrap();
        connection.connect().unwrap();
        let mut view = page(&connection, 0).await;
        let mut input = Input::default();
        input.observe(&view);
        input.keyboard.toggle();
        for step in 0..46 {
            input.observe(&view);
            let sequence = match step {
                30 | 42 => "gg",
                31 => "4j",
                32 => "l",
                33 | 39 | 44 | 45 => "h",
                34..=36 => "j",
                37 | 40 => "j",
                38 | 41 => "k",
                43 => "G",
                20.. if step % 2 == 0 => "l",
                20.. => "h",
                _ => match step % 5 {
                    0 => "j",
                    1 => "k",
                    2 => "gg",
                    3 => "G",
                    _ if step == 19 => "h",
                    _ => "l",
                },
            };
            for text in sequence.chars() {
                let key = key(&text.to_string());
                match input.keyboard.key(&key, &view.context()) {
                    Effect::None => {}
                    Effect::Browse(intent) => {
                        connection
                            .request_browse_key(view.stamp(), intent)
                            .unwrap()
                            .wait()
                            .await
                            .unwrap_or_else(|e| panic!("step {step}: {e}"));
                    }
                    effect => panic!("unexpected effect at step {step}: {effect:?}"),
                }
                input.keyboard.release(&key.code);
            }
            view = page(
                &connection,
                if matches!(step, 30 | 31) {
                    30
                } else {
                    step + 1
                },
            )
            .await;
        }
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );

        input.keyboard.key(&key("g"), &view.context());
        view.generation += 1;
        input.observe(&view);
        assert!(!input.keyboard.status(&view.context()).pending);
        assert_eq!(input.keyboard.status(&view.context()).mode, "normal");
        view.login_guidance = pttstella::login::Guidance::CodeRequired;
        input.observe(&view);
        assert_eq!(input.keyboard.status(&view.context()).mode, "input");
        input.observe(&view);
        view.site =
            pttstella::sites::Target::new("wss://example.test/bbs".into(), TextEncoding::Uao)
                .unwrap();
        input.observe(&view);
        assert!(!input.keyboard.status(&view.context()).enabled);
    }

    #[test]
    fn station_editor_owns_plain_control_v_and_leaves_modified_paste_local() {
        let key = PhysicalKey::Code(KeyCode::KeyV);
        for (modifiers, editor, local) in [
            (ModifiersState::CONTROL, true, false),
            (ModifiersState::CONTROL, false, true),
            (ModifiersState::CONTROL | ModifiersState::SHIFT, true, true),
            (ModifiersState::SUPER, true, true),
            (ModifiersState::CONTROL | ModifiersState::ALT, false, false),
        ] {
            assert_eq!(
                clipboard_shortcut(Key::Character("注"), key, modifiers, editor),
                local
            );
        }
    }

    #[test]
    fn composition_lease_survives_empty_preedit_but_not_new_generation_or_focus_loss() {
        let mut input = Input::default();
        input.preedit(Some(1), true);
        input.preedit(Some(1), false);
        assert!(input.take_commit(1));
        assert!(!input.take_commit(1));

        input.preedit(Some(1), true);
        input.preedit(None, true);
        input.preedit(Some(2), true);
        input.preedit(Some(2), false);
        assert!(!input.take_commit(2));

        input.preedit(Some(2), true);
        input.reset();
        assert!(!input.take_commit(2));
    }
}
