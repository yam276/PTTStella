//! Shared keyboard ordering for native content menus and floating tool controls.
pub fn move_selection(current: usize, enabled: &[usize], key: i32) -> Option<usize> {
    if enabled.is_empty() {
        return None;
    }
    let index = enabled.iter().position(|i| *i == current).unwrap_or(0);
    Some(match key {
        2 => enabled[(index + enabled.len() - 1) % enabled.len()],
        3 => enabled[(index + 1) % enabled.len()],
        4 => enabled[0],
        5 => enabled[enabled.len() - 1],
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_controls_are_skipped_and_empty_or_unknown_movements_do_nothing() {
        assert_eq!(move_selection(1, &[1, 3], 2), Some(3));
        assert_eq!(move_selection(3, &[1, 3], 3), Some(1));
        assert_eq!(move_selection(3, &[1, 3], 4), Some(1));
        assert_eq!(move_selection(1, &[1, 3], 5), Some(3));
        assert_eq!(move_selection(0, &[], 3), None);
        assert_eq!(move_selection(1, &[1, 3], 99), None);
    }
}
