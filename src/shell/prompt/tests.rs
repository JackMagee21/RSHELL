// Tests for src/shell/prompt.rs
use super::*;

#[test]
fn shortens_long_paths_to_last_two_components() {
    assert_eq!(shorten_path("/home/user/projects/rshell"), "projects/rshell");
}

#[test]
fn leaves_short_paths_unchanged() {
    assert_eq!(shorten_path("/home"), "home");
    assert_eq!(shorten_path("/a/b"), "a/b");
}

#[test]
fn handles_root() {
    assert_eq!(shorten_path("/"), "");
}

#[test]
fn normalises_windows_separators() {
    assert_eq!(shorten_path("C:\\Users\\me\\projects\\rshell"), "projects/rshell");
}
