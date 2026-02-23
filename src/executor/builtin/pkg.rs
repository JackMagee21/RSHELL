// src/executor/builtin/pkg.rs
//
// Built-in package manager for RShell.
// Downloads packages from a registry hosted on GitHub, extracts them
// into ~/.rshell/packages/<n>/, and creates shims in ~/.rshell/bin/.

use std::path::PathBuf;
use std::io::Write;

// ── Registry URL ──────────────────────────────────────────────────────────────
const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/JackMagee21/RSHELL/main/registry/registry.json";

const BAR_WIDTH   : usize = 20;
const FILLED_CHAR : &str  = "#";
const EMPTY_CHAR  : &str  = "·";
const BAR_OPEN    : &str  = "{";
const BAR_CLOSE   : &str  = "}";

// ── Public API ────────────────────────────────────────────────────────────────

pub fn builtin_pkg(args: &[String]) -> i32 {
    match args.get(1).map(|s| s.as_str()) {
        Some("install")   => cmd_install(args.get(2).map(|s| s.as_str())),
        Some("uninstall") => cmd_uninstall(args.get(2).map(|s| s.as_str())),
        Some("list")      => cmd_list(),
        Some("update")    => cmd_update(),
        Some("upgrade")   => cmd_upgrade(args.get(2).map(|s| s.as_str())),
        Some("search")    => cmd_search(args.get(2).map(|s| s.as_str())),
        _ => {
            println!("usage: pkg <command> [package]");
            println!();
            println!("commands:");
            println!("  pkg install <name>     install a package");
            println!("  pkg uninstall <name>   remove a package");
            println!("  pkg upgrade [name]     upgrade one or all packages");
            println!("  pkg list               show installed packages");
            println!("  pkg search [query]     search available packages");
            println!("  pkg update             refresh the package registry");
            1
        }
    }
}

pub fn builtin_install(args: &[String]) -> i32 {
    cmd_install(args.get(1).map(|s| s.as_str()))
}

pub fn builtin_uninstall(args: &[String]) -> i32 {
    cmd_uninstall(args.get(1).map(|s| s.as_str()))
}

// ── Commands ──────────────────────────────────────────────────────────────────

fn cmd_install(name: Option<&str>) -> i32 {
    let name = match name {
        Some(n) => n,
        None    => { eprintln!("pkg install: package name required"); return 1; }
    };

    println!("📦 Fetching registry...");
    let registry = match fetch_registry() {
        Ok(r)  => r,
        Err(e) => { eprintln!("pkg: failed to fetch registry: {}", e); return 1; }
    };

    let pkg = match registry.packages.get(name) {
        Some(p) => p,
        None    => {
            eprintln!("pkg: unknown package '{}'. Run 'pkg search' to see available packages.", name);
            return 1;
        }
    };

    let install_dir = package_dir(name);
    if install_dir.exists() {
        println!("✅ {} is already installed ({})", name, pkg.version);
        return 0;
    }

    let platform = match platform_pkg(pkg) {
        Some(p) => p,
        None    => { eprintln!("pkg: no binary available for this platform"); return 1; }
    };

    println!("⬇️  Downloading {} {}...", name, pkg.version);
    let archive = match download(&platform.url) {
        Ok(b)  => b,
        Err(e) => { eprintln!("\npkg: download failed: {}", e); return 1; }
    };

    println!("📂 Extracting...");
    if let Err(e) = extract(&archive, &platform.url, &install_dir) {
        eprintln!("\npkg: extraction failed: {}", e);
        let _ = std::fs::remove_dir_all(&install_dir);
        return 1;
    }

    let meta = Meta {
        name:    name.to_string(),
        version: pkg.version.clone(),
        bins:    platform.bins.clone(),
    };
    if let Err(e) = write_meta(&install_dir, &meta) {
        eprintln!("pkg: warning: could not write metadata: {}", e);
    }

    println!("🔗 Creating shims...");
    for bin in &platform.bins {
        if let Err(e) = create_shim(&install_dir, bin) {
            eprintln!("pkg: warning: could not create shim for {}: {}", bin.shim, e);
        }
    }

    println!("✅ Installed {} {}", name, pkg.version);

    let shim_names: Vec<&str> = platform.bins.iter()
        .map(|b| b.shim.trim_end_matches(".exe").trim_end_matches(".cmd"))
        .collect();
    println!("   Available commands: {}", shim_names.join(", "));

    if name == "zig" {
        println!();
        println!("   💡 Use Zig as a C/C++ compiler:");
        println!("      zig cc   hello.c   -o hello");
        println!("      zig c++  hello.cpp -o hello");
    }

    0
}

fn cmd_uninstall(name: Option<&str>) -> i32 {
    let name = match name {
        Some(n) => n,
        None    => { eprintln!("pkg uninstall: package name required"); return 1; }
    };

    let install_dir = package_dir(name);
    if !install_dir.exists() {
        eprintln!("pkg: {} is not installed", name);
        return 1;
    }

    // Remove shims first
    if let Ok(meta) = read_meta(&install_dir) {
        for bin in &meta.bins {
            let shim = rshell_bin_dir().join(&bin.shim);
            let _ = std::fs::remove_file(&shim);
            #[cfg(windows)]
            {
                let cmd = rshell_bin_dir().join(
                    format!("{}.cmd", bin.shim.trim_end_matches(".exe"))
                );
                let _ = std::fs::remove_file(&cmd);
            }
        }
    }

    // Count files then delete with progress bar
    let files = collect_files(&install_dir);
    let total = files.len();

    if total > 0 {
        println!("🗑️  Removing {} files...", total);
        for (i, path) in files.iter().enumerate() {
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            print_uninstall_progress(i + 1, total, filename);
            let _ = std::fs::remove_file(path);
        }
        clear_progress_line();
    }

    let _ = std::fs::remove_dir_all(&install_dir);
    println!("✅ Uninstalled {}", name);
    0
}

fn cmd_list() -> i32 {
    let packages_dir = rshell_packages_dir();
    if !packages_dir.exists() {
        println!("No packages installed.");
        return 0;
    }

    let mut entries: Vec<_> = std::fs::read_dir(&packages_dir)
        .unwrap_or_else(|_| panic!("could not read packages dir"))
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    if entries.is_empty() {
        println!("No packages installed.");
        return 0;
    }

    entries.sort_by_key(|e| e.file_name());
    println!("{:<20} {:<12} {}", "NAME", "VERSION", "COMMANDS");
    println!("{}", "-".repeat(55));

    for entry in entries {
        let name    = entry.file_name().to_string_lossy().to_string();
        let meta    = read_meta(&entry.path());
        let version = meta.as_ref().map(|m| m.version.as_str()).unwrap_or("unknown");
        let cmds    = meta.as_ref()
            .map(|m| m.bins.iter()
                .map(|b| b.shim.trim_end_matches(".exe").trim_end_matches(".cmd").to_string())
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default();
        println!("{:<20} {:<12} {}", name, version, cmds);
    }
    0
}

fn cmd_update() -> i32 {
    println!("🔄 Refreshing registry...");
    let cache = registry_cache_path();
    let _ = std::fs::remove_file(&cache);
    match fetch_registry() {
        Ok(r)  => { println!("✅ Registry updated ({} packages available)", r.packages.len()); 0 }
        Err(e) => { eprintln!("pkg: failed to update registry: {}", e); 1 }
    }
}

fn cmd_upgrade(name: Option<&str>) -> i32 {
    let registry = match fetch_registry() {
        Ok(r)  => r,
        Err(e) => { eprintln!("pkg: failed to fetch registry: {}", e); return 1; }
    };

    let packages_dir = rshell_packages_dir();
    let to_upgrade: Vec<String> = match name {
        Some(n) => vec![n.to_string()],
        None    => {
            if !packages_dir.exists() { println!("No packages installed."); return 0; }
            std::fs::read_dir(&packages_dir)
                .unwrap_or_else(|_| panic!("could not read packages dir"))
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        }
    };

    let mut upgraded = 0;
    for pkg_name in &to_upgrade {
        let install_dir = package_dir(pkg_name);
        if !install_dir.exists() { eprintln!("pkg: {} is not installed", pkg_name); continue; }

        let registry_pkg = match registry.packages.get(pkg_name.as_str()) {
            Some(p) => p,
            None    => { eprintln!("pkg: {} not found in registry", pkg_name); continue; }
        };

        let installed_version = read_meta(&install_dir).map(|m| m.version).unwrap_or_default();
        if installed_version == registry_pkg.version {
            println!("✅ {} is already up to date ({})", pkg_name, installed_version);
            continue;
        }

        println!("⬆️  Upgrading {} {} → {}...", pkg_name, installed_version, registry_pkg.version);
        cmd_uninstall(Some(pkg_name.as_str()));
        cmd_install(Some(pkg_name.as_str()));
        upgraded += 1;
    }

    if upgraded == 0 && to_upgrade.len() > 1 {
        println!("All packages are up to date.");
    }
    0
}

fn cmd_search(query: Option<&str>) -> i32 {
    let registry = match fetch_registry() {
        Ok(r)  => r,
        Err(e) => { eprintln!("pkg: failed to fetch registry: {}", e); return 1; }
    };

    let packages_dir = rshell_packages_dir();
    println!("{:<20} {:<12} {:<10} {}", "NAME", "VERSION", "STATUS", "DESCRIPTION");
    println!("{}", "-".repeat(70));

    let mut names: Vec<&String> = registry.packages.keys().collect();
    names.sort();
    let mut found = false;

    for name in names {
        let pkg = &registry.packages[name];
        if let Some(q) = query {
            if !name.contains(q) && !pkg.description.contains(q) { continue; }
        }
        let installed = packages_dir.join(name).exists();
        let status    = if installed { "installed" } else { "" };
        println!("{:<20} {:<12} {:<10} {}", name, pkg.version, status, pkg.description);
        found = true;
    }

    if !found {
        println!("No packages found matching '{}'", query.unwrap_or(""));
    }
    0
}

// ── Registry types ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Registry {
    version:  u32,
    packages: std::collections::HashMap<String, Package>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Package {
    description: String,
    version:     String,
    windows:     Option<PlatformPkg>,
    linux:       Option<PlatformPkg>,
    macos:       Option<PlatformPkg>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct PlatformPkg {
    url:  String,
    bins: Vec<BinEntry>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct BinEntry {
    path: String,
    shim: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Meta {
    name:    String,
    version: String,
    bins:    Vec<BinEntry>,
}

// ── Registry fetching ─────────────────────────────────────────────────────────

fn fetch_registry() -> anyhow::Result<Registry> {
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
    let _ = std::fs::create_dir_all(rshell_dir());
    let _ = std::fs::write(&cache, &content);
    Ok(serde_json::from_str(&content)?)
}

fn platform_pkg(pkg: &Package) -> Option<PlatformPkg> {
    #[cfg(windows)]
    return pkg.windows.clone();

    #[cfg(target_os = "macos")]
    return pkg.macos.clone().or_else(|| pkg.linux.clone());

    #[cfg(target_os = "linux")]
    return pkg.linux.clone();
}

// ── Download ──────────────────────────────────────────────────────────────────

fn download(url: &str) -> anyhow::Result<Vec<u8>> {
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

fn extract(data: &[u8], url: &str, dest: &PathBuf) -> anyhow::Result<()> {
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

fn create_shim(install_dir: &PathBuf, bin: &BinEntry) -> anyhow::Result<()> {
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

// ── Metadata ──────────────────────────────────────────────────────────────────

fn write_meta(dir: &PathBuf, meta: &Meta) -> anyhow::Result<()> {
    std::fs::write(dir.join("meta.json"), serde_json::to_string_pretty(meta)?)?;
    Ok(())
}

fn read_meta(dir: &PathBuf) -> anyhow::Result<Meta> {
    Ok(serde_json::from_str(&std::fs::read_to_string(dir.join("meta.json"))?)?)
}

// ── Progress bars ─────────────────────────────────────────────────────────────

fn make_bar(percent: usize) -> String {
    let filled = (percent * BAR_WIDTH) / 100;
    let empty  = BAR_WIDTH.saturating_sub(filled);
    format!(
        "{}{}{}{}",
        BAR_OPEN,
        FILLED_CHAR.repeat(filled),
        EMPTY_CHAR.repeat(empty),
        BAR_CLOSE,
    )
}

fn print_download_progress(downloaded: u64, total: Option<u64>) {
    match total {
        Some(t) if t > 0 => {
            let percent  = ((downloaded * 100) / t) as usize;
            let dl_mb    = downloaded as f64 / 1_048_576.0;
            let total_mb = t          as f64 / 1_048_576.0;
            print!("\r   {} {}%  {:.1}/{:.1} MB", make_bar(percent), percent, dl_mb, total_mb);
        }
        _ => {
            let dl_mb = downloaded as f64 / 1_048_576.0;
            print!("\r   ⬇️  {:.1} MB downloaded...", dl_mb);
        }
    }
    std::io::stdout().flush().ok();
}

fn print_extract_progress(current: usize, total: usize) {
    if total == 0 { return; }
    let percent = (current * 100) / total;
    print!("\r   {} {}%  ({}/{})", make_bar(percent), percent, current, total);
    std::io::stdout().flush().ok();
}

fn print_uninstall_progress(current: usize, total: usize, filename: &str) {
    if total == 0 { return; }
    let percent = (current * 100) / total;
    let name    = if filename.len() > 25 {
        format!("...{}", &filename[filename.len() - 22..])
    } else {
        filename.to_string()
    };
    print!("\r   {} {}%  {}", make_bar(percent), percent, name);
    std::io::stdout().flush().ok();
}

fn clear_progress_line() {
    print!("\r{}\r", " ".repeat(70));
    std::io::stdout().flush().ok();
}

// ── File helpers ──────────────────────────────────────────────────────────────

fn collect_files(dir: &PathBuf) -> Vec<PathBuf> {
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

// ── Paths ─────────────────────────────────────────────────────────────────────

fn rshell_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rshell")
}

pub fn rshell_bin_dir()    -> PathBuf { rshell_dir().join("bin") }
fn rshell_packages_dir()   -> PathBuf { rshell_dir().join("packages") }
fn package_dir(name: &str) -> PathBuf { rshell_packages_dir().join(name) }
fn registry_cache_path()   -> PathBuf { rshell_dir().join("registry_cache.json") }