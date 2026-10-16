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
        repeat: false,
    }
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
