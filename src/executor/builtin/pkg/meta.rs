// src/executor/builtin/pkg/meta.rs
//
// Thin wrapper around the per-package `meta.json` file that records the
// installed name, version, and bin entries so later commands (uninstall,
// upgrade, list) don't have to re-parse the registry.

use std::path::PathBuf;
use crate::executor::builtin::pkg::registry::BinEntry;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Meta {
    pub name:    String,
    pub version: String,
    pub bins:    Vec<BinEntry>,
}

pub fn write_meta(dir: &PathBuf, meta: &Meta) -> anyhow::Result<()> {
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(meta)?)?;
    Ok(())
}

pub fn read_meta(dir: &PathBuf) -> anyhow::Result<Meta> {
    Ok(serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json"))?)?)
}