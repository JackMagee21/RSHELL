// src/executor/builtin/util.rs

pub fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' { in_escape = true; }
        else if in_escape && ch.is_ascii_alphabetic() { in_escape = false; }
        else if !in_escape { len += 1; }
    }
    len
}

pub fn format_size(size: u64) -> String {
    if size >= 1_073_741_824      { format!("{:.1}G", size as f64 / 1_073_741_824.0) }
    else if size >= 1_048_576     { format!("{:.1}M", size as f64 / 1_048_576.0) }
    else if size >= 1024          { format!("{:.1}K", size as f64 / 1024.0) }
    else                          { format!("{}B", size) }
}

pub fn color_name(name: &str, is_dir: bool, path: &std::path::Path) -> String {
    if is_dir { format!("\x1b[34m{}/\x1b[0m", name) }
    else if is_executable(path) { format!("\x1b[32m{}\x1b[0m", name) }
    else { name.to_string() }
}

pub fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(windows)] {
        path.extension().map(|e| matches!(e.to_str(), Some("exe") | Some("bat") | Some("cmd"))).unwrap_or(false)
    }
}

pub fn command_not_found(cmd: &str) {
    eprintln!("\x1b[31mmyshell: command not found: {}\x1b[0m", cmd);
    if let Some(s) = find_closest_command(cmd) {
        eprintln!("\x1b[33m  did you mean: {}\x1b[0m", s);
    }
}

fn find_closest_command(cmd: &str) -> Option<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let mut best: Option<(String, usize)> = None;
    let builtins = vec![
        "cd","pwd","echo","export","unset","alias","unalias","history",
        "source","help","jobs","fg","bg","kill","clear","exit","ls",
        "true","false","test","functions",
    ];
    let mut candidates: Vec<String> = builtins.iter().map(|s| s.to_string()).collect();
    for dir in path_var.split(':') {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                candidates.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    for candidate in &candidates {
        let dist = levenshtein(cmd, candidate);
        if dist <= 3 {
            match &best {
                None => best = Some((candidate.clone(), dist)),
                Some((_, d)) if dist < *d => best = Some((candidate.clone(), dist)),
                _ => {}
            }
        }
    }
    best.map(|(s, _)| s)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i-1] == b[j-1] { dp[i-1][j-1] }
                       else { 1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1]) };
        }
    }
    dp[m][n]
}