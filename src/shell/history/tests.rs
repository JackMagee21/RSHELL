// Tests for src/shell/history.rs
use super::*;
use std::collections::HashMap;
use std::path::PathBuf;

fn mk_shell(history: Vec<&str>) -> Shell {
    Shell {
        env: HashMap::new(),
        cwd: PathBuf::from("."),
        prev_dir: None,
        history: history.into_iter().map(String::from).collect(),
        aliases: HashMap::new(),
        functions: HashMap::new(),
        last_exit_code: 0,
        jobs: HashMap::new(),
        dir_stack: Vec::new(),
        exit_on_error: false,
    }
}

#[test]
fn bang_bang_repeats_last_command() {
    let shell = mk_shell(vec!["echo one", "echo two"]);
    assert_eq!(shell.expand_history("!!"), "echo two");
}

#[test]
fn bang_bang_with_suffix_appends_args() {
    let shell = mk_shell(vec!["echo one"]);
    assert_eq!(shell.expand_history("!! world"), "echo one world");
}

#[test]
fn bang_bang_on_empty_history_is_unchanged() {
    let shell = mk_shell(vec![]);
    assert_eq!(shell.expand_history("!!"), "!!");
}

#[test]
fn bang_n_repeats_nth_command() {
    let shell = mk_shell(vec!["echo one", "echo two", "echo three"]);
    assert_eq!(shell.expand_history("!2"), "echo two");
}

#[test]
fn bang_n_out_of_range_is_unchanged() {
    let shell = mk_shell(vec!["echo one"]);
    assert_eq!(shell.expand_history("!99"), "!99");
}

#[test]
fn plain_input_passes_through_unchanged() {
    let shell = mk_shell(vec!["echo one"]);
    assert_eq!(shell.expand_history("echo hi"), "echo hi");
}
