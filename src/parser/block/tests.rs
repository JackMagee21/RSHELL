// Tests for src/parser/block.rs
use super::*;

#[test]
fn find_keyword_respects_word_boundaries() {
    assert_eq!(find_keyword("if true then echo", "then"), Some(8));
    // "thenceforth" contains "then" but not as a whole word
    assert_eq!(find_keyword("thenceforth x", "then"), None);
    assert_eq!(find_keyword("no keyword here", "fi"), None);
}

#[test]
fn find_keyword_at_start_or_end() {
    assert_eq!(find_keyword("fi", "fi"), Some(0));
    assert_eq!(find_keyword("echo hi fi", "fi"), Some(8));
}

#[test]
fn extract_block_brace_style_with_else() {
    let (body, else_part) = extract_block("{ echo a } else { echo b }").unwrap();
    assert_eq!(body, "echo a");
    assert_eq!(else_part, Some("{ echo b }".to_string()));
}

#[test]
fn extract_block_brace_style_handles_nesting() {
    let (body, _) = extract_block("{ if true { echo a } }").unwrap();
    assert_eq!(body, "if true { echo a }");
}

#[test]
fn extract_block_then_fi_with_else() {
    let (body, else_part) = extract_block("echo a else echo b fi").unwrap();
    assert_eq!(body, "echo a");
    assert_eq!(else_part, Some("echo b".to_string()));
}

#[test]
fn extract_block_then_fi_without_else() {
    let (body, else_part) = extract_block("echo a fi").unwrap();
    assert_eq!(body, "echo a");
    assert_eq!(else_part, None);
}

#[test]
fn extract_block_done_splits_on_done() {
    let (body, rest) = extract_block_done("echo a; echo b done").unwrap();
    assert_eq!(body, "echo a; echo b");
    assert_eq!(rest, "");
}

#[test]
fn parse_block_lines_splits_on_newline_and_semicolon() {
    let cmds = parse_block_lines("echo a\necho b; echo c").unwrap();
    assert_eq!(cmds.len(), 3);
}

#[test]
fn parse_block_lines_skips_blank_and_comment_lines() {
    let cmds = parse_block_lines("echo a\n\n# a comment\necho b").unwrap();
    assert_eq!(cmds.len(), 2);
}

#[test]
fn parse_for_rejects_missing_in() {
    assert!(parse_for("for i echo $i done").is_err());
}

#[test]
fn parse_while_rejects_missing_do() {
    assert!(parse_while("while true echo hi").is_err());
}
