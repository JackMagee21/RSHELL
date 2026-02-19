// src/executor/builtin.rs
use crate::shell::{Shell, JobStatus};
use std::path::PathBuf;

pub fn run_builtin(shell: &mut Shell, args: &[String]) -> Option<i32> {
    crossterm::terminal::disable_raw_mode().ok();

    let result = match args[0].as_str() {
        "cd"              => Some(builtin_cd(shell, args)),
        "exit" | "quit"   => std::process::exit(shell.last_exit_code),
        "export" | "set"  => Some(builtin_export(shell, args)),
        "unset"           => Some(builtin_unset(shell, args)),
        "alias"           => Some(builtin_alias(shell, args)),
        "unalias"         => Some(builtin_unalias(shell, args)),
        "history"         => Some(builtin_history(shell)),
        "echo"            => Some(builtin_echo(args)),
        "pwd"             => Some(builtin_pwd(shell)),
        "source" | "."    => Some(builtin_source(shell, args)),
        "help"            => Some(builtin_help()),
        "jobs"            => Some(builtin_jobs(shell)),
        "fg"              => Some(builtin_fg(shell, args)),
        "bg"              => Some(builtin_bg(shell, args)),
        "kill"            => Some(builtin_kill(shell, args)),
        "clear" | "cls"   => Some(builtin_clear()),
        "ls"              => Some(builtin_ls(shell, args)),
        "true"            => Some(0),
        "false"           => Some(1),
        "test" | "["      => Some(builtin_test(shell, args)),
        "functions"       => Some(builtin_functions(shell)),
        "sleep"           => Some(builtin_sleep(args)),
        "mkdir"           => Some(builtin_mkdir(args)),
        "touch"           => Some(builtin_touch(args)),
        "rm"              => Some(builtin_rm(args)),
        "cp"              => Some(builtin_cp(args)),
        "mv"              => Some(builtin_mv(args)),
        "cat"             => Some(builtin_cat(args)),
        _                 => None,
    };

    result
}

// ── Job control ───────────────────────────────────────────────────────────────

fn builtin_jobs(shell: &mut Shell) -> i32 {
    // Reap finished jobs first
    shell.reap_jobs();

    if shell.jobs.is_empty() {
        println!("No jobs");
        return 0;
    }

    let mut job_list: Vec<_> = shell.jobs.values().collect();
    job_list.sort_by_key(|j| j.id);

    for job in job_list {
        let marker = if job.status == JobStatus::Running { "+" } else { "-" };
        println!("[{}] {} {:10} {}", job.id, marker, job.status.to_string(), job.command);
    }
    0
}

fn builtin_fg(shell: &mut Shell, args: &[String]) -> i32 {
    // Get job id - default to most recent
    let job_id = get_job_id(shell, args);

    let (pid, command) = match job_id.and_then(|id| shell.jobs.get(&id)) {
        Some(job) => (job.pid, job.command.clone()),
        None => {
            eprintln!("fg: no such job");
            return 1;
        }
    };

    println!("{}", command);

    #[cfg(unix)]
    {
        // Send SIGCONT to resume if stopped
        unsafe { libc::kill(pid as i32, libc::SIGCONT); }

        // Wait for it to finish
        let mut status = 0i32;
        unsafe { libc::waitpid(pid as i32, &mut status, 0); }

        // Remove from jobs
        if let Some(id) = job_id {
            shell.jobs.remove(&id);
        }

        // Return exit code
        if libc::WIFEXITED(status) {
            libc::WEXITSTATUS(status)
        } else {
            1
        }
    }

    #[cfg(windows)]
    {
        eprintln!("fg: job control not fully supported on Windows");
        1
    }
}

fn builtin_bg(shell: &mut Shell, args: &[String]) -> i32 {
    let job_id = get_job_id(shell, args);

    let (pid, command) = match job_id.and_then(|id| shell.jobs.get_mut(&id)) {
        Some(job) => {
            job.status = JobStatus::Running;
            (job.pid, job.command.clone())
        }
        None => {
            eprintln!("bg: no such job");
            return 1;
        }
    };

    #[cfg(unix)]
    unsafe { libc::kill(pid as i32, libc::SIGCONT); }

    println!("[{}] {}", job_id.unwrap_or(0), command);
    0
}

fn builtin_kill(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: kill [%jobid | pid]");
        return 1;
    }

    let target = &args[1];

    // %1 means job id, otherwise treat as pid
    if target.starts_with('%') {
        let id: usize = match target[1..].parse() {
            Ok(n) => n,
            Err(_) => { eprintln!("kill: invalid job id"); return 1; }
        };
        if let Some(job) = shell.jobs.get(&id) {
            #[cfg(unix)]
            unsafe { libc::kill(job.pid as i32, libc::SIGTERM); }
            #[cfg(windows)]
            eprintln!("kill: not fully supported on Windows");
            shell.jobs.remove(&id);
        } else {
            eprintln!("kill: no such job: {}", id);
            return 1;
        }
    } else {
        // Direct PID
        let pid: i32 = match target.parse() {
            Ok(n) => n,
            Err(_) => { eprintln!("kill: invalid pid"); return 1; }
        };
        #[cfg(unix)]
        unsafe { libc::kill(pid, libc::SIGTERM); }
        #[cfg(windows)]
        {
            std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .output()
                .ok();
        }
    }
    0
}

/// Get job id from args, defaulting to most recent job
fn get_job_id(shell: &Shell, args: &[String]) -> Option<usize> {
    if let Some(arg) = args.get(1) {
        // %1 or just 1
        let s = arg.trim_start_matches('%');
        s.parse().ok()
    } else {
        // Most recent job
        shell.jobs.keys().max().copied()
    }
}

// ── command not found ─────────────────────────────────────────────────────────

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

// ── test / [ ] ────────────────────────────────────────────────────────────────

fn builtin_test(shell: &Shell, args: &[String]) -> i32 {
    use crate::executor::{expand_vars, expand_arithmetic};
    let expanded: Vec<String> = args.iter()
        .map(|a| { let a = expand_arithmetic(shell, a); expand_vars(shell, &a) })
        .collect();
    let args: Vec<&str> = expanded.iter()
        .skip(1)
        .map(|s| s.as_str())
        .filter(|&s| s != "]")
        .collect();
    if args.is_empty() { return 1; }
    if args[0] == "!" {
        return if eval_test(&args[1..]) == 0 { 1 } else { 0 };
    }
    eval_test(&args)
}

fn eval_test(args: &[&str]) -> i32 {
    match args {
        ["-n", s]        => if s.is_empty() { 1 } else { 0 },
        ["-z", s]        => if s.is_empty() { 0 } else { 1 },
        [a, "=",  b]     => if a == b { 0 } else { 1 },
        [a, "==", b]     => if a == b { 0 } else { 1 },
        [a, "!=", b]     => if a != b { 0 } else { 1 },
        [a, "-eq", b]    => compare_nums(a, b, |x,y| x == y),
        [a, "-ne", b]    => compare_nums(a, b, |x,y| x != y),
        [a, "-lt", b]    => compare_nums(a, b, |x,y| x <  y),
        [a, "-le", b]    => compare_nums(a, b, |x,y| x <= y),
        [a, "-gt", b]    => compare_nums(a, b, |x,y| x >  y),
        [a, "-ge", b]    => compare_nums(a, b, |x,y| x >= y),
        ["-f", p]        => if std::path::Path::new(p).is_file()  { 0 } else { 1 },
        ["-d", p]        => if std::path::Path::new(p).is_dir()   { 0 } else { 1 },
        ["-e", p]        => if std::path::Path::new(p).exists()   { 0 } else { 1 },
        ["-s", p]        => if std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false) { 0 } else { 1 },
        [s]              => if s.is_empty() { 1 } else { 0 },
        _                => { eprintln!("test: unsupported expression: {:?}", args); 1 }
    }
}

fn compare_nums(a: &str, b: &str, f: impl Fn(i64, i64) -> bool) -> i32 {
    match (a.parse::<i64>(), b.parse::<i64>()) {
        (Ok(x), Ok(y)) => if f(x, y) { 0 } else { 1 },
        _ => { eprintln!("test: '{}' or '{}' is not a number", a, b); 1 }
    }
}

// ── functions list ────────────────────────────────────────────────────────────

fn builtin_functions(shell: &Shell) -> i32 {
    if shell.functions.is_empty() {
        println!("No functions defined.");
        return 0;
    }
    for (name, func) in &shell.functions {
        println!("function {}() {{", name);
        for line in &func.body { println!("  {}", line); }
        println!("}}");
    }
    0
}

// ── ls ────────────────────────────────────────────────────────────────────────

fn builtin_ls(shell: &Shell, args: &[String]) -> i32 {
    let mut show_hidden = false;
    let mut long_format = false;
    let mut target = shell.cwd.clone();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch { 'a'|'A' => show_hidden = true, 'l' => long_format = true, _ => {} }
            }
        } else {
            target = shell.cwd.join(arg);
        }
    }

    let entries = match std::fs::read_dir(&target) {
        Ok(e) => e,
        Err(e) => { eprintln!("ls: {}: {}", target.display(), e); return 1; }
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
    return 0;  // ← add this
} else {
    let names: Vec<String> = items.iter().map(|item| {
        let name = item.file_name().to_string_lossy().to_string();
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        color_name(&name, is_dir, &item.path())
    }).collect();

    let max_len = names.iter()
        .map(|n| strip_ansi_len(n))
        .max()
        .unwrap_or(0);
    let col_width = (max_len + 2).max(16);
    let term_width = 80usize;
    let cols = (term_width / col_width).max(1);

    for (i, name) in names.iter().enumerate() {
        let visible_len = strip_ansi_len(name);
        let padding = col_width.saturating_sub(visible_len);
        print!("{}{}", name, " ".repeat(padding));
        if (i + 1) % cols == 0 { println!(); }
    }
    if !names.is_empty() && names.len() % cols != 0 { println!(); }
}
0  // ← make sure this is here at the end
}

fn color_name(name: &str, is_dir: bool, path: &std::path::Path) -> String {
    if is_dir { format!("\x1b[34m{}/\x1b[0m", name) }
    else if is_executable(path) { format!("\x1b[32m{}\x1b[0m", name) }
    else { name.to_string() }
}

fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(windows)] {
        path.extension().map(|e| matches!(e.to_str(), Some("exe")|Some("bat")|Some("cmd"))).unwrap_or(false)
    }
}

fn format_size(size: u64) -> String {
    if size >= 1_073_741_824 { format!("{:.1}G", size as f64 / 1_073_741_824.0) }
    else if size >= 1_048_576 { format!("{:.1}M", size as f64 / 1_048_576.0) }
    else if size >= 1024      { format!("{:.1}K", size as f64 / 1024.0) }
    else                      { format!("{}B", size) }
}

// ── mkdir ─────────────────────────────────────────────────────────────────────

fn builtin_mkdir(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: mkdir [-p] <dir>");
        return 1;
    }

    let mut parents = false;
    let mut dirs = Vec::new();

    for arg in &args[1..] {
        if arg == "-p" {
            parents = true;
        } else {
            dirs.push(arg);
        }
    }

    let mut code = 0;
    for dir in dirs {
        let result = if parents {
            std::fs::create_dir_all(dir)
        } else {
            std::fs::create_dir(dir)
        };

        match result {
            Ok(_) => println!("created {}", dir),
            Err(e) => {
                eprintln!("mkdir: {}: {}", dir, e);
                code = 1;
            }
        }
    }
    code
}

// ── rm, cp, mv, cat ────────────────────────────────────────────────────────────────────────

fn builtin_rm(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: rm [-rf] <file> [file2 ...]");
        return 1;
    }

    let mut recursive = false;
    let mut force = false;
    let mut targets = Vec::new();

    for arg in &args[1..] {
        if arg.starts_with('-') {
            for ch in arg.chars().skip(1) {
                match ch {
                    'r' | 'R' => recursive = true,
                    'f'       => force = true,
                    _         => {}
                }
            }
        } else {
            targets.push(arg);
        }
    }

    let mut code = 0;
    for target in targets {
        let path = std::path::Path::new(target);

        if !path.exists() {
            if !force {
                eprintln!("rm: {}: no such file or directory", target);
                code = 1;
            }
            continue;
        }

        let result = if path.is_dir() {
            if recursive {
                std::fs::remove_dir_all(path)
            } else {
                eprintln!("rm: {}: is a directory (use -r to remove)", target);
                code = 1;
                continue;
            }
        } else {
            std::fs::remove_file(path)
        };

        if let Err(e) = result {
            eprintln!("rm: {}: {}", target, e);
            code = 1;
        }
    }
    code
}

fn builtin_cp(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("usage: cp [-r] <source> <dest>");
        return 1;
    }

    let mut recursive = false;
    let mut files = Vec::new();

    for arg in &args[1..] {
        if arg == "-r" || arg == "-R" || arg == "-rf" || arg == "-fr" {
            recursive = true;
        } else {
            files.push(arg.as_str());
        }
    }

    if files.len() < 2 {
        eprintln!("cp: missing destination");
        return 1;
    }

    let dest = std::path::Path::new(files[files.len() - 1]);
    let sources = &files[..files.len() - 1];

    let mut code = 0;
    for src in sources {
        let src_path = std::path::Path::new(src);

        if !src_path.exists() {
            eprintln!("cp: {}: no such file or directory", src);
            code = 1;
            continue;
        }

        // Work out actual destination path
        let actual_dest = if dest.is_dir() {
            let filename = src_path.file_name().unwrap_or_default();
            dest.join(filename)
        } else {
            dest.to_path_buf()
        };

        let result = if src_path.is_dir() {
            if recursive {
                copy_dir_all(src_path, &actual_dest)
            } else {
                eprintln!("cp: {}: is a directory (use -r to copy)", src);
                code = 1;
                continue;
            }
        } else {
            std::fs::copy(src_path, &actual_dest).map(|_| ())
        };

        if let Err(e) = result {
            eprintln!("cp: {}: {}", src, e);
            code = 1;
        }
    }
    code
}

/// Recursively copy a directory
fn copy_dir_all(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

fn builtin_mv(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("usage: mv <source> <dest>");
        return 1;
    }

    let dest = std::path::Path::new(&args[args.len() - 1]);
    let sources = &args[1..args.len() - 1];

    let mut code = 0;
    for src in sources {
        let src_path = std::path::Path::new(src);

        if !src_path.exists() {
            eprintln!("mv: {}: no such file or directory", src);
            code = 1;
            continue;
        }

        let actual_dest = if dest.is_dir() {
            let filename = src_path.file_name().unwrap_or_default();
            dest.join(filename)
        } else {
            dest.to_path_buf()
        };

        if let Err(e) = std::fs::rename(src_path, &actual_dest) {
            eprintln!("mv: {}: {}", src, e);
            code = 1;
        }
    }
    code
}

fn builtin_cat(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: cat <file> [file2 ...]");
        return 1;
    }

    let mut code = 0;
    for filename in &args[1..] {
        match std::fs::read_to_string(filename) {
            Ok(contents) => print!("{}", contents),
            Err(e) => {
                eprintln!("cat: {}: {}", filename, e);
                code = 1;
            }
        }
    }
    code
}

// ── touch ─────────────────────────────────────────────────────────────────────

fn builtin_touch(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: touch <file> [file2 ...]");
        return 1;
    }

    let mut code = 0;
    for filename in &args[1..] {
        let path = std::path::Path::new(filename);

        if path.exists() {
            // File exists - just update the modified time
            match filetime::set_file_mtime(path, filetime::FileTime::now()) {
                Ok(_) => {}
                Err(e) => { eprintln!("touch: {}: {}", filename, e); code = 1; }
            }
        } else {
            // Create empty file
            match std::fs::File::create(path) {
                Ok(_) => {}
                Err(e) => { eprintln!("touch: {}: {}", filename, e); code = 1; }
            }
        }
    }
    code
}

// ── other builtins ────────────────────────────────────────────────────────────

fn builtin_sleep(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("usage: sleep <seconds>");
        return 1;
    }
    match args[1].parse::<f64>() {
        Ok(secs) => {
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            0
        }
        Err(_) => {
            eprintln!("sleep: invalid time: {}", args[1]);
            1
        }
    }
}

fn builtin_cd(shell: &mut Shell, args: &[String]) -> i32 {
    let target: PathBuf = match args.get(1).map(|s| s.as_str()) {
        None | Some("~") => match dirs::home_dir() {
            Some(h) => h,
            None => { eprintln!("cd: cannot find home directory"); return 1; }
        },
        Some("-") => match &shell.prev_dir {
            Some(p) => p.clone(),
            None => { eprintln!("cd: no previous directory"); return 1; }
        },
        Some(path) => {
            if path.starts_with("~/") || path.starts_with("~\\") {
                dirs::home_dir().unwrap_or_default().join(&path[2..])
            } else {
                shell.cwd.join(path)
            }
        }
    };

    let target = match target.canonicalize() {
        Ok(p) => p,
        Err(e) => { eprintln!("cd: {}: {}", args.get(1).unwrap_or(&String::new()), e); return 1; }
    };

    match std::env::set_current_dir(&target) {
        Ok(_) => { shell.prev_dir = Some(shell.cwd.clone()); shell.cwd = target; 0 }
        Err(e) => { eprintln!("cd: {e}"); 1 }
    }
}

fn builtin_pwd(shell: &Shell) -> i32 {
    println!("{}", shell.cwd.display()); 0
}

fn builtin_export(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() == 1 {
        for (k, v) in &shell.env { println!("{}={}", k, v); }
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
        for (k, v) in &shell.aliases { println!("alias {}='{}'", k, v); }
        return 0;
    }
    for arg in &args[1..] {
        if let Some((k, v)) = arg.split_once('=') {
            shell.aliases.insert(k.to_string(), v.trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = shell.aliases.get(arg.as_str()) {
            println!("alias {}='{}'", arg, v);
        } else {
            eprintln!("alias: {}: not found", arg);
        }
    }
    0
}

fn builtin_unalias(shell: &mut Shell, args: &[String]) -> i32 {
    for arg in &args[1..] { shell.aliases.remove(arg.as_str()); }
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
    if args.get(1).map(|s| s.as_str()) == Some("-n") { no_newline = true; start = 2; }
    let output = args[start..].join(" ").replace("\\n", "\n").replace("\\t", "\t");
    if no_newline { print!("{}", output); } else { println!("{}", output); }
    0
}

fn builtin_source(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("source: filename required"); return 1; }
    let path = shell.cwd.join(&args[1]);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Err(e) = shell.eval(line) { eprintln!("source: {e}"); }
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

/// Get visible length of string ignoring ANSI escape codes
fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' { in_escape = true; }
        else if in_escape && ch.is_ascii_alphabetic() { in_escape = false; }
        else if !in_escape { len += 1; }
    }
    len
}

fn builtin_help() -> i32 {
    println!(r#"
╔══════════════════════════════════════════════╗
║         myshell  —  Built-in Commands        ║
╚══════════════════════════════════════════════╝

  cd [dir]           Change directory (- for previous, ~ for home)
  pwd                Print working directory
  ls [-la] [dir]     List directory contents
  echo [-n] [args]   Print text
  export [VAR=VAL]   Set or show environment variables
  unset VAR          Remove environment variable
  alias [k=v]        Set or show aliases
  unalias NAME       Remove alias
  history            Show command history
  source FILE        Execute commands from a file
  clear / cls        Clear the screen
  help               Show this help
  exit               Exit myshell

  Job Control:
    jobs             List background jobs
    fg [%id]         Bring job to foreground
    bg [%id]         Resume stopped job in background
    kill [%id|pid]   Kill a job or process
    cmd &            Run command in background
    Ctrl+Z           Suspend current command (Linux)

  Scripting:
    if test -f file {{ cmd }} else {{ cmd }}
    for x in a b c; do cmd; done
    while test $X -lt 10; do cmd; done
    function foo() {{ cmd; }}
    echo $((2 + 2 * 3))

  Operators:
    |  pipe   &&  and   ||  or   ;  sequence   &  background
    >  stdout  >>  append  <  stdin  2>  stderr
"#);
    0
}