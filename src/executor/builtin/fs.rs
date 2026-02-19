// src/executor/builtin/fs.rs
use crate::shell::Shell;
use super::util::{strip_ansi_len, format_size, color_name};

fn normalise_str(s: &str) -> String {
    let s = s.trim_start_matches("\\\\?\\");
    s.replace('\\', "/")
}

fn normalise_cwd(p: &std::path::Path) -> std::path::PathBuf {
    std::path::PathBuf::from(normalise_str(&p.display().to_string()))
}

pub fn builtin_ls(shell: &Shell, args: &[String]) -> i32 {
    let mut show_hidden = false;
    let mut long_format = false;
    let mut targets: Vec<std::path::PathBuf> = Vec::new();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch { 'a' | 'A' => show_hidden = true, 'l' => long_format = true, _ => {} }
            }
        } else {
            let joined = shell.cwd.join(arg);
            targets.push(std::path::PathBuf::from(normalise_str(&joined.display().to_string())));
        }
    }

    // Default to cwd if no targets specified
    if targets.is_empty() {
        targets.push(normalise_cwd(&shell.cwd));
    }

    let mut code = 0;
    for target in &targets {
        // If it's a plain file just print it, don't try to read_dir it
        if target.is_file() {
            let name = target.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| target.display().to_string());
            if long_format {
                if let Ok(meta) = target.metadata() {
                    println!("-  {:>10}  {}", format_size(meta.len()), color_name(&name, false, target));
                }
            } else {
                println!("{}", color_name(&name, false, target));
            }
            continue;
        }

        // Directory listing
        let entries = match std::fs::read_dir(target) {
            Ok(e) => e,
            Err(e) => { eprintln!("ls: {}: {}", target.display(), e); code = 1; continue; }
        };

        let mut items: Vec<std::fs::DirEntry> = entries.flatten()
            .filter(|e| show_hidden || !e.file_name().to_string_lossy().starts_with('.'))
            .collect();

        items.sort_by(|a, b| {
            let ad = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let bd = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (ad, bd) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        if long_format {
            for item in &items {
                let meta = match item.metadata() { Ok(m) => m, Err(_) => continue };
                let name = item.file_name().to_string_lossy().to_string();
                let is_dir = meta.is_dir();
                println!("{} {:>10}  {}",
                    if is_dir { "d" } else { "-" },
                    format_size(meta.len()),
                    color_name(&name, is_dir, &item.path())
                );
            }
            continue;
        }

        let names: Vec<String> = items.iter().map(|item| {
            let name = item.file_name().to_string_lossy().to_string();
            let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            color_name(&name, is_dir, &item.path())
        }).collect();

        let max_len = names.iter().map(|n| strip_ansi_len(n)).max().unwrap_or(0);
        let col_width = (max_len + 2).max(16);
        let cols = (80usize / col_width).max(1);

        for (i, name) in names.iter().enumerate() {
            let padding = col_width.saturating_sub(strip_ansi_len(name));
            print!("{}{}", name, " ".repeat(padding));
            if (i + 1) % cols == 0 { println!(); }
        }
        if !names.is_empty() && names.len() % cols != 0 { println!(); }
    }
    code
}

pub fn builtin_mkdir(args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("usage: mkdir [-p] <dir>"); return 1; }
    let mut parents = false;
    let mut dirs = Vec::new();
    for arg in &args[1..] {
        if arg == "-p" { parents = true; } else { dirs.push(arg); }
    }
    let mut code = 0;
    for dir in dirs {
        let result = if parents { std::fs::create_dir_all(dir) } else { std::fs::create_dir(dir) };
        match result {
            Ok(_) => println!("created {}", dir),
            Err(e) => { eprintln!("mkdir: {}: {}", dir, e); code = 1; }
        }
    }
    code
}

pub fn builtin_rm(args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("usage: rm [-rf] <file> [file2 ...]"); return 1; }
    let mut recursive = false;
    let mut force = false;
    let mut targets = Vec::new();
    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch { 'r' | 'R' => recursive = true, 'f' => force = true, _ => {} }
            }
        } else { targets.push(arg); }
    }
    let mut code = 0;
    for target in targets {
        let path = std::path::Path::new(target);
        if !path.exists() {
            if !force { eprintln!("rm: {}: no such file or directory", target); code = 1; }
            continue;
        }
        let result = if path.is_dir() {
            if recursive { std::fs::remove_dir_all(path) }
            else { eprintln!("rm: {}: is a directory (use -r to remove)", target); code = 1; continue; }
        } else { std::fs::remove_file(path) };
        if let Err(e) = result { eprintln!("rm: {}: {}", target, e); code = 1; }
    }
    code
}

pub fn builtin_cp(args: &[String]) -> i32 {
    if args.len() < 3 { eprintln!("usage: cp [-r] <source> <dest>"); return 1; }
    let mut recursive = false;
    let mut files = Vec::new();
    for arg in &args[1..] {
        if arg == "-r" || arg == "-R" || arg == "-rf" || arg == "-fr" { recursive = true; }
        else { files.push(arg.as_str()); }
    }
    if files.len() < 2 { eprintln!("cp: missing destination"); return 1; }
    let dest = std::path::Path::new(files[files.len() - 1]);
    let sources = &files[..files.len() - 1];
    let mut code = 0;
    for src in sources {
        let src_path = std::path::Path::new(src);
        if !src_path.exists() { eprintln!("cp: {}: no such file or directory", src); code = 1; continue; }
        let actual_dest = if dest.is_dir() { dest.join(src_path.file_name().unwrap_or_default()) }
                          else { dest.to_path_buf() };
        let result = if src_path.is_dir() {
            if recursive { copy_dir_all(src_path, &actual_dest) }
            else { eprintln!("cp: {}: is a directory (use -r to copy)", src); code = 1; continue; }
        } else { std::fs::copy(src_path, &actual_dest).map(|_| ()) };
        if let Err(e) = result { eprintln!("cp: {}: {}", src, e); code = 1; }
    }
    code
}

pub fn builtin_mv(args: &[String]) -> i32 {
    if args.len() < 3 { eprintln!("usage: mv <source> <dest>"); return 1; }
    let dest = std::path::Path::new(&args[args.len() - 1]);
    let mut code = 0;
    for src in &args[1..args.len() - 1] {
        let src_path = std::path::Path::new(src);
        if !src_path.exists() { eprintln!("mv: {}: no such file or directory", src); code = 1; continue; }
        let actual_dest = if dest.is_dir() { dest.join(src_path.file_name().unwrap_or_default()) }
                          else { dest.to_path_buf() };
        if let Err(e) = std::fs::rename(src_path, &actual_dest) { eprintln!("mv: {}: {}", src, e); code = 1; }
    }
    code
}

pub fn builtin_cat(args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("usage: cat <file> [file2 ...]"); return 1; }
    let mut code = 0;
    for filename in &args[1..] {
        match std::fs::read_to_string(filename) {
            Ok(contents) => print!("{}", contents),
            Err(e) => { eprintln!("cat: {}: {}", filename, e); code = 1; }
        }
    }
    code
}

pub fn builtin_touch(args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("usage: touch <file> [file2 ...]"); return 1; }
    let mut code = 0;
    for filename in &args[1..] {
        let path = std::path::Path::new(filename);
        if path.exists() {
            if let Err(e) = filetime::set_file_mtime(path, filetime::FileTime::now()) {
                eprintln!("touch: {}: {}", filename, e); code = 1;
            }
        } else {
            if let Err(e) = std::fs::File::create(path) {
                eprintln!("touch: {}: {}", filename, e); code = 1;
            }
        }
    }
    code
}

fn copy_dir_all(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() { copy_dir_all(&entry.path(), &dest_path)?; }
        else { std::fs::copy(entry.path(), dest_path)?; }
    }
    Ok(())
}