// Tests for src/executor/builtin/fs.rs
//
// parse_chmod_mode is #[cfg(unix)] only, so these tests are too.
#![cfg(unix)]
use super::*;

#[test]
fn octal_mode_parses_directly() {
    assert_eq!(parse_chmod_mode("755", 0), Some(0o755));
    assert_eq!(parse_chmod_mode("644", 0), Some(0o644));
}

#[test]
fn symbolic_add_permission_for_all() {
    assert_eq!(parse_chmod_mode("+x", 0o644), Some(0o755));
}

#[test]
fn symbolic_remove_permission_for_user() {
    assert_eq!(parse_chmod_mode("u-w", 0o644), Some(0o444));
}

#[test]
fn symbolic_set_permission_with_equals() {
    assert_eq!(parse_chmod_mode("go=r", 0o777), Some(0o744));
}

#[test]
fn symbolic_multiple_who_targets() {
    // owner and group both gain execute; other is untouched
    assert_eq!(parse_chmod_mode("ug+x", 0o644), Some(0o754));
}

#[test]
fn symbolic_comma_separated_clauses() {
    // u+x: 0o644 -> 0o744 (owner gains execute)
    // g-r: 0o744 -> 0o704 (group loses read)
    assert_eq!(parse_chmod_mode("u+x,g-r", 0o644), Some(0o704));
}

#[test]
fn symbolic_invalid_permission_char_is_none() {
    assert_eq!(parse_chmod_mode("+z", 0o644), None);
}

#[test]
fn symbolic_missing_operator_is_none() {
    assert_eq!(parse_chmod_mode("u", 0o644), None);
}
