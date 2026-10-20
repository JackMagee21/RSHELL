// Tests for src/executor/builtin/test.rs
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

fn run(args: &[&str]) -> i32 {
    let shell = mk_shell();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    builtin_test(&shell, &args)
}

#[test]
fn string_equality() {
    assert_eq!(run(&["test", "a", "=", "a"]), 0);
    assert_eq!(run(&["test", "a", "=", "b"]), 1);
    assert_eq!(run(&["test", "a", "!=", "b"]), 0);
    assert_eq!(run(&["test", "a", "!=", "a"]), 1);
}

#[test]
fn string_empty_checks() {
    assert_eq!(run(&["test", "-z", ""]), 0);
    assert_eq!(run(&["test", "-z", "x"]), 1);
    assert_eq!(run(&["test", "-n", "x"]), 0);
    assert_eq!(run(&["test", "-n", ""]), 1);
}

#[test]
fn single_arg_is_truthy_if_nonempty() {
    assert_eq!(run(&["test", "nonempty"]), 0);
    assert_eq!(run(&["test", ""]), 1);
}

#[test]
fn no_args_is_false() {
    assert_eq!(run(&["test"]), 1);
}

#[test]
fn numeric_comparisons() {
    assert_eq!(run(&["test", "3", "-eq", "3"]), 0);
    assert_eq!(run(&["test", "3", "-ne", "4"]), 0);
    assert_eq!(run(&["test", "3", "-lt", "4"]), 0);
    assert_eq!(run(&["test", "4", "-gt", "3"]), 0);
    assert_eq!(run(&["test", "3", "-le", "3"]), 0);
    assert_eq!(run(&["test", "3", "-ge", "3"]), 0);
    assert_eq!(run(&["test", "3", "-gt", "4"]), 1);
}

#[test]
fn numeric_comparison_with_non_number_fails_closed() {
    assert_eq!(run(&["test", "abc", "-eq", "3"]), 1);
}

#[test]
fn negation_with_bang() {
    assert_eq!(run(&["test", "!", "a", "=", "b"]), 0);
    assert_eq!(run(&["test", "!", "a", "=", "a"]), 1);
}

#[test]
fn closing_bracket_is_stripped_for_test_alias() {
    assert_eq!(run(&["[", "1", "-eq", "1", "]"]), 0);
}

#[test]
fn file_and_directory_checks_use_real_filesystem() {
    let dir = std::env::temp_dir().join("rshell_test_file_checks");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("exists.txt");
    std::fs::write(&file, "data").unwrap();

    assert_eq!(run(&["test", "-f", file.to_str().unwrap()]), 0);
    assert_eq!(run(&["test", "-d", dir.to_str().unwrap()]), 0);
    assert_eq!(run(&["test", "-e", file.to_str().unwrap()]), 0);
    assert_eq!(run(&["test", "-s", file.to_str().unwrap()]), 0);
    assert_eq!(run(&["test", "-f", dir.to_str().unwrap()]), 1);
    assert_eq!(run(&["test", "-e", "/no/such/path/rshell"]), 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn expands_variables_and_arithmetic_in_operands() {
    let mut shell = mk_shell();
    shell.env.insert("N".to_string(), "5".to_string());
    let args: Vec<String> = ["test", "$N", "-eq", "$((2+3))"].iter().map(|s| s.to_string()).collect();
    assert_eq!(builtin_test(&shell, &args), 0);
}

// ── Failure cases ─────────────────────────────────────────────────────────────

#[test]
fn unknown_operator_returns_failure() {
    assert_eq!(run(&["test", "a", "-x", "b"]), 1);
}

#[test]
fn missing_operand_returns_failure() {
    assert_eq!(run(&["test", "1", "-eq"]), 1);
    assert_eq!(run(&["test", "a", "="]), 1);
}

#[test]
fn too_many_operands_returns_failure() {
    assert_eq!(run(&["test", "a", "=", "b", "c"]), 1);
}

#[test]
fn non_numeric_operand_fails_closed() {
    assert_eq!(run(&["test", "3", "-eq", "x"]), 1);
    assert_eq!(run(&["test", "3", "-lt", ""]), 1);
}

#[test]
fn file_checks_on_missing_paths_fail() {
    assert_eq!(run(&["test", "-f", "/no/such/rshell/file"]), 1);
    assert_eq!(run(&["test", "-d", "/no/such/rshell/dir"]), 1);
    assert_eq!(run(&["test", "-s", "/no/such/rshell/file"]), 1);
}

#[test]
fn size_check_fails_on_empty_file() {
    let file = std::env::temp_dir().join("rshell_test_empty_file.txt");
    std::fs::write(&file, "").unwrap();

    assert_eq!(run(&["test", "-s", file.to_str().unwrap()]), 1);
    assert_eq!(run(&["test", "-e", file.to_str().unwrap()]), 0);

    std::fs::remove_file(&file).ok();
}
