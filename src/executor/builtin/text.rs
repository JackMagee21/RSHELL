// src/executor/builtin/text.rs
// Text processing commands: head, tail, wc, env

pub fn builtin_head(args: &[String]) -> i32 {
    let mut lines = 10usize;
    let mut files = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-n" => {
                i += 1;
                if let Some(n) = args.get(i) {
                    lines = n.parse().unwrap_or(10);
                }
            }
            s if s.starts_with("-n") => {
                lines = s[2..].parse().unwrap_or(10);
            }
            s if s.starts_with('-') && s[1..].chars().all(|c| c.is_ascii_digit()) => {
                lines = s[1..].parse().unwrap_or(10);
            }
            _ => files.push(args[i].clone()),
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("usage: head [-n N] <file> [file2 ...]");
        return 1;
    }

    let multiple = files.len() > 1;
    let mut code = 0;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => { eprintln!("head: {}: {}", file, e); code = 1; continue; }
        };

        if multiple {
            println!("==> {} <==", file);
        }

        for line in content.lines().take(lines) {
            println!("{}", line);
        }

        if multiple { println!(); }
    }
    code
}

pub fn builtin_tail(args: &[String]) -> i32 {
    let mut lines = 10usize;
    let mut files = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-n" => {
                i += 1;
                if let Some(n) = args.get(i) {
                    lines = n.parse().unwrap_or(10);
                }
            }
            s if s.starts_with("-n") => {
                lines = s[2..].parse().unwrap_or(10);
            }
            s if s.starts_with('-') && s[1..].chars().all(|c| c.is_ascii_digit()) => {
                lines = s[1..].parse().unwrap_or(10);
            }
            _ => files.push(args[i].clone()),
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("usage: tail [-n N] <file> [file2 ...]");
        return 1;
    }

    let multiple = files.len() > 1;
    let mut code = 0;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => { eprintln!("tail: {}: {}", file, e); code = 1; continue; }
        };

        if multiple {
            println!("==> {} <==", file);
        }

        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines);
        for line in &all_lines[start..] {
            println!("{}", line);
        }

        if multiple { println!(); }
    }
    code
}

pub fn builtin_wc(args: &[String]) -> i32 {
    let mut count_lines = false;
    let mut count_words = false;
    let mut count_chars = false;
    let mut files = Vec::new();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch {
                    'l' => count_lines = true,
                    'w' => count_words = true,
                    'c' | 'm' => count_chars = true,
                    _ => {}
                }
            }
        } else {
            files.push(arg.clone());
        }
    }

    // Default: count everything if no flags given
    if !count_lines && !count_words && !count_chars {
        count_lines = true;
        count_words = true;
        count_chars = true;
    }

    if files.is_empty() {
        eprintln!("usage: wc [-lwc] <file> [file2 ...]");
        return 1;
    }

    let mut total_l = 0usize;
    let mut total_w = 0usize;
    let mut total_c = 0usize;
    let mut code = 0;
    let multiple = files.len() > 1;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => { eprintln!("wc: {}: {}", file, e); code = 1; continue; }
        };

        let l = content.lines().count();
        let w = content.split_whitespace().count();
        let c = content.chars().count();

        total_l += l;
        total_w += w;
        total_c += c;

        print_wc(l, w, c, count_lines, count_words, count_chars, file);
    }

    if multiple {
        print_wc(total_l, total_w, total_c, count_lines, count_words, count_chars, "total");
    }

    code
}

fn print_wc(l: usize, w: usize, c: usize,
            count_lines: bool, count_words: bool, count_chars: bool,
            label: &str) {
    let mut parts = Vec::new();
    if count_lines { parts.push(format!("{:>8}", l)); }
    if count_words { parts.push(format!("{:>8}", w)); }
    if count_chars { parts.push(format!("{:>8}", c)); }
    println!("{} {}", parts.join(""), label);
}

pub fn builtin_env(args: &[String]) -> i32 {
    // env with no args prints all environment variables sorted
    // env VAR=val CMD runs CMD with extra variable (basic support)
    if args.len() == 1 {
        let mut vars: Vec<(String, String)> = std::env::vars().collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in vars {
            println!("{}={}", k, v);
        }
        return 0;
    }

    // Check if any args look like VAR=val assignments
    let mut extra_vars: Vec<(String, String)> = Vec::new();
    let mut cmd_start = 1;

    for (i, arg) in args[1..].iter().enumerate() {
        if let Some((k, v)) = arg.split_once('=') {
            extra_vars.push((k.to_string(), v.to_string()));
            cmd_start = i + 2;
        } else {
            break;
        }
    }

    if cmd_start >= args.len() {
        // Just assignments, print resulting env
        let mut vars: Vec<(String, String)> = std::env::vars().collect();
        for (k, v) in &extra_vars {
            vars.retain(|(ek, _)| ek != k);
            vars.push((k.clone(), v.clone()));
        }
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in vars {
            println!("{}={}", k, v);
        }
        return 0;
    }

    // Run a command with extra env vars
    let mut cmd = std::process::Command::new(&args[cmd_start]);
    cmd.args(&args[cmd_start + 1..]);
    for (k, v) in extra_vars {
        cmd.env(k, v);
    }

    match cmd.status() {
        Ok(status) => status.code().unwrap_or(0),
        Err(e) => { eprintln!("env: {}: {}", args[cmd_start], e); 1 }
    }
}