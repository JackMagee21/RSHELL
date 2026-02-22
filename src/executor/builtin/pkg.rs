// src/executor/builtin/pkg.rs
//
// Built-in package manager for RShell.
// Packages are downloaded from a registry JSON file hosted on GitHub,
// extracted into ~/.rshell/packages/<name>/, and shimmed into ~/.rshell/bin/.

use std::path::PathBuf;

// ── Registry URL ──────────────────────────────────────────────────────────────
// Change this to your own GitHub raw URL once you push registry.json
const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/YOURUSERNAME/YOURREPO/main/registry/registry.json";

// ── Public API ────────────────────────────────────────────────────────────────

pub fn builtin_pkg(args: &[String]) -> i32 {
    match args.get(1).map(|s| s.as_str()) {
        Some("install")   => cmd_install(args.get(2).map(|s| s.as_str())),
        Some("uninstall") => cmd_uninstall(args.get(2).map(|s| s.as_str())),
        Some("list")      => cmd_list(),
        Some("update")    => cmd_update(),
        Some("search")    => cmd_search(args.get(2).map(|s| s.as_str())),
        _ => {
            println!("usage: pkg <command> [package]");
            println!();
            println!("commands:");
            println!("  pkg install <name>     install a package");
            println!("  pkg uninstall <name>   remove a package");
            println!("  pkg list               show installed packages");
            println!("  pkg search [query]     search available packages");
            println!("  pkg update             refresh the package registry");
            1
        }
    }
}

// Allow `install pkgname` as shorthand for `pkg install pkgname`
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

    // Check if already installed
    let install_dir = package_dir(name);
    if install_dir.exists() {
        println!("✅ {} is already installed ({})", name, pkg.version);
        return 0;
    }

    // Get platform-specific info
    let platform = match platform_pkg(pkg) {
        Some(p) => p,
        None    => { eprintln!("pkg: no binary available for this platform"); return 1; }
    };

    println!("⬇️  Downloading {} {}...", name, pkg.version);
    let archive = match download(&platform.url) {
        Ok(b)  => b,
        Err(e) => { eprintln!("pkg: download failed: {}", e); return 1; }
    };

    println!("📂 Extracting...");
    if let Err(e) = extract(&archive, &platform.url, &install_dir) {
        eprintln!("pkg: extraction failed: {}", e);
        return 1;
    }

    // Write meta.json
    let meta = Meta {
        name:    name.to_string(),
        version: pkg.version.clone(),
        bin:     platform.bin.clone(),
        shim:    platform.shim.clone(),
    };
    if let Err(e) = write_meta(&install_dir, &meta) {
        eprintln!("pkg: warning: could not write metadata: {}", e);
    }

    // Create shim in ~/.rshell/bin/
    println!("🔗 Creating shim...");
    if let Err(e) = create_shim(name, &install_dir, &platform) {
        eprintln!("pkg: warning: could not create shim: {}", e);
    }

    println!("✅ Installed {} {}", name, pkg.version);
    println!("   Run '{}' to use it", platform.shim.trim_end_matches(".exe"));
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

    // Read meta to find the shim name
    if let Ok(meta) = read_meta(&install_dir) {
        let shim_path = rshell_bin_dir().join(&meta.shim);
        let _ = std::fs::remove_file(&shim_path);
    }

    if let Err(e) = std::fs::remove_dir_all(&install_dir) {
        eprintln!("pkg: failed to remove {}: {}", name, e);
        return 1;
    }

    println!("🗑️  Uninstalled {}", name);
    0
}

fn cmd_list() -> i32 {
    let packages_dir = rshell_packages_dir();
    if !packages_dir.exists() {
        println!("No packages installed.");
        return 0;
    }

    let entries: Vec<_> = std::fs::read_dir(&packages_dir)
        .unwrap_or_else(|_| panic!("could not read packages dir"))
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();

    if entries.is_empty() {
        println!("No packages installed.");
        return 0;
    }

    println!("Installed packages:");
    println!("{:<20} {}", "NAME", "VERSION");
    println!("{}", "-".repeat(30));

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let version = read_meta(&entry.path())
            .map(|m| m.version)
            .unwrap_or_else(|_| "unknown".to_string());
        println!("{:<20} {}", name, version);
    }
    0
}

fn cmd_update() -> i32 {
    println!("🔄 Refreshing registry...");
    match fetch_registry() {
        Ok(r) => {
            // Cache the registry locally
            let cache = registry_cache_path();
            if let Ok(json) = serde_json::to_string_pretty(&r) {
                let _ = std::fs::write(&cache, json);
            }
            println!("✅ Registry updated ({} packages available)", r.packages.len());
            0
        }
        Err(e) => { eprintln!("pkg: failed to update registry: {}", e); 1 }
    }
}

fn cmd_search(query: Option<&str>) -> i32 {
    let registry = match fetch_registry() {
        Ok(r)  => r,
        Err(e) => { eprintln!("pkg: failed to fetch registry: {}", e); return 1; }
    };

    let packages_dir = rshell_packages_dir();

    println!("{:<20} {:<12} {:<8} {}", "NAME", "VERSION", "STATUS", "DESCRIPTION");
    println!("{}", "-".repeat(65));

    let mut found = false;
    let mut names: Vec<&String> = registry.packages.keys().collect();
    names.sort();

    for name in names {
        let pkg = &registry.packages[name];

        // Filter by query if provided
        if let Some(q) = query {
            if !name.contains(q) && !pkg.description.contains(q) {
                continue;
            }
        }

        let installed = packages_dir.join(name).exists();
        let status    = if installed { "installed" } else { "" };

        println!("{:<20} {:<12} {:<8} {}",
            name, pkg.version, status, pkg.description);
        found = true;
    }

    if !found {
        println!("No packages found matching '{}'", query.unwrap_or(""));
    }
    0
}

// ── Registry ──────────────────────────────────────────────────────────────────

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
    url:  String,  // download URL
    bin:  String,  // path inside archive to the binary
    shim: String,  // name of the shim to create in ~/.rshell/bin/
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Meta {
    name:    String,
    version: String,
    bin:     String,
    shim:    String,
}

fn fetch_registry() -> anyhow::Result<Registry> {
    // Try cache first (< 1 hour old)
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

    // Fetch from network
    let content = ureq::get(REGISTRY_URL)
        .call()?
        .into_string()?;

    // Cache it
    let _ = std::fs::create_dir_all(cache.parent().unwrap_or(&cache));
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
    let response = ureq::get(url).call()?;
    let mut bytes = Vec::new();
    use std::io::Read;
    response.into_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

// ── Extraction ────────────────────────────────────────────────────────────────

fn extract(data: &[u8], url: &str, dest: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest)?;

    if url.ends_with(".zip") {
        extract_zip(data, dest)
    } else if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        extract_tar_gz(data, dest)
    } else {
        anyhow::bail!("unsupported archive format: {}", url);
    }
}

fn extract_zip(data: &[u8], dest: &PathBuf) -> anyhow::Result<()> {
    use std::io::Cursor;
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let out_path = dest.join(file.name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }
    Ok(())
}

fn extract_tar_gz(data: &[u8], dest: &PathBuf) -> anyhow::Result<()> {
    use std::io::Cursor;
    let cursor     = Cursor::new(data);
    let gz         = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(dest)?;
    Ok(())
}

// ── Shims ─────────────────────────────────────────────────────────────────────

fn create_shim(name: &str, install_dir: &PathBuf, platform: &PlatformPkg) -> anyhow::Result<()> {
    let bin_dir = rshell_bin_dir();
    std::fs::create_dir_all(&bin_dir)?;

    let actual_bin = install_dir.join(&platform.bin);
    let shim_path  = bin_dir.join(&platform.shim);

    #[cfg(unix)]
    {
        // Make the binary executable
        use std::os::unix::fs::PermissionsExt;
        if actual_bin.exists() {
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&actual_bin, perms)?;
        }

        // Create a symlink shim
        if shim_path.exists() { std::fs::remove_file(&shim_path)?; }
        std::os::unix::fs::symlink(&actual_bin, &shim_path)?;
    }

    #[cfg(windows)]
    {
        // On Windows create a small .cmd wrapper
        let cmd_shim = bin_dir.join(format!("{}.cmd", name));
        let actual   = actual_bin.display().to_string();
        let content  = format!("@echo off\n\"{}\" %*\n", actual);
        std::fs::write(&cmd_shim, content)?;

        // Also try a direct copy as fallback
        if actual_bin.exists() && !shim_path.exists() {
            let _ = std::fs::copy(&actual_bin, &shim_path);
        }
    }

    Ok(())
}

// ── Metadata ──────────────────────────────────────────────────────────────────

fn write_meta(dir: &PathBuf, meta: &Meta) -> anyhow::Result<()> {
    let path    = dir.join("meta.json");
    let content = serde_json::to_string_pretty(meta)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn read_meta(dir: &PathBuf) -> anyhow::Result<Meta> {
    let path    = dir.join("meta.json");
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

// ── Paths ─────────────────────────────────────────────────────────────────────

/// ~/.rshell/
fn rshell_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rshell")
}

/// ~/.rshell/bin/  — added to PATH on shell startup
pub fn rshell_bin_dir() -> PathBuf {
    rshell_dir().join("bin")
}

/// ~/.rshell/packages/
fn rshell_packages_dir() -> PathBuf {
    rshell_dir().join("packages")
}

/// ~/.rshell/packages/<name>/
fn package_dir(name: &str) -> PathBuf {
    rshell_packages_dir().join(name)
}

/// ~/.rshell/registry_cache.json
fn registry_cache_path() -> PathBuf {
    rshell_dir().join("registry_cache.json")
}