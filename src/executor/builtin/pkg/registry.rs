// src/executor/builtin/pkg/registry.rs
//
// Registry types, remote fetching with a 1-hour local cache,
// and platform selection logic.

use std::collections::HashMap;
use crate::executor::builtin::pkg::paths::registry_cache_path;

pub const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/JackMagee21/RSHELL/main/registry/registry.json";

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Registry {
    pub version:  u32,
    pub packages: HashMap<String, Package>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Package {
    pub description: String,
    pub version:     String,
    pub windows:     Option<PlatformPkg>,
    pub linux:       Option<PlatformPkg>,
    pub macos:       Option<PlatformPkg>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PlatformPkg {
    pub url:  String,
    pub bins: Vec<BinEntry>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BinEntry {
    pub path: String,
    pub shim: String,
}

// ── Fetching ──────────────────────────────────────────────────────────────────

/// Returns the registry, using a 1-hour on-disk cache to avoid hammering the
/// remote URL on every command.
pub fn fetch_registry() -> anyhow::Result<Registry> {
    let cache = registry_cache_path();

    if let Ok(meta) = std::fs::metadata(&cache) {
        if let Ok(modified) = meta.modified() {
            if modified.elapsed().unwrap_or_default().as_secs() < 3600 {
                if let Ok(content) = std::fs::read_to_string(&cache) {
                    if let Ok(registry) = serde_json::from_str(&content) {
                        return Ok(registry);
                    }
                }
            }
        }
    }

    let content = attohttpc::get(REGISTRY_URL).send()?.text()?;
    let _ = std::fs::create_dir_all(crate::executor::builtin::pkg::paths::rshell_dir());
    let _ = std::fs::write(&cache, &content);
    Ok(serde_json::from_str(&content)?)
}

/// Returns the `PlatformPkg` appropriate for the current OS, or `None` if the
/// package has no binary for this platform.
pub fn platform_pkg(pkg: &Package) -> Option<PlatformPkg> {
    #[cfg(windows)]
    return pkg.windows.clone();

    #[cfg(target_os = "macos")]
    return pkg.macos.clone().or_else(|| pkg.linux.clone());

    #[cfg(target_os = "linux")]
    return pkg.linux.clone();
}