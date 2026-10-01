// Tests for src/executor/builtin/mini.rs
use super::*;

#[test]
fn highlights_keywords_in_rust() {
    let out = highlight_rust("if else");
    assert_eq!(out, "\x1b[34mif\x1b[0m \x1b[34melse\x1b[0m");
}

#[test]
fn non_keyword_identifiers_are_unstyled() {
    let out = highlight_rust("let value");
    assert_eq!(out, "\x1b[34mlet\x1b[0m value");
}

#[test]
fn highlights_numbers() {
    let out = highlight_rust("let x = 42");
    assert_eq!(out, "\x1b[34mlet\x1b[0m x = \x1b[33m42\x1b[0m");
}

#[test]
fn highlights_string_literals() {
    let out = highlight_rust(r#"let s = "hi""#);
    assert_eq!(out, "\x1b[34mlet\x1b[0m s = \x1b[32m\"hi\"\x1b[0m");
}

#[test]
fn whole_line_comment_is_fully_coloured() {
    let out = highlight_rust("// a comment");
    assert_eq!(out, "\x1b[90m// a comment\x1b[0m");
}

#[test]
fn trailing_comment_is_separated_from_code() {
    let out = highlight_rust("let x = 1 // note");
    assert_eq!(out, "\x1b[34mlet\x1b[0m x = \x1b[33m1\x1b[0m \x1b[90m// note\x1b[0m");
}

#[test]
fn comment_marker_inside_string_is_not_a_comment() {
    assert_eq!(find_comment_pos(r#"let s = "http://x";"#, "//"), None);
}

#[test]
fn unknown_extension_is_not_highlighted() {
    assert_eq!(highlight_line("plain text", "txt"), "plain text");
}

#[test]
fn python_uses_hash_comments() {
    let out = highlight_python("# comment");
    assert_eq!(out, "\x1b[90m# comment\x1b[0m");
}

#[test]
fn undo_stack_caps_at_100_entries() {
    let mut undo = UndoStack::new();
    for i in 0..150 {
        undo.push(&vec![format!("line{i}")], 0, 0);
    }
    assert_eq!(undo.stack.len(), 100);
    // Oldest entries should have been dropped — the first remaining entry is #50
    assert_eq!(undo.stack[0].0[0], "line50");
}

#[test]
fn undo_stack_pop_returns_last_pushed() {
    let mut undo = UndoStack::new();
    undo.push(&vec!["a".to_string()], 1, 2);
    undo.push(&vec!["b".to_string()], 3, 4);
    let (lines, row, col) = undo.pop().unwrap();
    assert_eq!(lines, vec!["b".to_string()]);
    assert_eq!((row, col), (3, 4));
}
