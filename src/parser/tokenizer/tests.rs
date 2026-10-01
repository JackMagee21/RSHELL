// Tests for src/parser/tokenizer.rs
use super::*;

fn words(input: &str) -> Vec<String> {
    tokenize(input).unwrap().into_iter().map(|t| match t {
        Token::Word(w) => w,
        other => panic!("expected Word, got {:?}", other),
    }).collect()
}

#[test]
fn plain_words_split_on_whitespace() {
    assert_eq!(words("echo hello world"), vec!["echo", "hello", "world"]);
}

#[test]
fn single_quotes_are_literal() {
    assert_eq!(words("echo 'a b  c'"), vec!["echo", "a b  c"]);
}

#[test]
fn double_quotes_handle_escapes() {
    assert_eq!(words(r#"echo "a \"b\" c""#), vec!["echo", "a \"b\" c"]);
}

#[test]
fn backslash_escapes_single_char_in_unquoted_word() {
    assert_eq!(words(r"echo foo\ bar"), vec!["echo", "foo bar"]);
}

#[test]
fn comment_truncates_rest_of_line() {
    assert_eq!(words("echo hi # this is a comment"), vec!["echo", "hi"]);
}

#[test]
fn line_continuation_between_words_is_dropped() {
    // A '\' that starts a fresh token (i.e. after whitespace) triggers the
    // top-level line-continuation handling and disappears along with the
    // newline, leaving "a" and "b" as separate words.
    assert_eq!(words("echo a \\\nb"), vec!["echo", "a", "b"]);
}

#[test]
fn backslash_newline_inside_a_word_is_a_literal_escape() {
    // A '\' that appears *inside* a word is handled by read_word()'s own
    // escape logic, which just copies the next char verbatim — including a
    // newline. Line continuation only applies at word boundaries.
    assert_eq!(words("echo a\\\nb"), vec!["echo", "a\nb"]);
}

#[test]
fn pipe_vs_or() {
    let t = tokenize("a | b || c").unwrap();
    assert_eq!(t, vec![
        Token::Word("a".into()), Token::Pipe,
        Token::Word("b".into()), Token::Or,
        Token::Word("c".into()),
    ]);
}

#[test]
fn ampersand_vs_and() {
    let t = tokenize("a & b && c").unwrap();
    assert_eq!(t, vec![
        Token::Word("a".into()), Token::Ampersand,
        Token::Word("b".into()), Token::And,
        Token::Word("c".into()),
    ]);
}

#[test]
fn redirects() {
    let t = tokenize("a > b >> c < d").unwrap();
    assert_eq!(t, vec![
        Token::Word("a".into()), Token::RedirectOut, Token::Word("b".into()),
        Token::RedirectAppend, Token::Word("c".into()),
        Token::RedirectIn, Token::Word("d".into()),
    ]);
}

#[test]
fn stderr_redirects() {
    let t = tokenize("a 2> err 2>&1").unwrap();
    assert_eq!(t, vec![
        Token::Word("a".into()), Token::RedirectErr, Token::Word("err".into()),
        Token::RedirectErrOut,
    ]);
}

#[test]
fn digit_two_not_followed_by_redirect_is_a_word() {
    assert_eq!(words("echo 2plus2"), vec!["echo", "2plus2"]);
}

#[test]
fn semicolons_are_separate_tokens() {
    let t = tokenize("a;b").unwrap();
    assert_eq!(t, vec![
        Token::Word("a".into()), Token::Semicolon, Token::Word("b".into()),
    ]);
}

#[test]
fn tilde_expands_to_home_dir() {
    let t = tokenize("~").unwrap();
    let home = dirs::home_dir().unwrap().display().to_string();
    assert_eq!(t, vec![Token::Word(home)]);
}
