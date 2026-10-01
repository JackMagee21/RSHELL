// Tests for src/completion/mod.rs
use super::*;

#[test]
fn complete_with_empty_partial_returns_nothing() {
    assert!(complete("", true).is_empty());
    assert!(complete("", false).is_empty());
}

#[test]
fn complete_path_lists_matching_entries_in_a_directory() {
    let dir = std::env::temp_dir().join(format!("rshell_test_completion_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("alpha.txt"), "").unwrap();
    std::fs::write(dir.join("alpine.txt"), "").unwrap();
    std::fs::write(dir.join("beta.txt"), "").unwrap();
    std::fs::create_dir_all(dir.join("alsodir")).unwrap();

    let partial = format!("{}/al", dir.to_string_lossy().replace('\\', "/"));
    let mut matches = complete_path(&partial);
    matches.sort();

    assert_eq!(matches.len(), 3, "expected 3 entries starting with 'al', got {:?}", matches);
    assert!(matches.iter().any(|m| m.ends_with("alpha.txt")));
    assert!(matches.iter().any(|m| m.ends_with("alpine.txt")));
    assert!(matches.iter().any(|m| m.ends_with("alsodir/")), "directory matches should have a trailing slash");
    assert!(!matches.iter().any(|m| m.ends_with("beta.txt")));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn complete_path_on_nonexistent_dir_is_empty() {
    let partial = "/this/path/really/should/not/exist/rshell_test/prefix";
    assert!(complete_path(partial).is_empty());
}

#[test]
fn builtin_names_contains_core_builtins() {
    let names = builtin_names();
    assert!(names.contains(&"cd"));
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"exit"));
}
