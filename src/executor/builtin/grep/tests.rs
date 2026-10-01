// Tests for src/executor/builtin/grep.rs
use super::*;

#[test]
fn highlight_match_wraps_single_occurrence() {
    let out = highlight_match("hello world", "world", false);
    assert_eq!(out, "hello \x1b[31m\x1b[1mworld\x1b[0m");
}

#[test]
fn highlight_match_wraps_all_occurrences() {
    let out = highlight_match("foo foo", "foo", false);
    assert_eq!(out, "\x1b[31m\x1b[1mfoo\x1b[0m \x1b[31m\x1b[1mfoo\x1b[0m");
}

#[test]
fn highlight_match_no_match_returns_original() {
    assert_eq!(highlight_match("hello", "xyz", false), "hello");
}

#[test]
fn highlight_match_ignore_case_preserves_original_casing() {
    let out = highlight_match("Hello World", "world", true);
    assert_eq!(out, "Hello \x1b[31m\x1b[1mWorld\x1b[0m");
}
