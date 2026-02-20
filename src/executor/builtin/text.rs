// src/executor/builtin/text.rs
// Text processing commands: head, tail, wc, env, sort, uniq, xargs

pub fn builtin_head(args: &[String]) -> i32 {
    let mut lines = 10usize;
    let mut files = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-n" => { i += 1; if let Some(n) = args.get(i) { lines = n.parse().unwrap_or(10); } }
            s if s.starts_with("-n") => { lines = s[2..].parse().unwrap_or(10); }
            s if s.starts_with('-') && s[1..].chars().all(|c| c.is_ascii_digit()) => { lines = s[1..].parse().unwrap_or(10); }
            _ => files.push(args[i].clone()),
        }
        i += 1;
    }

    if files.is_empty() { eprintln!("usage: head [-n N] <file> [file2 ...]"); return 1; }
    let multiple = files.len() > 1;
    let mut code = 0;
    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c, Err(e) => { eprintln!("head: {}: {}", file, e); code = 1; continue; }
        };
        if multiple { println!("==> {} <==", file); }
        for line in content.lines().take(lines) { println!("{}", line); }
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
            "-n" => { i += 1; if let Some(n) = args.get(i) { lines = n.parse().unwrap_or(10); } }
            s if s.starts_with("-n") => { lines = s[2..].parse().unwrap_or(10); }
            s if s.starts_with('-') && s[1..].chars().all(|c| c.is_ascii_digit()) => { lines = s[1..].parse().unwrap_or(10); }
            _ => files.push(args[i].clone()),
        }
        i += 1;
    }

    if files.is_empty() { eprintln!("usage: tail [-n N] <file> [file2 ...]"); return 1; }
    let multiple = files.len() > 1;
    let mut code = 0;
    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c, Err(e) => { eprintln!("tail: {}: {}", file, e); code = 1; continue; }
        };
        if multiple { println!("==> {} <==", file); }
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines);
        for line in &all_lines[start..] { println!("{}", line); }
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
                match ch { 'l' => count_lines = true, 'w' => count_words = true, 'c'|'m' => count_chars = true, _ => {} }
            }
        } else { files.push(arg.clone()); }
    }

    if !count_lines && !count_words && !count_chars {
        count_lines = true; count_words = true; count_chars = true;
    }

    if files.is_empty() { eprintln!("usage: wc [-lwc] <file> [file2 ...]"); return 1; }

    let mut total_l = 0usize;
    let mut total_w = 0usize;
    let mut total_c = 0usize;
    let mut code = 0;
    let multiple = files.len() > 1;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c, Err(e) => { eprintln!("wc: {}: {}", file, e); code = 1; continue; }
        };
        let l = content.lines().count();
        let w = content.split_whitespace().count();
        let c = content.chars().count();
        total_l += l; total_w += w; total_c += c;
        print_wc(l, w, c, count_lines, count_words, count_chars, file);
    }
    if multiple { print_wc(total_l, total_w, total_c, count_lines, count_words, count_chars, "total"); }
    code
}

fn print_wc(l: usize, w: usize, c: usize, cl: bool, cw: bool, cc: bool, label: &str) {
    let mut parts = Vec::new();
    if cl { parts.push(format!("{:>8}", l)); }
    if cw { parts.push(format!("{:>8}", w)); }
    if cc { parts.push(format!("{:>8}", c)); }
    println!("{} {}", parts.join(""), label);
}

pub fn builtin_env(args: &[String]) -> i32 {
    if args.len() == 1 {
        let mut vars: Vec<(String, String)> = std::env::vars().collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in vars { println!("{}={}", k, v); }
        return 0;
    }
    let mut extra_vars: Vec<(String, String)> = Vec::new();
    let mut cmd_start = 1;
    for (i, arg) in args[1..].iter().enumerate() {
        if let Some((k, v)) = arg.split_once('=') {
            extra_vars.push((k.to_string(), v.to_string()));
            cmd_start = i + 2;
        } else { break; }
    }
    if cmd_start >= args.len() {
        let mut vars: Vec<(String, String)> = std::env::vars().collect();
        for (k, v) in &extra_vars { vars.retain(|(ek, _)| ek != k); vars.push((k.clone(), v.clone())); }
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in vars { println!("{}={}", k, v); }
        return 0;
    }
    let mut cmd = std::process::Command::new(&args[cmd_start]);
    cmd.args(&args[cmd_start + 1..]);
    for (k, v) in extra_vars { cmd.env(k, v); }
    match cmd.status() {
        Ok(status) => status.code().unwrap_or(0),
        Err(e) => { eprintln!("env: {}: {}", args[cmd_start], e); 1 }
    }
}

pub fn builtin_sort(args: &[String]) -> i32 {
    let mut reverse = false;
    let mut unique = false;
    let mut numeric = false;
    let mut files = Vec::new();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch { 'r' => reverse = true, 'u' => unique = true, 'n' => numeric = true, _ => {} }
            }
        } else { files.push(arg.clone()); }
    }

    if files.is_empty() { eprintln!("usage: sort [-rnu] <file> [file2 ...]"); return 1; }

    let mut all = String::new();
    for file in &files {
        match std::fs::read_to_string(file) {
            Ok(c) => all.push_str(&c),
            Err(e) => { eprintln!("sort: {}: {}", file, e); return 1; }
        }
    }

    let mut lines: Vec<&str> = all.lines().collect();
    if numeric {
        lines.sort_by(|a, b| {
            let an: f64 = a.trim().parse().unwrap_or(0.0);
            let bn: f64 = b.trim().parse().unwrap_or(0.0);
            an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal)
        });
    } else { lines.sort(); }
    if reverse { lines.reverse(); }
    if unique { lines.dedup(); }
    for line in lines { println!("{}", line); }
    0
}

pub fn builtin_uniq(args: &[String]) -> i32 {
    let mut count = false;
    let mut unique_only = false;
    let mut repeated_only = false;
    let mut files = Vec::new();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch { 'c' => count = true, 'u' => unique_only = true, 'd' => repeated_only = true, _ => {} }
            }
        } else { files.push(arg.clone()); }
    }

    if files.is_empty() { eprintln!("usage: uniq [-cud] <file>"); return 1; }

    let content = match std::fs::read_to_string(&files[0]) {
        Ok(c) => c, Err(e) => { eprintln!("uniq: {}: {}", files[0], e); return 1; }
    };

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() { return 0; }

    let mut groups: Vec<(&str, usize)> = Vec::new();
    for line in &lines {
        if let Some(last) = groups.last_mut() {
            if last.0 == *line { last.1 += 1; continue; }
        }
        groups.push((line, 1));
    }

    for (line, n) in groups {
        if unique_only && n > 1 { continue; }
        if repeated_only && n == 1 { continue; }
        if count { println!("{:>7} {}", n, line); } else { println!("{}", line); }
    }
    0
}

pub fn builtin_xargs(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: xargs <command> [args...]");
        return 1;
    }

    // Read input from pipe temp file
    let tmp = std::env::temp_dir().join("rshell_pipe_in.tmp");
    let input = match std::fs::read_to_string(&tmp) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("xargs: no input (must be used in a pipeline)");
            return 1;
        }
    };

    let file_args: Vec<String> = input
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if file_args.is_empty() { return 0; }

    let cmd_name = &args[1];
    let initial_args = &args[2..];

    let mut full_args: Vec<String> = vec![cmd_name.clone()];
    full_args.extend(initial_args.iter().cloned());
    full_args.extend(file_args);

    crossterm::terminal::disable_raw_mode().ok();
    let mut cmd = std::process::Command::new(cmd_name);
    cmd.args(&full_args[1..]);

    let code = match cmd.status() {
        Ok(status) => status.code().unwrap_or(0),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("xargs: {}: command not found", cmd_name);
            } else {
                eprintln!("xargs: {}: {}", cmd_name, e);
            }
            1
        }
    };
    crossterm::terminal::enable_raw_mode().ok();
    code
}