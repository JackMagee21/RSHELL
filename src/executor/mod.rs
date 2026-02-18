// src/executor/mod.rs
pub mod builtin;

use crate::parser::ast::{Command, Redirect};
use crate::shell::Shell;
use anyhow::Result;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, ForkResult, execvp, pipe, dup2};
use std::ffi::CString;
use std::os::unix::io::RawFd;
use nix::unistd::close;

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
        Command::Pipeline(cmds) => {
            run_pipeline(shell, cmds)
        }
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

fn run_simple(shell: &mut Shell, mut args: Vec<String>, redirects: Vec<Redirect>, background: bool) -> Result<i32> {
    if args.is_empty() { return Ok(0); }

    // Expand environment variables in args
    for arg in &mut args {
        *arg = expand_vars(shell, arg);
    }

    // Check aliases (only for first arg)
    if let Some(alias_val) = shell.aliases.get(&args[0]).cloned() {
        let alias_args: Vec<String> = alias_val.split_whitespace().map(String::from).collect();
        // Prevent infinite alias expansion
        if alias_args[0] != args[0] {
            let mut new_args = alias_args;
            new_args.extend(args.into_iter().skip(1));
            args = new_args;
        }
    }

    // Try builtins first
    if let Some(code) = builtin::run_builtin(shell, &args) {
        return Ok(code);
    }

    // External command via fork/exec
    run_external(shell, &args, &redirects, background)
}

fn run_external(shell: &Shell, args: &[String], redirects: &[Redirect], background: bool) -> Result<i32> {
    let c_args: Vec<CString> = args.iter()
        .map(|a| CString::new(a.as_str()).unwrap())
        .collect();

    match unsafe { fork() }? {
        ForkResult::Child => {
            // Apply redirections
            for redirect in redirects {
                apply_redirect(redirect);
            }

            execvp(&c_args[0], &c_args)
                .map_err(|e| {
                    eprintln!("myshell: {}: {}", args[0], e);
                    std::process::exit(127);
                })
                .ok();
            std::process::exit(127);
        }
        ForkResult::Parent { child } => {
            if background {
                println!("[bg] pid {}", child);
                Ok(0)
            } else {
                match waitpid(child, None)? {
                    WaitStatus::Exited(_, code) => Ok(code),
                    WaitStatus::Signaled(_, sig, _) => {
                        eprintln!("Killed by signal: {:?}", sig);
                        Ok(128 + sig as i32)
                    }
                    _ => Ok(0),
                }
            }
        }
    }
}

fn run_pipeline(shell: &mut Shell, cmds: Vec<Command>) -> Result<i32> {
    if cmds.len() == 1 {
        return run(shell, cmds.into_iter().next().unwrap());
    }

    let n = cmds.len();
    let mut pipes: Vec<(RawFd, RawFd)> = Vec::new();

    // Create n-1 pipes
    for _ in 0..n - 1 {
        let (r, w) = pipe()?;
        pipes.push((r, w));
    }

    let mut child_pids = Vec::new();

    for (i, cmd) in cmds.into_iter().enumerate() {
        let args = match &cmd {
            Command::Simple { args, .. } => args.clone(),
            _ => continue, // simplified: only simple cmds in pipeline for now
        };

        let redirects = match &cmd {
            Command::Simple { redirects, .. } => redirects.clone(),
            _ => vec![],
        };

        let c_args: Vec<CString> = args.iter()
            .map(|a| CString::new(a.as_str()).unwrap())
            .collect();

        match unsafe { fork() }? {
            ForkResult::Child => {
                // Set up stdin from previous pipe
                if i > 0 {
                    dup2(pipes[i - 1].0, 0).ok();
                }
                // Set up stdout to next pipe
                if i < n - 1 {
                    dup2(pipes[i].1, 1).ok();
                }
                // Close all pipe fds
                for &(r, w) in &pipes {
                    close(r).ok();
                    close(w).ok();
                }
                // Apply explicit redirects
                for redirect in &redirects {
                    apply_redirect(redirect);
                }
                // Try builtin first in child context (limited)
                execvp(&c_args[0], &c_args)
                    .map_err(|e| {
                        eprintln!("myshell: {}: {}", args[0], e);
                        std::process::exit(127);
                    })
                    .ok();
                std::process::exit(127);
            }
            ForkResult::Parent { child } => {
                child_pids.push(child);
            }
        }
    }

    // Close all pipe fds in parent
    for (r, w) in pipes {
        close(r).ok();
        close(w).ok();
    }

    // Wait for all children, return last exit code
    let mut last_code = 0;
    for pid in child_pids {
        match waitpid(pid, None)? {
            WaitStatus::Exited(_, code) => last_code = code,
            _ => {}
        }
    }

    Ok(last_code)
}

fn apply_redirect(redirect: &Redirect) {
    use std::fs::OpenOptions;
    use std::os::unix::io::IntoRawFd;

    match redirect {
        Redirect::StdoutTo(file) => {
            if let Ok(f) = OpenOptions::new().write(true).create(true).truncate(true).open(file) {
                dup2(f.into_raw_fd(), 1).ok();
            }
        }
        Redirect::StdoutAppend(file) => {
            if let Ok(f) = OpenOptions::new().write(true).create(true).append(true).open(file) {
                dup2(f.into_raw_fd(), 1).ok();
            }
        }
        Redirect::StdinFrom(file) => {
            if let Ok(f) = OpenOptions::new().read(true).open(file) {
                dup2(f.into_raw_fd(), 0).ok();
            }
        }
        Redirect::StderrTo(file) => {
            if let Ok(f) = OpenOptions::new().write(true).create(true).truncate(true).open(file) {
                dup2(f.into_raw_fd(), 2).ok();
            }
        }
        Redirect::StderrToStdout => {
            dup2(1, 2).ok();
        }
    }
}

fn expand_vars(shell: &Shell, s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            match chars.peek() {
                Some(&'{') => {
                    chars.next();
                    let mut var = String::new();
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ch == '}' { break; }
                        var.push(ch);
                    }
                    let val = shell.env.get(&var)
                        .cloned()
                        .or_else(|| std::env::var(&var).ok())
                        .unwrap_or_default();
                    result.push_str(&val);
                }
                Some(&'?') => {
                    chars.next();
                    result.push_str(&shell.last_exit_code.to_string());
                }
                Some(&c) if c.is_alphanumeric() || c == '_' => {
                    let mut var = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            var.push(ch);
                            chars.next();
                        } else { break; }
                    }
                    let val = shell.env.get(&var)
                        .cloned()
                        .or_else(|| std::env::var(&var).ok())
                        .unwrap_or_default();
                    result.push_str(&val);
                }
                _ => result.push('$'),
            }
        } else {
            result.push(c);
        }
    }
    result
}