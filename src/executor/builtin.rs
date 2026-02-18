// src/executor/builtin.rs
use crate::shell::Shell;
use std::path::PathBuf;

pub fn run_builtin(shell: &mut Shell, args: &[String]) -> Option<i32> {
    match args[0].as_str() {
        "cd"             => Some(builtin_cd(shell, args)),
        "exit" | "quit" => std::process::exit(shell.last_exit_code),
        "export"         => Some(builtin_export(shell, args)),
        "unset"          => Some(builtin_unset(shell, args)),
        "alias"          => Some(builtin_alias(shell, args)),
        "unalias"        => Some(builtin_unalias(shell, args)),
        "history"        => Some(builtin_history(shell)),
        "echo"           => Some(builtin_echo(args)),
        "pwd"            => { println!("{}", shell.cwd.display()); Some(0) }
        "source" | "."   => Some(builtin_source(shell, args)),
        "help"           => Some(builtin_help()),
        "jobs"           => Some(builtin_jobs(shell)),
        _                => None,
    }
}

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
        Some(path) => shell.cwd.join(path),
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

fn builtin_export(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() == 1 {
        for (k, v) in &shell.env {
            println!("export {}={}", k, v);
        }
        return 0;
    }
    for arg in &args[1..] {
        if let Some((k, v)) = arg.split_once('=') {
            let v = v.trim_matches('"').trim_matches('\'').to_string();
            shell.env.insert(k.to_string(), v.clone());
            // SAFETY: single-threaded shell, safe to set env vars
            unsafe { std::env::set_var(k, &v); }
        }
    }
    0
}

fn builtin_unset(shell: &mut Shell, args: &[String]) -> i32 {
    for arg in &args[1..] {
        shell.env.remove(arg);
        // SAFETY: single-threaded shell
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
        .replace("\\t", "\t");
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

fn builtin_help() -> i32 {
    println!(r#"
╔══════════════════════════════════════════╗
║        myshell  -  Built-in Commands     ║
╚══════════════════════════════════════════╝

  cd [dir]          Change directory (- for previous)
  pwd               Print working directory
  echo [-n] [args]  Print text
  export [VAR=VAL]  Set/show environment variables
  unset VAR         Remove environment variable
  alias [k=v]       Set/show aliases
  unalias NAME      Remove alias
  history           Show command history
  source FILE       Execute commands from file
  jobs              List background jobs
  help              Show this help
  exit              Exit myshell

  Operators:
    |   pipe         &&  and        ||  or
    ;   sequence     &   background
    >   stdout       >>  append     <   stdin
    2>  stderr       2>&1  merge stderr
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