// src/executor/builtin/pkg/mod.rs
//
// Built-in package manager for RShell.
// Downloads packages from a registry hosted on GitHub, extracts them
// into ~/.rshell/packages/<n>/, and creates shims in ~/.rshell/bin/.
//
// Public surface:
//   builtin_pkg()        — `pkg <subcommand>` entry point
//   builtin_install()    — `install <name>` shorthand
//   builtin_uninstall()  — `uninstall <name>` shorthand
//   rshell_bin_dir()     — re-exported for the shell's PATH resolution

mod install;
mod meta;
mod paths;
mod progress;
mod registry;

pub use paths::rshell_bin_dir;

use install::{collect_files, create_shim, download, extract};
use meta::{read_meta, write_meta, Meta};
use paths::{package_dir, rshell_packages_dir};
use progress::{clear_progress_line, print_uninstall_progress};
use registry::{fetch_registry, platform_pkg};

// ── Public entry points ───────────────────────────────────────────────────────

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
    let cache = paths::registry_cache_path();
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