// src/executor/builtin.rs - Cross-platform builtins
use crate::shell::Shell;
use std::path::PathBuf;

pub fn run_builtin(shell: &mut Shell, args: &[String]) -> Option<i32> {
    crossterm::terminal::disable_raw_mode().ok();

    let result = match args[0].as_str() {
        "cd"             => Some(builtin_cd(shell, args)),
        "exit" | "quit" => std::process::exit(shell.last_exit_code),
        "export" | "set" => Some(builtin_export(shell, args)),
        "unset"          => Some(builtin_unset(shell, args)),
        "alias"          => Some(builtin_alias(shell, args)),
        "unalias"        => Some(builtin_unalias(shell, args)),
        "history"        => Some(builtin_history(shell)),
        "echo"           => Some(builtin_echo(args)),
        "pwd"            => Some(builtin_pwd(shell)),
        "source" | "."   => Some(builtin_source(shell, args)),
        "help"           => Some(builtin_help()),
        "jobs"           => Some(builtin_jobs(shell)),
        "clear" | "cls"  => Some(builtin_clear()),
        "ls"             => Some(builtin_ls(shell, args)),
        "true"           => Some(0),
        "false"          => Some(1),
        "test" | "["     => Some(builtin_test(args)),
        _                => None,
    };

    result
}

// ── command not found ─────────────────────────────────────────────────────────

pub fn command_not_found(cmd: &str) {
    eprintln!("\x1b[31mmyshell: command not found: {}\x1b[0m", cmd);

    // Search PATH for close matches
    let suggestion = find_closest_command(cmd);
    if let Some(s) = suggestion {
        eprintln!("\x1b[33m  did you mean: {}\x1b[0m", s);
    }
}

fn find_closest_command(cmd: &str) -> Option<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let mut best: Option<(String, usize)> = None;

    // Also check builtins
    let builtins = vec![
        "cd", "pwd", "echo", "export", "unset", "alias", "unalias",
        "history", "source", "help", "jobs", "clear", "exit", "ls",
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
        // Only suggest if reasonably close (distance <= 3 and not too long)
        if dist <= 3 {
            match &best {
                None => best = Some((candidate.clone(), dist)),
                Some((_, best_dist)) if dist < *best_dist => {
                    best = Some((candidate.clone(), dist));
                }
                _ => {}
            }
        }
    }

    best.map(|(s, _)| s)
}

/// Levenshtein distance between two strings
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }

    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i-1] == b[j-1] {
                dp[i-1][j-1]
            } else {
                1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1])
            };
        }
    }

    dp[m][n]
}

// ── ls ────────────────────────────────────────────────────────────────────────

fn builtin_ls(shell: &Shell, args: &[String]) -> i32 {
    // Parse flags and target directory
    let mut show_hidden = false;
    let mut long_format = false;
    let mut target = shell.cwd.clone();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch {
                    'a' => show_hidden = true,
                    'l' => long_format = true,
                    'A' => show_hidden = true,
                    _ => {}
                }
            }
        } else {
            target = shell.cwd.join(arg);
        }
    }

    let entries = match std::fs::read_dir(&target) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("ls: {}: {}", target.display(), e);
            return 1;
        }
    };

    let mut items: Vec<std::fs::DirEntry> = entries
        .flatten()
        .filter(|e| {
            if show_hidden { true }
            else {
                !e.file_name().to_string_lossy().starts_with('.')
            }
        })
        .collect();

    // Sort: directories first, then files, both alphabetically
    items.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    if long_format {
        // Long format: permissions size name
        for item in &items {
            let meta = match item.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let is_dir = meta.is_dir();
            let size = meta.len();
            let name = item.file_name().to_string_lossy().to_string();

            let type_char = if is_dir { "d" } else { "-" };
            let name_colored = if is_dir {
                format!("\x1b[34m{}/\x1b[0m", name)
            } else if is_executable(&item.path()) {
                format!("\x1b[32m{}\x1b[0m", name)
            } else {
                name
            };

            println!("{} {:>10}  {}", type_char, format_size(size), name_colored);
        }
    } else {
        // Short format: colored names in columns
        let names: Vec<String> = items.iter().map(|item| {
            let name = item.file_name().to_string_lossy().to_string();
            let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                format!("\x1b[34m{}/\x1b[0m", name)
            } else if is_executable(&item.path()) {
                format!("\x1b[32m{}\x1b[0m", name)
            } else {
                name
            }
        }).collect();

        // Print in columns
        let col_width = 20usize;
        let term_width = 80usize;
        let cols = term_width / col_width;
        for (i, name) in names.iter().enumerate() {
            print!("{:<width$}", name, width = col_width);
            if (i + 1) % cols == 0 {
                println!();
            }
        }
        if names.len() % cols != 0 {
            println!();
        }
    }

    0
}

fn builtin_test(args: &[String]) -> i32 {
    // Strip surrounding [ ] if used as [ condition ]
    let args: Vec<&str> = args.iter()
        .map(|s| s.as_str())
        .filter(|&s| s != "[" && s != "]")
        .collect();

    // No arguments = false
    if args.len() <= 1 { return 1; }

    match args.as_slice() {
        // String tests
        [_, "-n", s]       => if s.is_empty() { 1 } else { 0 },
        [_, "-z", s]       => if s.is_empty() { 0 } else { 1 },
        [_, a, "=",  b]    => if a == b { 0 } else { 1 },
        [_, a, "!=", b]    => if a != b { 0 } else { 1 },

        // Numeric tests
        [_, a, "-eq", b]   => compare_nums(a, b, |x,y| x == y),
        [_, a, "-ne", b]   => compare_nums(a, b, |x,y| x != y),
        [_, a, "-lt", b]   => compare_nums(a, b, |x,y| x <  y),
        [_, a, "-le", b]   => compare_nums(a, b, |x,y| x <= y),
        [_, a, "-gt", b]   => compare_nums(a, b, |x,y| x >  y),
        [_, a, "-ge", b]   => compare_nums(a, b, |x,y| x >= y),

        // File tests
        [_, "-f", path]    => if std::path::Path::new(path).is_file() { 0 } else { 1 },
        [_, "-d", path]    => if std::path::Path::new(path).is_dir()  { 0 } else { 1 },
        [_, "-e", path]    => if std::path::Path::new(path).exists()  { 0 } else { 1 },

        // Single string (true if non-empty)
        [_, s]             => if s.is_empty() { 1 } else { 0 },

        _ => { eprintln!("test: unsupported expression"); 1 }
    }
}

fn compare_nums(a: &str, b: &str, f: impl Fn(i64, i64) -> bool) -> i32 {
    match (a.parse::<i64>(), b.parse::<i64>()) {
        (Ok(x), Ok(y)) => if f(x, y) { 0 } else { 1 },
        _ => { eprintln!("test: not a number"); 1 }
    }
}

fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        path.extension()
            .map(|e| matches!(e.to_str(), Some("exe") | Some("bat") | Some("cmd")))
            .unwrap_or(false)
    }
}

fn format_size(size: u64) -> String {
    if size >= 1_073_741_824 {
        format!("{:.1}G", size as f64 / 1_073_741_824.0)
    } else if size >= 1_048_576 {
        format!("{:.1}M", size as f64 / 1_048_576.0)
    } else if size >= 1024 {
        format!("{:.1}K", size as f64 / 1024.0)
    } else {
        format!("{}B", size)
    }
}

// ── existing builtins ─────────────────────────────────────────────────────────

fn builtin_cd(shell: &mut Shell, args: &[String]) -> i32 {
    let target: PathBuf = match args.get(1).map(|s| s.as_str()) {
        None | Some("~") => {
            match dirs::home_dir() {
                Some(h) => h,
                None => { eprintln!("cd: cannot find home directory"); return 1; }
            }
        }
        Some("-") => {
            match &shell.prev_dir {
                Some(prev) => prev.clone(),
                None => { eprintln!("cd: no previous directory"); return 1; }
            }
        }
        Some(path) => {
            let path = if path.starts_with("~/") || path.starts_with("~\\") {
                let home = dirs::home_dir().unwrap_or_default();
                home.join(&path[2..])
            } else {
                shell.cwd.join(path)
            };
            path
        }
    };

    let target = match target.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("cd: {}: {}", args.get(1).unwrap_or(&String::new()), e);
            return 1;
        }
    };

    match std::env::set_current_dir(&target) {
        Ok(_) => {
            shell.prev_dir = Some(shell.cwd.clone());
            shell.cwd = target;
            0
        }
        Err(e) => { eprintln!("cd: {e}"); 1 }
    }
}

fn builtin_pwd(shell: &Shell) -> i32 {
    println!("{}", shell.cwd.display());
    0
}

fn builtin_export(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() == 1 {
        for (k, v) in &shell.env {
            println!("{}={}", k, v);
        }
        return 0;
    }
    for arg in &args[1..] {
        if let Some((k, v)) = arg.split_once('=') {
            let v = v.trim_matches('"').trim_matches('\'').to_string();
            shell.env.insert(k.to_string(), v.clone());
            unsafe { std::env::set_var(k, &v); }
        }
    }
    0
}

fn builtin_unset(shell: &mut Shell, args: &[String]) -> i32 {
    for arg in &args[1..] {
        shell.env.remove(arg);
        unsafe { std::env::remove_var(arg); }
    }
    0
}

fn builtin_alias(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() == 1 {
        for (k, v) in &shell.aliases {
            println!("alias {}='{}'", k, v);
        }
        return 0;
    }
    for arg in &args[1..] {
        if let Some((k, v)) = arg.split_once('=') {
            shell.aliases.insert(
                k.to_string(),
                v.trim_matches('"').trim_matches('\'').to_string(),
            );
        } else if let Some(v) = shell.aliases.get(arg.as_str()) {
            println!("alias {}='{}'", arg, v);
        } else {
            eprintln!("alias: {}: not found", arg);
        }
    }
    0
}

fn builtin_unalias(shell: &mut Shell, args: &[String]) -> i32 {
    for arg in &args[1..] {
        shell.aliases.remove(arg.as_str());
    }
    0
}

fn builtin_history(shell: &Shell) -> i32 {
    for (i, line) in shell.history.iter().enumerate() {
        println!("{:4}  {}", i + 1, line);
    }
    0
}

fn builtin_echo(args: &[String]) -> i32 {
    let mut no_newline = false;
    let mut start = 1;
    if args.get(1).map(|s| s.as_str()) == Some("-n") {
        no_newline = true;
        start = 2;
    }
    let output = args[start..].join(" ")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r");
    if no_newline { print!("{}", output); } else { println!("{}", output); }
    0
}

fn builtin_source(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("source: filename argument required");
        return 1;
    }
    let path = shell.cwd.join(&args[1]);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Err(e) = shell.eval(line) {
                    eprintln!("source: {e}");
                }
            }
            0
        }
        Err(e) => { eprintln!("source: {}: {e}", args[1]); 1 }
    }
}

fn builtin_clear() -> i32 {
    print!("\x1B[2J\x1B[H");
    use std::io::Write;
    std::io::stdout().flush().ok();
    0
}

fn builtin_help() -> i32 {
    println!(r#"
╔══════════════════════════════════════════════╗
║       myshell  —  Built-in Commands          ║
╚══════════════════════════════════════════════╝

  cd [dir]          Change directory  (- for previous, ~ for home)
  pwd               Print working directory
  ls [-la] [dir]    List directory contents
  echo [-n] [args]  Print text  (\n \t supported)
  export [VAR=VAL]  Set or show environment variables
  unset VAR         Remove environment variable
  alias [k=v]       Set or show aliases
  unalias NAME      Remove alias
  history           Show command history
  source FILE       Execute commands from a file
  clear / cls       Clear the screen
  jobs              List background jobs
  help              Show this help
  exit              Exit myshell

  Operators:
    |    pipe          &&   and         ||   or
    ;    sequence      &    background
    >    stdout        >>   append      <    stdin
    2>   stderr        2>&1 merge stderr into stdout

  Multiline:
    End a line with | && || or \\ to continue on the next line
"#);
    0
}

fn builtin_jobs(shell: &Shell) -> i32 {
    if shell.jobs.is_empty() {
        println!("No background jobs");
    }
    for (id, job) in &shell.jobs {
        println!("[{}] {} - {}", id, job.pid, job.command);
    }
    0
}