// Tests for src/shell/mod.rs
use super::*;

#[test]
fn detects_function_keyword_style() {
    assert_eq!(parse_function_start("function greet {"), Some("greet".to_string()));
    assert_eq!(parse_function_start("function greet() {"), Some("greet".to_string()));
}

#[test]
fn detects_posix_style() {
    assert_eq!(parse_function_start("greet() {"), Some("greet".to_string()));
}

#[test]
fn posix_style_requires_brace_after_parens() {
    assert_eq!(parse_function_start("greet()"), None);
}

#[test]
fn rejects_non_identifier_names() {
    assert_eq!(parse_function_start("foo-bar() {"), None);
}

#[test]
fn plain_command_is_not_a_function() {
    assert_eq!(parse_function_start("echo hello"), None);
}
