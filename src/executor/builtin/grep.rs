// src/executor/builtin/grep.rs
// Built-in grep — basic pattern matching in files or stdin

pub fn builtin_grep(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: grep [-rnivc] <pattern> [file ...]");
        return 1;
    }

    let mut recursive  = false;
    let mut ignore_case = false;
    let mut invert     = false;
    let mut line_nums  = false;
    let mut count_only = false;
    let mut pattern_set = false;
    let mut pattern    = String::new();
    let mut files      = Vec::new();

    for arg in &args[1..] {
        if arg.starts_with('-') && !pattern_set {
            for ch in arg.chars().skip(1) {
                match ch {
                    'r' | 'R' => recursive   = true,
                    'i'       => ignore_case = true,
                    'v'       => invert      = true,
                    'n'       => line_nums   = true,
                    'c'       => count_only  = true,
                    _         => {}
                }
            }
        } else if !pattern_set {
            pattern = arg.clone();
            pattern_set = true;
        } else {
            files.push(arg.clone());
        }
    }

    if !pattern_set {
        eprintln!("grep: missing pattern");
        return 1;
    }

    let search_pat = if ignore_case { pattern.to_lowercase() } else { pattern.clone() };

    if files.is_empty() {
        // No files — would need stdin, just print usage hint for now
        eprintln!("grep: no files specified (stdin not yet supported)");
        return 1;
    }

    let mut total_matches = 0i32;
    let multiple_files = files.len() > 1 || recursive;

    for file in &files {
        let path = std::path::Path::new(file);
        if path.is_dir() {
            if recursive {
                total_matches += grep_dir(path, &search_pat, &pattern,
                    ignore_case, invert, line_nums, count_only, multiple_files);
            } else {
                eprintln!("grep: {}: is a directory (use -r)", file);
            }
        } else {
            total_matches += grep_file(path, file, &search_pat,
                ignore_case, invert, line_nums, count_only, multiple_files);
        }
    }

    if total_matches > 0 { 0 } else { 1 }
}

fn grep_dir(
    dir: &std::path::Path,
    search_pat: &str,
    original_pat: &str,
    ignore_case: bool,
    invert: bool,
    line_nums: bool,
    count_only: bool,
    multiple_files: bool,
) -> i32 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.display().to_string();
        if path.is_dir() {
            total += grep_dir(&path, search_pat, original_pat,
                ignore_case, invert, line_nums, count_only, true);
        } else {
            total += grep_file(&path, &name, search_pat,
                ignore_case, invert, line_nums, count_only, true);
        }
    }
    total
}

fn grep_file(
    path: &std::path::Path,
    display_name: &str,
    search_pat: &str,
    ignore_case: bool,
    invert: bool,
    line_nums: bool,
    count_only: bool,
    show_filename: bool,
) -> i32 {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let mut match_count = 0;

    for (i, line) in content.lines().enumerate() {
        let compare = if ignore_case { line.to_lowercase() } else { line.to_string() };
        let matched = compare.contains(search_pat);
        let show = if invert { !matched } else { matched };

        if show {
            match_count += 1;
            if !count_only {
                // Highlight the match in the line
                let highlighted = highlight_match(line, search_pat, ignore_case);
                if show_filename && line_nums {
                    println!("\x1b[35m{}\x1b[0m:\x1b[32m{}\x1b[0m:{}", display_name, i + 1, highlighted);
                } else if show_filename {
                    println!("\x1b[35m{}\x1b[0m:{}", display_name, highlighted);
                } else if line_nums {
                    println!("\x1b[32m{}\x1b[0m:{}", i + 1, highlighted);
                } else {
                    println!("{}", highlighted);
                }
            }
        }
    }

    if count_only {
        if show_filename {
            println!("{}:{}", display_name, match_count);
        } else {
            println!("{}", match_count);
        }
    }

    match_count
}

/// Highlight matching text in red within a line
fn highlight_match(line: &str, pattern: &str, ignore_case: bool) -> String {
    let compare = if ignore_case { line.to_lowercase() } else { line.to_string() };
    let pat = if ignore_case { pattern.to_lowercase() } else { pattern.to_string() };

    let mut result = String::new();
    let mut rest = line;
    let mut compare_rest = compare.as_str();

    loop {
        match compare_rest.find(&pat) {
            None => {
                result.push_str(rest);
                break;
            }
            Some(pos) => {
                result.push_str(&rest[..pos]);
                result.push_str("\x1b[31m\x1b[1m");
                result.push_str(&rest[pos..pos + pat.len()]);
                result.push_str("\x1b[0m");
                rest = &rest[pos + pat.len()..];
                compare_rest = &compare_rest[pos + pat.len()..];
            }
        }
    }
    result
}