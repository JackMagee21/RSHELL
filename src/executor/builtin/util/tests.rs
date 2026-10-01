// Tests for src/executor/builtin/util.rs
use super::*;

#[test]
fn strip_ansi_len_counts_visible_chars_only() {
    assert_eq!(strip_ansi_len("hello"), 5);
    assert_eq!(strip_ansi_len("\x1b[31mhello\x1b[0m"), 5);
    assert_eq!(strip_ansi_len(""), 0);
}

#[test]
fn format_size_picks_the_right_unit() {
    assert_eq!(format_size(500), "500B");
    assert_eq!(format_size(2048), "2.0K");
    assert_eq!(format_size(5 * 1_048_576), "5.0M");
    assert_eq!(format_size(3 * 1_073_741_824), "3.0G");
}

#[test]
fn levenshtein_identical_strings_is_zero() {
    assert_eq!(levenshtein("ls", "ls"), 0);
}

#[test]
fn levenshtein_counts_edits() {
    assert_eq!(levenshtein("kitten", "sitting"), 3);
    assert_eq!(levenshtein("gti", "git"), 2);
    assert_eq!(levenshtein("", "abc"), 3);
}

#[test]
fn color_name_wraps_directories_in_blue() {
    let colored = color_name("src", true, std::path::Path::new("src"));
    assert_eq!(colored, "\x1b[34msrc/\x1b[0m");
}
