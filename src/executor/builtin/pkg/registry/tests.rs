// Tests for src/executor/builtin/pkg/registry.rs
use super::*;

#[test]
fn deserializes_a_minimal_registry() {
    let json = r#"{
        "version": 1,
        "packages": {
            "jq": {
                "description": "JSON processor",
                "version": "1.7",
                "windows": { "url": "https://example.com/jq-win.zip", "bins": [{"path": "jq.exe", "shim": "jq"}] },
                "linux": null,
                "macos": null
            }
        }
    }"#;
    let registry: Registry = serde_json::from_str(json).unwrap();
    assert_eq!(registry.version, 1);
    let jq = registry.packages.get("jq").unwrap();
    assert_eq!(jq.description, "JSON processor");
    assert!(jq.windows.is_some());
    assert!(jq.linux.is_none());
}

#[test]
fn the_committed_registry_json_is_valid() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("registry/registry.json");
    let content = std::fs::read_to_string(&path)
        .expect("registry/registry.json should exist in the repo");
    let registry: Registry = serde_json::from_str(&content)
        .expect("registry/registry.json should deserialize into Registry");
    assert!(!registry.packages.is_empty(), "registry should list at least one package");
    for (name, pkg) in &registry.packages {
        assert!(
            pkg.windows.is_some() || pkg.linux.is_some() || pkg.macos.is_some(),
            "package '{name}' has no platform binaries at all"
        );
    }
}
