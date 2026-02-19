// src/executor/builtin/find.rs
// Basic find command: find [dir] [-name pattern] [-type f/d] [-maxdepth N]

pub fn builtin_find(args: &[String]) -> i32 {
    let mut start_dir = ".".to_string();
    let mut name_pat: Option<String> = None;
    let mut file_type: Option<char> = None; // 'f' = file, 'd' = dir
    let mut max_depth: Option<usize> = None;
    let mut min_depth: Option<usize> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-name" => {
                i += 1;
                if let Some(pat) = args.get(i) {
                    name_pat = Some(pat.clone());
                }
            }
            "-type" => {
                i += 1;
                if let Some(t) = args.get(i) {
                    file_type = t.chars().next();
                }
            }
            "-maxdepth" => {
                i += 1;
                if let Some(n) = args.get(i) {
                    max_depth = n.parse().ok();
                }
            }
            "-mindepth" => {
                i += 1;
                if let Some(n) = args.get(i) {
                    min_depth = n.parse().ok();
                }
            }
            s if !s.starts_with('-') && i == 1 => {
                start_dir = s.to_string();
            }
            unknown => {
                eprintln!("find: unknown option: {}", unknown);
                return 1;
            }
        }
        i += 1;
    }

    let path = std::path::Path::new(&start_dir);
    if !path.exists() {
        eprintln!("find: {}: no such file or directory", start_dir);
        return 1;
    }

    let mut results: Vec<String> = Vec::new();
    walk_find(
        path,
        &name_pat,
        file_type,
        max_depth,
        min_depth,
        0,
        &mut results,
    );

    for r in &results {
        println!("{}", r);
    }

    if results.is_empty() { 1 } else { 0 }
}

fn walk_find(
    dir: &std::path::Path,
    name_pat: &Option<String>,
    file_type: Option<char>,
    max_depth: Option<usize>,
    min_depth: Option<usize>,
    depth: usize,
    results: &mut Vec<String>,
) {
    // Check depth limits
    if let Some(max) = max_depth {
        if depth > max { return; }
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = path.is_dir();
        let name = entry.file_name().to_string_lossy().to_string();

        // Check type filter
        let type_ok = match file_type {
            Some('f') => !is_dir,
            Some('d') => is_dir,
            Some('l') => path.symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false),
            _ => true,
        };

        // Check name pattern
        let name_ok = match name_pat {
            Some(pat) => crate::glob::matches_pattern(&name, pat),
            None => true,
        };

        // Check mindepth
        let depth_ok = match min_depth {
            Some(min) => depth + 1 >= min,
            None => true,
        };

        if type_ok && name_ok && depth_ok {
            // Normalise path separators
            let display = path.display().to_string().replace('\\', "/");
            // Strip leading ./ for cleaner output
            let display = display.strip_prefix("./").unwrap_or(&display).to_string();
            results.push(display);
        }

        // Recurse into directories
        if is_dir {
            if let Some(max) = max_depth {
                if depth + 1 > max { continue; }
            }
            walk_find(&path, name_pat, file_type, max_depth, min_depth, depth + 1, results);
        }
    }
}