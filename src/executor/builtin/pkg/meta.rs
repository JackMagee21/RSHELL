/**
 * PATH: src/executor/builtin/pkg/meta.rs
 * 
 * This file defines and manages the persistent package
 * metadata layer for RSHELL system by:
 * 
 * -> Defines a meta data model
 * -> serializes that model to disk (meta.json)
 * -> deserializes it back into memory
 */

use std::path::PathBuf;
use crate::executor::builtin::pkg::registry::BinEntry;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Meta {
    pub name:    String,
    pub version: String,
    pub bins:    Vec<BinEntry>,
}

pub fn write_meta(dir: &PathBuf, meta: &Meta) -> anyhow::Result<()> {
    let path = dir.join("meta.json");
    let contents = serde_json::to_string_pretty(meta)?;
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn read_meta(dir: &PathBuf) -> anyhow::Result<Meta> {
    let path = dir.join("meta.json");
    let contents = std::fs::read_to_string(path)?;
    let meta = serde_json::from_str(&contents)?;
    Ok(meta)
}