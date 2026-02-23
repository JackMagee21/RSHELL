// src/executor/builtin/pkg/install.rs
//
// Low-level installation mechanics: HTTP download, archive extraction,
// shim/symlink creation, and filesystem helpers used during uninstall.

use std::io::Write;
use std::path::PathBuf;

use crate::executor::builtin::pkg::{
    paths::rshell_bin_dir,
    progress::{clear_progress_line, print_download_progress, print_extract_progress},
    registry::BinEntry,
};

// ── Download ──────────────────────────────────────────────────────────────────

pub fn download(url: &str) -> anyhow::Result<Vec<u8>> {
    let response = attohttpc::get(url).send()?;

    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let mut reader     = response;
    let mut buf        = Vec::new();
    let mut downloaded = 0u64;
    let mut chunk      = [0u8; 8192];

    use std::io::Read;
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 { break; }
        buf.extend_from_slice(&chunk[..n]);
        downloaded += n as u64;
        print_download_progress(downloaded, total);
    }
    clear_progress_line();
    Ok(buf)
}

// ── Extraction ────────────────────────────────────────────────────────────────

pub fn extract(data: &[u8], url: &str, dest: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;
    if url.ends_with(".zip") {
        extract_zip(data, dest)
    } else if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        extract_tar_gz(data, dest)
    } else if url.ends_with(".tar.xz") {
        extract_tar_xz(data, dest)
    } else if url.ends_with(".exe") {
        let filename = url.split('/').last().unwrap_or("bin.exe");
        std::fs::write(dest.join(filename), data)?;
        Ok(())
    } else {
        let filename = url.split('/').last().unwrap_or("bin");
        std::fs::write(dest.join(filename), data)?;
        Ok(())
    }
}

fn extract_zip(data: &[u8], dest: &PathBuf) -> anyhow::Result<()> {
    use std::io::Cursor;
    let mut archive = zip::ZipArchive::new(Cursor::new(data))?;
    let total       = archive.len();
    for i in 0..total {
        let mut file     = archive.by_index(i)?;
        let out_path = dest.join(file.name());
        print_extract_progress(i + 1, total);
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(p) = out_path.parent() { std::fs::create_dir_all(p)?; }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }
    clear_progress_line();
    Ok(())
}

fn extract_tar_gz(data: &[u8], dest: &PathBuf) -> anyhow::Result<()> {
    use std::io::Cursor;
    let gz      = flate2::read::GzDecoder::new(Cursor::new(data));
    let mut tar = tar::Archive::new(gz);
    unpack_tar_with_progress(&mut tar, dest)
}

fn extract_tar_xz(data: &[u8], dest: &PathBuf) -> anyhow::Result<()> {
    use std::io::Cursor;
    let xz      = xz2::read::XzDecoder::new(Cursor::new(data));
    let mut tar = tar::Archive::new(xz);
    unpack_tar_with_progress(&mut tar, dest)
}

fn unpack_tar_with_progress<R: std::io::Read>(
    tar: &mut tar::Archive<R>,
    dest: &PathBuf,
) -> anyhow::Result<()> {
    let mut count = 0usize;
    for entry in tar.entries()? {
        let mut entry = entry?;
        entry.unpack_in(dest)?;
        count += 1;
        print!("\r   {} files extracted...", count);
        std::io::stdout().flush().ok();
    }
    clear_progress_line();
    Ok(())
}

// ── Shims ─────────────────────────────────────────────────────────────────────

pub fn create_shim(install_dir: &PathBuf, bin: &BinEntry) -> anyhow::Result<()> {
    let bin_dir    = rshell_bin_dir();
    std::fs::create_dir_all(&bin_dir)?;
    let actual_bin = install_dir.join(&bin.path);
    let shim_path  = bin_dir.join(&bin.shim);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if actual_bin.exists() {
            std::fs::set_permissions(&actual_bin, std::fs::Permissions::from_mode(0o755))?;
        }
        if shim_path.exists() { std::fs::remove_file(&shim_path)?; }
        std::os::unix::fs::symlink(&actual_bin, &shim_path)?;
    }

    #[cfg(windows)]
    {
        let stem     = bin.shim.trim_end_matches(".exe").trim_end_matches(".cmd");
        let cmd_shim = bin_dir.join(format!("{}.cmd", stem));
        let content  = format!("@echo off\n\"{}\" %*\n", actual_bin.display());
        std::fs::write(&cmd_shim, &content)?;
        if actual_bin.exists() && !shim_path.exists() {
            let _ = std::fs::copy(&actual_bin, &shim_path);
        }
    }

    Ok(())
}

// ── Filesystem helpers ────────────────────────────────────────────────────────

/// Recursively collects all files (not directories) under `dir`.
pub fn collect_files(dir: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_files(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}