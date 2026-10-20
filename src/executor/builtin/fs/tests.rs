// Tests for src/executor/builtin/fs.rs
//
// parse_chmod_mode is #[cfg(unix)] only, so the chmod tests are too.
// The rest run on every platform.
use super::*;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// A fresh, empty scratch directory under the system temp dir.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rshell_fs_test_{}", name));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
#[test]
fn octal_mode_parses_directly() {
    assert_eq!(parse_chmod_mode("755", 0), Some(0o755));
    assert_eq!(parse_chmod_mode("644", 0), Some(0o644));
}

#[cfg(unix)]
#[test]
fn symbolic_add_permission_for_all() {
    assert_eq!(parse_chmod_mode("+x", 0o644), Some(0o755));
}

#[cfg(unix)]
#[test]
fn symbolic_remove_permission_for_user() {
    assert_eq!(parse_chmod_mode("u-w", 0o644), Some(0o444));
}

#[cfg(unix)]
#[test]
fn symbolic_set_permission_with_equals() {
    assert_eq!(parse_chmod_mode("go=r", 0o777), Some(0o744));
}

#[cfg(unix)]
#[test]
fn symbolic_multiple_who_targets() {
    // owner and group both gain execute; other is untouched
    assert_eq!(parse_chmod_mode("ug+x", 0o644), Some(0o754));
}

#[cfg(unix)]
#[test]
fn symbolic_comma_separated_clauses() {
    // u+x: 0o644 -> 0o744 (owner gains execute)
    // g-r: 0o744 -> 0o704 (group loses read)
    assert_eq!(parse_chmod_mode("u+x,g-r", 0o644), Some(0o704));
}

#[cfg(unix)]
#[test]
fn symbolic_invalid_permission_char_is_none() {
    assert_eq!(parse_chmod_mode("+z", 0o644), None);
}

#[cfg(unix)]
#[test]
fn symbolic_missing_operator_is_none() {
    assert_eq!(parse_chmod_mode("u", 0o644), None);
}

// ── Failure cases ─────────────────────────────────────────────────────────────

#[test]
fn missing_arguments_print_usage_and_fail() {
    assert_eq!(builtin_mkdir(&args(&["mkdir"])), 1);
    assert_eq!(builtin_rm(&args(&["rm"])), 1);
    assert_eq!(builtin_cp(&args(&["cp", "a"])), 1);
    assert_eq!(builtin_mv(&args(&["mv", "a"])), 1);
    assert_eq!(builtin_cat(&args(&["cat"])), 1);
    assert_eq!(builtin_touch(&args(&["touch"])), 1);
}

#[test]
fn mkdir_existing_dir_fails() {
    let dir = scratch("mkdir_existing");
    assert_eq!(builtin_mkdir(&args(&["mkdir", dir.to_str().unwrap()])), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rm_missing_file_fails_unless_forced() {
    let dir = scratch("rm_missing");
    let missing = dir.join("nope.txt");
    let m = missing.to_str().unwrap();

    assert_eq!(builtin_rm(&args(&["rm", m])), 1);
    assert_eq!(builtin_rm(&args(&["rm", "-f", m])), 0);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rm_directory_without_recursive_flag_fails_and_keeps_it() {
    let dir = scratch("rm_dir");
    assert_eq!(builtin_rm(&args(&["rm", dir.to_str().unwrap()])), 1);
    assert!(dir.exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cp_missing_source_fails_and_writes_nothing() {
    let dir = scratch("cp_missing");
    let src = dir.join("nope.txt");
    let dest = dir.join("out.txt");

    assert_eq!(builtin_cp(&args(&["cp", src.to_str().unwrap(), dest.to_str().unwrap()])), 1);
    assert!(!dest.exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cp_directory_without_recursive_flag_fails() {
    let dir = scratch("cp_dir");
    let src = dir.join("srcdir");
    let dest = dir.join("dest");
    std::fs::create_dir_all(&src).unwrap();

    assert_eq!(builtin_cp(&args(&["cp", src.to_str().unwrap(), dest.to_str().unwrap()])), 1);
    assert!(!dest.exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn mv_missing_source_fails_and_writes_nothing() {
    let dir = scratch("mv_missing");
    let src = dir.join("nope.txt");
    let dest = dir.join("out.txt");

    assert_eq!(builtin_mv(&args(&["mv", src.to_str().unwrap(), dest.to_str().unwrap()])), 1);
    assert!(!dest.exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cat_missing_file_fails() {
    let dir = scratch("cat_missing");
    let missing = dir.join("nope.txt");
    assert_eq!(builtin_cat(&args(&["cat", missing.to_str().unwrap()])), 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cat_with_one_missing_file_fails_overall() {
    let dir = scratch("cat_partial");
    let good = dir.join("good.txt");
    let missing = dir.join("nope.txt");
    std::fs::write(&good, "hi").unwrap();

    assert_eq!(builtin_cat(&args(&["cat", good.to_str().unwrap(), missing.to_str().unwrap()])), 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn touch_into_missing_directory_fails() {
    let dir = scratch("touch_missing");
    let path = dir.join("no_such_dir").join("f.txt");
    assert_eq!(builtin_touch(&args(&["touch", path.to_str().unwrap()])), 1);

    std::fs::remove_dir_all(&dir).ok();
}
