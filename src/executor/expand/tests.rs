// Tests for src/executor/expand.rs
use super::*;
use std::collections::HashMap;
use std::path::PathBuf;

fn mk_shell() -> Shell {
    Shell {
        env: HashMap::new(),
        cwd: PathBuf::from("."),
        prev_dir: None,
        history: Vec::new(),
        aliases: HashMap::new(),
        functions: HashMap::new(),
        last_exit_code: 0,
        jobs: HashMap::new(),
        dir_stack: Vec::new(),
        exit_on_error: false,
    }
}

// ── eval_arithmetic ──────────────────────────────────────────────────────

#[test]
fn arithmetic_precedence_and_grouping() {
    assert_eq!(eval_arithmetic("2 + 3 * 4").unwrap(), 14);
    assert_eq!(eval_arithmetic("(2 + 3) * 4").unwrap(), 20);
    assert_eq!(eval_arithmetic("10 - 2 - 3").unwrap(), 5);
    assert_eq!(eval_arithmetic("2 * 3 + 4 * 5").unwrap(), 26);
}

#[test]
fn arithmetic_unary_minus() {
    assert_eq!(eval_arithmetic("-5 + 10").unwrap(), 5);
}

#[test]
fn arithmetic_division_and_modulo() {
    assert_eq!(eval_arithmetic("7 / 2").unwrap(), 3);
    assert_eq!(eval_arithmetic("7 % 2").unwrap(), 1);
}

#[test]
fn arithmetic_division_by_zero_errors() {
    assert!(eval_arithmetic("1 / 0").is_err());
    assert!(eval_arithmetic("1 % 0").is_err());
}

#[test]
fn arithmetic_rejects_garbage() {
    assert!(eval_arithmetic("abc").is_err());
}

// ── expand_arithmetic ────────────────────────────────────────────────────

#[test]
fn expand_arithmetic_substitutes_result_in_place() {
    let shell = mk_shell();
    assert_eq!(expand_arithmetic(&shell, "sum is $((2+3))!"), "sum is 5!");
}

#[test]
fn expand_arithmetic_passes_through_unmatched_open() {
    let shell = mk_shell();
    assert_eq!(expand_arithmetic(&shell, "echo $((1+2"), "echo $((1+2");
}

#[test]
fn expand_arithmetic_uses_variables() {
    let mut shell = mk_shell();
    shell.env.insert("N".to_string(), "4".to_string());
    assert_eq!(expand_arithmetic(&shell, "$(($N * 2))"), "8");
}

// ── expand_vars ──────────────────────────────────────────────────────────

#[test]
fn expand_vars_plain_and_braced() {
    let mut shell = mk_shell();
    shell.env.insert("FOO".to_string(), "bar".to_string());
    assert_eq!(expand_vars(&shell, "$FOO and ${FOO}!"), "bar and bar!");
}

#[test]
fn expand_vars_missing_var_is_empty() {
    let shell = mk_shell();
    assert_eq!(expand_vars(&shell, "[$NOPE]"), "[]");
}

#[test]
fn expand_vars_exit_code_and_pid() {
    let mut shell = mk_shell();
    shell.last_exit_code = 42;
    assert_eq!(expand_vars(&shell, "code=$?"), "code=42");
    assert_eq!(expand_vars(&shell, "pid=$$"), format!("pid={}", std::process::id()));
}

#[test]
fn expand_vars_positional_params() {
    let mut shell = mk_shell();
    shell.env.insert("1".to_string(), "one".to_string());
    shell.env.insert("2".to_string(), "two".to_string());
    assert_eq!(expand_vars(&shell, "$1 $2"), "one two");
    assert_eq!(expand_vars(&shell, "$#"), "2");
    assert_eq!(expand_vars(&shell, "$@"), "one two");
    assert_eq!(expand_vars(&shell, "$*"), "one two");
}

#[test]
fn expand_vars_bare_dollar_passes_through() {
    let shell = mk_shell();
    // '!' is not a valid variable-name char, so '$' is passed through literally.
    assert_eq!(expand_vars(&shell, "price: $!"), "price: $!");
}

#[test]
fn expand_vars_underscore_and_digits_in_name() {
    let mut shell = mk_shell();
    shell.env.insert("MY_VAR_1".to_string(), "x".to_string());
    assert_eq!(expand_vars(&shell, "$MY_VAR_1!"), "x!");
}
