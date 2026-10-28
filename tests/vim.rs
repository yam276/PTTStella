use pttstella::{
    navigation::{self, Context, Intent, UnknownReason},
    vim::{Effect, Key, Keyboard},
};
fn reading(first: usize, percent: u8) -> Context {
    Context::ArticleReading {
        first_line: first,
        last_line: first + 22,
        percent,
    }
}
fn key(value: &str) -> Key {
    Key {
        key: value.into(),
        code: format!("Key{value}"),
        ctrl: false,
        alt: false,
        shift: false,
        repeat: false,
    }
}

#[test]
fn native_control_shortcuts_survive_cjk_logical_keys_in_each_raw_mode() {
    let context = Context::Unknown {
        reason: UnknownReason::Unrecognized,
    };
    for mode in 0..3 {
        for (glyph, code, byte) in [("ㄌ", "KeyX", 24), ("Ω", "KeyZ", 26), ("中", "KeyA", 1)] {
            let mut keyboard = Keyboard::default();
            keyboard.context(&context);
            if mode > 0 {
                keyboard.toggle();
            }
            if mode == 1 {
                keyboard.use_native_input();
            }
            if mode == 2 {
                keyboard.arm();
            }
            let mut command = key(glyph);
            command.code = code.into();
            command.ctrl = true;
            assert_eq!(keyboard.key(&command, &context), Effect::Native(vec![byte]));
            assert!(!keyboard.accept_text(false));
        }
    }
    let mut keyboard = Keyboard::default();
    let mut command = key("ㄌ");
    command.ctrl = true;
    command.code = "KeyX".into();
    command.alt = true;
    assert_eq!(keyboard.key(&command, &context), Effect::None);
    command.alt = false;
    command.code = "Unidentified".into();
    assert_eq!(keyboard.key(&command, &context), Effect::None);
    command.code = "KeyX".into();
    keyboard.composition_start();
    assert_eq!(keyboard.key(&command, &context), Effect::None);
    keyboard.composition_end();
    command.key = "a".into();
    assert_eq!(
        keyboard.key(&command, &context),
        Effect::Native(vec![1]),
        "an ASCII logical key still respects the active layout"
    );
    command.key = "ㄌ".into();
    command.ctrl = false;
    assert_eq!(keyboard.key(&command, &context), Effect::None);
    assert!(keyboard.accept_text(false));
}

#[test]
fn authentication_handoff_clears_prefix_and_keeps_native_input_until_explicit_escape() {
    let mut keyboard = Keyboard::default();
    keyboard.toggle();
    let context = Context::Unknown {
        reason: UnknownReason::Unrecognized,
    };
    keyboard.use_native_input();
    assert!(keyboard.status(&context).enabled);
    assert_eq!(keyboard.status(&context).mode, "input");
    assert!(!keyboard.status(&context).pending);
    for value in ["1", "2", "3", "4", "5", "6"] {
        assert_eq!(press(&mut keyboard, value, &context), Effect::None);
        assert!(keyboard.accept_text(false));
    }
    assert_eq!(
        press(&mut keyboard, "Enter", &context),
        Effect::Native(vec![13])
    );
    assert_eq!(
        press(&mut keyboard, "Escape", &context),
        Effect::Native(vec![27])
    );
    keyboard.context(&reading(1, 50));
    assert_eq!(keyboard.status(&reading(1, 50)).mode, "input");
    assert_eq!(
        press(&mut keyboard, "Escape", &reading(1, 50)),
        Effect::None
    );
    assert_eq!(keyboard.status(&reading(1, 50)).mode, "normal");
}

#[test]
fn command_help_keeps_browsing_and_requires_explicit_native_execution() {
    let context = Context::CommandHelp {
        first_row: 2,
        last_row: 4,
    };
    let mut keyboard = Keyboard::default();
    keyboard.toggle();
    keyboard.context(&context);
    assert_eq!(keyboard.status(&context).mode, "normal");
    assert_eq!(
        press(&mut keyboard, "j", &context),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 1
        })
    );
    assert_eq!(
        press(&mut keyboard, "Enter", &context),
        Effect::Browse(Intent::Forward)
    );
    assert_eq!(
        navigation::plan(&context, Intent::Forward),
        Err(navigation::Error::RequiresNativeInput)
    );
    assert_eq!(press(&mut keyboard, "f", &context), Effect::None);
    assert_eq!(
        press(&mut keyboard, "Enter", &context),
        Effect::Native(vec![13])
    );
    assert_eq!(keyboard.status(&context).mode, "normal");
    keyboard.context(&Context::EmptyList);
    assert_eq!(
        press(&mut keyboard, "h", &Context::EmptyList),
        Effect::Browse(Intent::Back)
    );
}

#[test]
fn continue_prompt_translates_once_and_holds_the_key_across_screen_changes() {
    for enabled in [false, true] {
        for value in [
            " ",
            "Enter",
            "l",
            "ArrowRight",
            "j",
            "h",
            "i",
            "f",
            "3",
            "g",
            "Escape",
            "Tab",
            "F1",
            "中",
        ] {
            let mut keyboard = Keyboard::default();
            if enabled {
                keyboard.toggle();
            }
            let mut k = key(value);
            assert_eq!(
                keyboard.key(&k, &Context::ContinuePrompt),
                Effect::Browse(Intent::Continue)
            );
            assert_eq!(
                keyboard.status(&Context::ContinuePrompt).mode,
                if enabled { "normal" } else { "input" }
            );
            assert!(!keyboard.accept_text(false));
            assert_eq!(keyboard.key(&k, &Context::ContinuePrompt), Effect::None);
            k.repeat = true;
            assert_eq!(keyboard.key(&k, &Context::ContinuePrompt), Effect::None);
            let list = Context::BoardList {
                first_row: 3,
                last_row: 22,
                cursor_row: 4,
            };
            assert_eq!(keyboard.key(&k, &list), Effect::None);
            assert!(!keyboard.accept_text(false));
            keyboard.release(&k.code);
            k.repeat = false;
            assert_eq!(
                keyboard.key(&k, &Context::ContinuePrompt),
                Effect::Browse(Intent::Continue)
            );
        }
    }
}

#[test]
fn continue_prompt_preserves_editing_modes_and_rejects_composition_and_modifier_keys() {
    for enabled in [false, true] {
        let mut keyboard = Keyboard::default();
        if enabled {
            keyboard.toggle();
        }
        for value in [
            "Shift",
            "Control",
            "Alt",
            "Meta",
            "CapsLock",
            "NumLock",
            "ScrollLock",
            "Fn",
            "Dead",
            "Process",
            "Unidentified",
            "",
        ] {
            assert_eq!(
                press(&mut keyboard, value, &Context::ContinuePrompt),
                Effect::None
            );
            assert!(!keyboard.accept_text(false));
        }
        keyboard.composition_start();
        assert_eq!(
            keyboard.key(&key("Enter"), &Context::ContinuePrompt),
            Effect::None
        );
        assert!(!keyboard.accept_text(true));
        keyboard.composition_end();
        let mut repeated = key("Enter");
        repeated.repeat = true;
        assert_eq!(
            keyboard.key(&repeated, &Context::ContinuePrompt),
            Effect::None
        );
        for modifier in ["ctrl", "alt", "shift"] {
            let mut k = key("a");
            k.ctrl = modifier == "ctrl";
            k.alt = modifier == "alt";
            k.shift = modifier == "shift";
            assert_eq!(
                keyboard.key(&k, &Context::ContinuePrompt),
                Effect::Browse(Intent::Continue)
            );
            keyboard.release(&k.code);
        }
    }
    let mut keyboard = Keyboard::default();
    keyboard.toggle();
    keyboard.use_native_input();
    assert_eq!(
        press(&mut keyboard, "Escape", &Context::ContinuePrompt),
        Effect::Browse(Intent::Continue)
    );
    assert_eq!(keyboard.status(&Context::ContinuePrompt).mode, "input");
    assert_eq!(press(&mut keyboard, "a", &Context::EmptyList), Effect::None);
    assert!(keyboard.accept_text(false));
    press(&mut keyboard, "Escape", &Context::EmptyList);
    keyboard.context(&Context::ContinuePrompt);
    keyboard.arm();
    assert_eq!(
        press(&mut keyboard, "f", &Context::ContinuePrompt),
        Effect::Browse(Intent::Continue)
    );
    assert_eq!(keyboard.status(&Context::ContinuePrompt).mode, "normal");
}

#[test]
fn continue_prompt_keeps_normal_physical_keys_but_not_native_ime_confirmation() {
    for enabled in [false, true] {
        let mut keyboard = Keyboard::default();
        if enabled {
            keyboard.toggle();
        }
        let mut process = key("Process");
        process.code = "KeyJ".into();
        assert_eq!(
            keyboard.key(&process, &Context::ContinuePrompt),
            if enabled {
                Effect::Browse(Intent::Continue)
            } else {
                Effect::None
            }
        );
        keyboard.release(&process.code);
        keyboard.composition_start();
        assert_eq!(
            keyboard.key(&process, &Context::ContinuePrompt),
            Effect::None
        );
        assert!(!keyboard.accept_text(true));
        keyboard.composition_end();
    }
}

#[test]
fn cjk_input_source_uses_physical_keys_only_in_normal_mode() {
    let c = reading(24, 50);
    let mut p = Keyboard::default();
    p.toggle();
    let mut process = key("Process");
    process.code = "KeyJ".into();
    assert_eq!(
        p.key(&process, &c),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 1
        })
    );
    assert!(!p.accept_text(false));
    process.code = "KeyG".into();
    process.shift = true;
    assert_eq!(p.key(&process, &c), Effect::Browse(Intent::Last));
    process.shift = false;
    process.code = "Digit3".into();
    assert_eq!(p.key(&process, &c), Effect::None);
    process.code = "KeyJ".into();
    assert_eq!(
        p.key(&process, &c),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 3
        })
    );
    process.code = "KeyI".into();
    assert_eq!(p.key(&process, &c), Effect::None);
    assert_eq!(p.status(&c).mode, "input");
    process.code = "KeyJ".into();
    assert_eq!(p.key(&process, &c), Effect::None);
    p.composition_start();
    assert!(p.accept_text(true));
    p.composition_end();
    press(&mut p, "Escape", &c);
    press(&mut p, "f", &c);
    assert_eq!(p.key(&process, &c), Effect::None);
    p.composition_start();
    assert!(p.accept_text(true));
    p.composition_end();
    assert_eq!(p.status(&c).mode, "normal");
}
fn press(policy: &mut Keyboard, value: &str, context: &Context) -> Effect {
    let k = key(value);
    let effect = policy.key(&k, context);
    policy.release(&k.code);
    effect
}
#[test]
fn native_once_owns_text_repeats_and_modifiers_and_restores_normal() {
    let c = reading(24, 50);
    let mut p = Keyboard::default();
    p.context(&c);
    p.toggle();
    assert_eq!(press(&mut p, "h", &c), Effect::Browse(Intent::Back));
    assert!(!p.accept_text(false));
    press(&mut p, "f", &c);
    assert_eq!(p.status(&c).mode, "once");
    press(&mut p, "Shift", &c);
    assert_eq!(p.status(&c).mode, "once");
    let h = key("h");
    assert_eq!(p.key(&h, &c), Effect::None);
    assert!(p.accept_text(false));
    assert_eq!(p.status(&c).mode, "normal");
    let mut repeat = h.clone();
    repeat.repeat = true;
    assert_eq!(p.key(&repeat, &c), Effect::None);
    assert!(!p.accept_text(false));
    p.release(&h.code);
    assert_eq!(press(&mut p, "h", &c), Effect::Browse(Intent::Back));
    press(&mut p, "f", &c);
    press(&mut p, "f", &c);
    assert!(p.accept_text(false));
    assert_eq!(p.status(&c).mode, "normal");
}

#[test]
fn modified_printable_text_is_owned_by_native_editing_not_vim_commands() {
    let c = reading(24, 50);
    let mut p = Keyboard::default();
    let mut modified = key("a");
    modified.alt = true;
    assert_eq!(p.key(&modified, &c), Effect::None);
    assert!(p.accept_text(false));
    p.toggle();
    assert!(matches!(p.key(&modified, &c), Effect::Notice(_)));
    assert!(!p.accept_text(false));
    press(&mut p, "f", &c);
    assert_eq!(p.key(&modified, &c), Effect::None);
    assert!(p.accept_text(false));
    assert_eq!(p.status(&c).mode, "normal");
}
#[test]
fn destructive_native_control_is_only_available_in_explicit_native_modes() {
    let c = reading(24, 50);
    let mut p = Keyboard::default();
    p.toggle();
    let mut d = key("d");
    d.ctrl = true;
    assert_eq!(
        p.key(&d, &c),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 11
        })
    );
    press(&mut p, "f", &c);
    assert_eq!(p.key(&d, &c), Effect::Native(vec![4]));
    p.release(&d.code);
    press(&mut p, "i", &c);
    assert!(!p.accept_text(false));
    assert_eq!(p.key(&d, &c), Effect::Native(vec![4]));
    assert_eq!(press(&mut p, "Escape", &c), Effect::None);
    assert_eq!(p.status(&c).mode, "normal");
    press(&mut p, "f", &c);
    assert_eq!(press(&mut p, "Escape", &c), Effect::Native(vec![27]));
}
#[test]
fn counts_prefixes_half_scroll_and_unknown_context_never_leak_text() {
    let c = reading(24, 50);
    let mut p = Keyboard::default();
    p.toggle();
    press(&mut p, "3", &c);
    assert!(!p.accept_text(false));
    assert_eq!(
        press(&mut p, "j", &c),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 3
        })
    );
    press(&mut p, "g", &c);
    assert_eq!(press(&mut p, "g", &c), Effect::Browse(Intent::First));
    press(&mut p, "g", &c);
    assert!(matches!(press(&mut p, "h", &c), Effect::Notice(_)));
    let mut d = key("d");
    d.ctrl = true;
    press(&mut p, "4", &c);
    assert_eq!(
        p.key(&d, &c),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 4
        })
    );
    assert_eq!(
        p.key(&d, &c),
        Effect::Browse(Intent::Lines {
            down: true,
            count: 4
        })
    );
    for _ in 0..10 {
        press(&mut p, "9", &c);
    }
    assert!(!p.accept_text(false));
    assert!(matches!(press(&mut p, "j", &c), Effect::Notice(_)));
    let unknown = Context::Unknown {
        reason: UnknownReason::Unrecognized,
    };
    assert!(matches!(press(&mut p, "h", &unknown), Effect::Notice(_)));
    assert_eq!(p.status(&unknown).mode, "suspended");
    assert_eq!(
        p.status(&unknown).suspended_reason,
        Some(UnknownReason::Unrecognized)
    );
    assert!(!p.accept_text(false));
    press(&mut p, "f", &unknown);
    press(&mut p, "h", &unknown);
    assert!(p.accept_text(false));
}
#[test]
fn ime_is_one_native_edit_and_cancel_does_not_leak_enter() {
    let c = reading(24, 50);
    let mut p = Keyboard::default();
    p.context(&c);
    p.toggle();
    p.composition_start();
    assert!(!p.accept_text(true));
    p.composition_end();
    press(&mut p, "f", &c);
    p.composition_start();
    assert!(p.accept_text(true));
    p.composition_end();
    assert_eq!(p.status(&c).mode, "normal");
    assert!(!p.accept_text(false));
    press(&mut p, "f", &c);
    p.composition_start();
    p.composition_end();
    assert_eq!(p.status(&c).mode, "normal");
}
#[test]
fn reading_edges_are_local_noops_and_repeat_bytes_are_bounded() {
    assert_eq!(
        navigation::plan(&reading(1, 30), Intent::First).unwrap(),
        b""
    );
    assert_eq!(
        navigation::plan(
            &reading(24, 100),
            Intent::Lines {
                down: true,
                count: 10
            }
        )
        .unwrap(),
        b""
    );
    assert_eq!(
        navigation::plan(
            &reading(3, 50),
            Intent::Lines {
                down: false,
                count: 11
            }
        )
        .unwrap(),
        b"kk"
    );
    assert!(
        navigation::plan(
            &reading(24, 50),
            Intent::Lines {
                down: true,
                count: usize::MAX
            }
        )
        .is_err()
    );
}

#[test]
fn main_menu_and_board_half_pages_use_their_content_height() {
    for (context, count) in [
        (
            Context::MainMenu {
                first_row: 12,
                last_row: 15,
                cursor_row: 13,
            },
            2,
        ),
        (
            Context::BoardList {
                first_row: 3,
                last_row: 22,
                cursor_row: 4,
            },
            10,
        ),
    ] {
        let mut policy = Keyboard::default();
        policy.toggle();
        policy.context(&context);
        let mut control = key("d");
        control.ctrl = true;
        assert_eq!(
            policy.key(&control, &context),
            Effect::Browse(Intent::Lines { down: true, count })
        );
        assert_eq!(policy.status(&context).mode, "normal");
    }
}
