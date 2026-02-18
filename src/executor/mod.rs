// src/executor/mod.rs - Cross-platform executor (Windows + Linux)
pub mod builtin;

use crate::parser::ast::{Command, Redirect};
use crate::shell::Shell;
use anyhow::Result;
use std::fs::OpenOptions;
use std::process::{Command as Proc, Stdio};

pub fn execute(shell: &mut Shell, cmd: Command) -> Result<()> {
    let code = run(shell, cmd)?;
    shell.last_exit_code = code;
    Ok(())
}

fn run(shell: &mut Shell, cmd: Command) -> Result<i32> {
    match cmd {
        Command::Simple { args, redirects, background } => {
            run_simple(shell, args, redirects, background)
        }
        Command::Pipeline(cmds) => run_pipeline(shell, cmds),
        Command::And(left, right) => {
            let code = run(shell, *left)?;
            if code == 0 { run(shell, *right) } else { Ok(code) }
        }
        Command::Or(left, right) => {
            let code = run(shell, *left)?;
            if code != 0 { run(shell, *right) } else { Ok(code) }
        }
        Command::Sequence(left, right) => {
            run(shell, *left)?;
            run(shell, *right)
        }
    }
}

fn run_simple(
    shell: &mut Shell,
    mut args: Vec<String>,
    redirects: Vec<Redirect>,
    background: bool,
) -> Result<i32> {
    if args.is_empty() { return Ok(0); }

    // Expand $VARIABLES
    for arg in &mut args {
        *arg = expand_vars(shell, arg);
    }

    // Expand aliases
    if let Some(alias_val) = shell.aliases.get(&args[0]).cloned() {
        let alias_args: Vec<String> = alias_val
            .split_whitespace()
            .map(String::from)
            .collect();
        if alias_args[0] != args[0] {
            let mut new_args = alias_args;
            new_args.extend(args.into_iter().skip(1));
            args = new_args;
        }
    }

    // Builtins run in-process
    if let Some(code) = builtin::run_builtin(shell, &args) {
        return Ok(code);
    }

    run_external(shell, &args, &redirects, background)
}

fn run_external(
    shell: &Shell,
    args: &[String],
    redirects: &[Redirect],
    background: bool,
) -> Result<i32> {
    let mut cmd = build_command(args, redirects)?;

    // Inherit environment from shell state
    cmd.envs(&shell.env);

    if background {
        // Spawn and don't wait
        match cmd.spawn() {
            Ok(child) => {
                println!("[bg] pid {}", child.id());
                Ok(0)
            }
            Err(e) => {
                eprintln!("myshell: {}: {}", args[0], e);
                Ok(1)
            }
        }
    } else {
        match cmd.status() {
            Ok(status) => Ok(status.code().unwrap_or(0)),
            Err(e) => {
                eprintln!("myshell: {}: {}", args[0], friendly_error(e));
                Ok(127)
            }
        }
    }
}

/// Build a std::process::Command with redirects applied
fn build_command(args: &[String], redirects: &[Redirect]) -> Result<Proc> {
    let mut cmd = platform_command(&args[0]);
    cmd.args(&args[1..]);

    for redirect in redirects {
        match redirect {
            Redirect::StdoutTo(file) => {
                let f = OpenOptions::new()
                    .write(true).create(true).truncate(true)
                    .open(file)?;
                cmd.stdout(Stdio::from(f));
            }
            Redirect::StdoutAppend(file) => {
                let f = OpenOptions::new()
                    .write(true).create(true).append(true)
                    .open(file)?;
                cmd.stdout(Stdio::from(f));
            }
            Redirect::StdinFrom(file) => {
                let f = OpenOptions::new().read(true).open(file)?;
                cmd.stdin(Stdio::from(f));
            }
            Redirect::StderrTo(file) => {
                let f = OpenOptions::new()
                    .write(true).create(true).truncate(true)
                    .open(file)?;
                cmd.stderr(Stdio::from(f));
            }
            Redirect::StderrToStdout => {
                // Capture stderr and send to stdout - handled below
                // For simplicity we use inherit for both here
                // A full implementation would dup the stdout handle
                cmd.stderr(Stdio::inherit());
            }
        }
    }

    Ok(cmd)
}

/// Run a pipeline: cmd1 | cmd2 | cmd3
/// Uses std::process piping — fully cross-platform
fn run_pipeline(shell: &mut Shell, cmds: Vec<Command>) -> Result<i32> {
    if cmds.len() == 1 {
        return run(shell, cmds.into_iter().next().unwrap());
    }

    // Extract simple commands from the pipeline
    let simple_cmds: Vec<(Vec<String>, Vec<Redirect>)> = cmds
        .into_iter()
        .filter_map(|cmd| match cmd {
            Command::Simple { mut args, redirects, .. } => {
                // Expand variables in each arg
                for arg in &mut args {
                    *arg = expand_vars(shell, arg);
                }
                Some((args, redirects))
            }
            _ => None,
        })
        .collect();

    let n = simple_cmds.len();
    let mut prev_stdout: Option<Stdio> = None;
    let mut children = Vec::new();

    for (i, (args, redirects)) in simple_cmds.into_iter().enumerate() {
        if args.is_empty() { continue; }

        // Builtins in pipelines: for now just run them (no pipe support for builtins)
        // A full implementation would redirect builtin stdout to the pipe
        let mut cmd = build_command(&args, &redirects)?;
        cmd.envs(&shell.env);

        // Hook up stdin from previous command's stdout
        if let Some(prev) = prev_stdout.take() {
            cmd.stdin(prev);
        }

        // If not the last command, pipe stdout to next
        let is_last = i == n - 1;
        if !is_last {
            cmd.stdout(Stdio::piped());
        }

        match cmd.spawn() {
            Ok(mut child) => {
                // Take stdout pipe to feed into next command
                if !is_last {
                    if let Some(stdout) = child.stdout.take() {
                        prev_stdout = Some(Stdio::from(stdout));
                    }
                }
                children.push(child);
            }
            Err(e) => {
                eprintln!("myshell: {}: {}", args[0], friendly_error(e));
            }
        }
    }

    // Wait for all children, return last exit code
    let mut last_code = 0;
    for mut child in children {
        if let Ok(status) = child.wait() {
            last_code = status.code().unwrap_or(0);
        }
    }

    Ok(last_code)
}

/// On Windows, wrap commands through cmd.exe for built-in commands like `dir`
/// On Linux/Mac, run directly
fn platform_command(program: &str) -> Proc {
    #[cfg(target_os = "windows")]
    {
        // Check if it's a cmd.exe built-in
        let cmd_builtins = [
            "dir", "cls", "type", "copy", "del", "move", "ren", "md", "rd",
            "echo", "set", "path", "ver", "vol", "date", "time",
        ];
        if cmd_builtins.contains(&program.to_lowercase().as_str()) {
            let mut cmd = Proc::new("cmd");
            cmd.args(["/C", program]);
            return cmd;
        }
        Proc::new(program)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Proc::new(program)
    }
}

/// Give friendlier error messages
fn friendly_error(e: std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "command not found".to_string(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => e.to_string(),
    }
}

/// Expand $VARIABLE and ${VARIABLE} references
fn expand_vars(shell: &Shell, s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '$' {
            result.push(c);
            continue;
        }
        match chars.peek() {
            Some(&'{') => {
                chars.next();
                let mut var = String::new();
                for ch in chars.by_ref() {
                    if ch == '}' { break; }
                    var.push(ch);
                }
                result.push_str(&lookup_var(shell, &var));
            }
            Some(&'?') => {
                chars.next();
                result.push_str(&shell.last_exit_code.to_string());
            }
            Some(&ch) if ch.is_alphanumeric() || ch == '_' => {
                let mut var = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var.push(ch);
                        chars.next();
                    } else { break; }
                }
                result.push_str(&lookup_var(shell, &var));
            }
            _ => result.push('$'),
        }
    }
    result
}

fn lookup_var(shell: &Shell, name: &str) -> String {
    shell.env.get(name)
        .cloned()
        .or_else(|| std::env::var(name).ok())
        .unwrap_or_default()
}