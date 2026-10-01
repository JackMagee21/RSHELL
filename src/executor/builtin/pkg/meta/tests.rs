// Tests for src/executor/builtin/pkg/meta.rs
use super::*;

#[test]
fn write_then_read_meta_roundtrips() {
    let dir = std::env::temp_dir().join(format!("rshell_test_meta_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let meta = Meta {
        name: "jq".to_string(),
        version: "1.7".to_string(),
        bins: vec![BinEntry { path: "bin/jq".to_string(), shim: "jq".to_string() }],
    };
    write_meta(&dir, &meta).unwrap();

    let read_back = read_meta(&dir).unwrap();
    assert_eq!(read_back.name, "jq");
    assert_eq!(read_back.version, "1.7");
    assert_eq!(read_back.bins.len(), 1);
    assert_eq!(read_back.bins[0].path, "bin/jq");
    assert_eq!(read_back.bins[0].shim, "jq");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn read_meta_missing_file_is_an_error() {
    let dir = std::env::temp_dir().join(format!("rshell_test_meta_missing_{}", std::process::id()));
    assert!(read_meta(&dir).is_err());
}
