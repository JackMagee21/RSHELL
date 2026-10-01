// Tests for src/executor/mod.rs
use super::*;

fn tmp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rshell_test_{}_{}", name, std::process::id()))
}

#[test]
fn echo_redirect_overwrites_file() {
    let path = tmp_path("echo_overwrite");
    let args = vec!["echo".to_string(), "hello".to_string(), "world".to_string()];
    let redirects = vec![Redirect::StdoutTo(path.to_string_lossy().to_string())];
    let code = run_echo_redirect(&args, &redirects).unwrap();
    assert_eq!(code, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world\n");
    std::fs::remove_file(&path).ok();
}

#[test]
fn echo_redirect_dash_n_omits_trailing_newline() {
    let path = tmp_path("echo_no_newline");
    let args = vec!["echo".to_string(), "-n".to_string(), "hi".to_string()];
    let redirects = vec![Redirect::StdoutTo(path.to_string_lossy().to_string())];
    run_echo_redirect(&args, &redirects).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hi");
    std::fs::remove_file(&path).ok();
}

#[test]
fn echo_redirect_append_adds_to_existing_file() {
    let path = tmp_path("echo_append");
    std::fs::write(&path, "first\n").unwrap();
    let args = vec!["echo".to_string(), "second".to_string()];
    let redirects = vec![Redirect::StdoutAppend(path.to_string_lossy().to_string())];
    run_echo_redirect(&args, &redirects).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first\nsecond\n");
    std::fs::remove_file(&path).ok();
}

#[test]
fn echo_redirect_expands_escape_sequences() {
    let path = tmp_path("echo_escapes");
    let args = vec!["echo".to_string(), r"a\nb\tc".to_string()];
    let redirects = vec![Redirect::StdoutTo(path.to_string_lossy().to_string())];
    run_echo_redirect(&args, &redirects).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\tc\n");
    std::fs::remove_file(&path).ok();
}

#[test]
fn is_builtin_cmd_recognises_builtins_and_rejects_unknowns() {
    assert!(super::pipeline::is_builtin_cmd("echo"));
    assert!(super::pipeline::is_builtin_cmd("grep"));
    assert!(!super::pipeline::is_builtin_cmd("totally-not-a-command"));
}
