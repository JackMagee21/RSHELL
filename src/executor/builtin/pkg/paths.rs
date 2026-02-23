// src/executor/builtin/pkg/paths.rs
//
// Centralised path resolution for the RShell user directory layout.
//
//   ~/.rshell/
//     bin/                  shims / symlinks
//     packages/<name>/      extracted package contents
//     registry_cache.json   cached copy of the remote registry

use std::path::PathBuf;

pub fn rshell_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rshell")
}

pub fn rshell_bin_dir() -> PathBuf {
    rshell_dir().join("bin")
}

pub fn rshell_packages_dir() -> PathBuf {
    rshell_dir().join("packages")
}

pub fn package_dir(name: &str) -> PathBuf {
    rshell_packages_dir().join(name)
}

pub fn registry_cache_path() -> PathBuf {
    rshell_dir().join("registry_cache.json")
}