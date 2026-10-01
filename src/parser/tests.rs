// Tests for src/parser/mod.rs
use super::*;

fn simple_args(cmd: &Command) -> &[String] {
    match cmd {
        Command::Simple { args, .. } => args,
        other => panic!("expected Simple, got {:?}", other),
    }
}

#[test]
fn empty_input_is_an_error() {
    assert!(parse("").is_err());
    assert!(parse("   ").is_err());
}

#[test]
fn simple_command() {
    let cmd = parse("echo hello world").unwrap();
    assert_eq!(simple_args(&cmd), &["echo", "hello", "world"]);
}

#[test]
fn background_flag_is_set_and_stripped_from_args() {
    let cmd = parse("sleep 5 &").unwrap();
    match cmd {
        Command::Simple { args, background, .. } => {
            assert!(background);
            assert_eq!(args, vec!["sleep", "5"]);
        }
        other => panic!("expected Simple, got {:?}", other),
    }
}

#[test]
fn redirects_are_parsed() {
    let cmd = parse("cmd > out.txt").unwrap();
    match cmd {
        Command::Simple { redirects, .. } => {
            assert_eq!(redirects.len(), 1);
            assert!(matches!(&redirects[0], Redirect::StdoutTo(f) if f == "out.txt"));
        }
        other => panic!("expected Simple, got {:?}", other),
    }
}

#[test]
fn redirect_without_filename_is_an_error() {
    assert!(parse("cmd >").is_err());
}

#[test]
fn pipeline_builds_nested_commands() {
    let cmd = parse("a | b | c").unwrap();
    match cmd {
        Command::Pipeline(cmds) => {
            assert_eq!(cmds.len(), 3);
            assert_eq!(simple_args(&cmds[0]), &["a"]);
            assert_eq!(simple_args(&cmds[2]), &["c"]);
        }
        other => panic!("expected Pipeline, got {:?}", other),
    }
}

#[test]
fn and_or_precedence_is_left_associative() {
    match parse("a && b || c").unwrap() {
        Command::Or(left, right) => {
            assert!(matches!(*left, Command::And(_, _)));
            assert_eq!(simple_args(&right), &["c"]);
        }
        other => panic!("expected Or, got {:?}", other),
    }
}

#[test]
fn sequence_splits_on_semicolon() {
    match parse("a; b; c").unwrap() {
        Command::Sequence(left, right) => {
            assert_eq!(simple_args(&left), &["a"]);
            match *right {
                Command::Sequence(l2, r2) => {
                    assert_eq!(simple_args(&l2), &["b"]);
                    assert_eq!(simple_args(&r2), &["c"]);
                }
                other => panic!("expected nested Sequence, got {:?}", other),
            }
        }
        other => panic!("expected Sequence, got {:?}", other),
    }
}

#[test]
fn if_then_fi_parses_condition_and_body() {
    let cmd = parse("if true; then echo yes; fi").unwrap();
    match cmd {
        Command::If { condition, body, else_body } => {
            assert_eq!(simple_args(&condition), &["true"]);
            assert_eq!(body.len(), 1);
            assert!(else_body.is_none());
        }
        other => panic!("expected If, got {:?}", other),
    }
}

#[test]
fn if_else_fi_parses_else_body() {
    let cmd = parse("if false; then echo a; else echo b; fi").unwrap();
    match cmd {
        Command::If { else_body: Some(else_body), .. } => {
            assert_eq!(else_body.len(), 1);
            assert_eq!(simple_args(&else_body[0]), &["echo", "b"]);
        }
        other => panic!("expected If with else, got {:?}", other),
    }
}

#[test]
fn if_brace_style_parses() {
    let cmd = parse("if true { echo yes }").unwrap();
    assert!(matches!(cmd, Command::If { .. }));
}

#[test]
fn for_loop_collects_items() {
    let cmd = parse("for i in a b c; do echo $i; done").unwrap();
    match cmd {
        Command::For { var, items, body } => {
            assert_eq!(var, "i");
            assert_eq!(items, vec!["a", "b", "c"]);
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected For, got {:?}", other),
    }
}

#[test]
fn while_loop_parses_condition_and_body() {
    let cmd = parse("while true; do echo hi; done").unwrap();
    match cmd {
        Command::While { condition, body } => {
            assert_eq!(simple_args(&condition), &["true"]);
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected While, got {:?}", other),
    }
}
