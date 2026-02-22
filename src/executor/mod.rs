// src/executor/mod.rs
//
// Top-level executor — dispatches parsed AST nodes to the appropriate
// handler. The heavy lifting lives in submodules:
//
//   expand.rs   — variable and arithmetic expansion
//   pipeline.rs — pipe-connected command sequences

pub mod builtin;
mod expand;
mod pipeline;

use crate::parser::ast::{Command, Redirect};
use crate::shell::Shell;
use anyhow::Result;
use std::fs::OpenOptions;
use std::process::{Command as Proc, Stdio};

// Re-export the expand functions that other modules need
pub use expand::{expand_arithmetic, expand_vars};

// ── Public API ────────────────────────────────────────────────────────────────

pub fn execute(shell: &mut Shell, cmd: Command) -> Result<()> {
    let code = run(shell, cmd)?;
    shell.last_exit_code = code;
    Ok(())
}

// ── Command dispatch ──────────────────────────────────────────────────────────

pub fn run(shell: &mut Shell, cmd: Command) -> Result<i32> {
    match cmd {
        Command::Simple { args, redirects, background } => {
            run_simple(shell, args, redirects, background)
        }

        Command::Pipeline(cmds) => {
            pipeline::run_pipeline(shell, cmds)
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

        Command::If { condition, body, else_body } => {
            let code = run(shell, *condition)?;
            if code == 0 {
                run_block(shell, body)
            } else if let Some(else_cmds) = else_body {
                run_block(shell, else_cmds)
            } else {
                Ok(0)
            }
        }

        Command::For { var, items, body } => {
            let mut last_code = 0;
            for item in items {
                let item = expand_vars(shell, &item);
                shell.env.insert(var.clone(), item.clone());
                unsafe { std::env::set_var(&var, &item); }
                last_code = run_block(shell, body.clone())?;
            }
            Ok(last_code)
        }

        Command::While { condition, body } => {
            let mut last_code = 0;
            loop {
                let code = run(shell, *condition.clone())?;
                if code != 0 { break; }
                last_code = run_block(shell, body.clone())?;
            }
            Ok(last_code)
        }

        Command::FunctionDef { name, body } => {
            shell.functions.insert(
                name.clone(),
                crate::shell::ShellFunction { body },
            );
            shell.save_functions();
            Ok(0)
        }

        Command::FunctionCall { name, args } => {
            run_function(shell, &name, &args)
        }
    }
}

// ── Block / function execution ────────────────────────────────────────────────

fn run_block(shell: &mut Shell, cmds: Vec<Command>) -> Result<i32> {
    let mut last_code = 0;
    for cmd in cmds {
        last_code = run(shell, cmd)?;
        // set -e: stop on first non-zero exit
        if last_code != 0 && shell.exit_on_error {
            return Ok(last_code);
        }
    }
    Ok(last_code)
}

fn run_function(shell: &mut Shell, name: &str, args: &[String]) -> Result<i32> {
    let func = match shell.functions.get(name).cloned() {
        Some(f) => f,
        None    => { builtin::command_not_found(name); return Ok(127); }
    };

    // Save and set positional parameters $1..$9
    let saved_args = save_positional_args(shell);
    for (i, arg) in args.iter().enumerate() {
        let key = (i + 1).to_string();
        shell.env.insert(key.clone(), arg.clone());
        unsafe { std::env::set_var(&key, arg); }
    }

    // Execute function body
    let mut last_code = 0;
    for line in &func.body {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        match shell.eval(line) {
            Ok(_)  => last_code = shell.last_exit_code,
            Err(e) => { eprintln!("myshell: function {}: {}", name, e); last_code = 1; }
        }
    }

    // Restore positional parameters
    restore_positional_args(shell, saved_args);

    Ok(last_code)
}

/// Save $1..$9 so they can be restored after a function call.
fn save_positional_args(shell: &Shell) -> Vec<(String, Option<String>)> {
    (1..=9).map(|i| {
        let key = i.to_string();
        let old = shell.env.get(&key).cloned();
        (key, old)
    }).collect()
}

/// Restore $1..$9 after a function call.
fn restore_positional_args(shell: &mut Shell, saved: Vec<(String, Option<String>)>) {
    for (key, old_val) in saved {
        match old_val {
            Some(v) => {
                shell.env.insert(key.clone(), v.clone());
                unsafe { std::env::set_var(&key, v); }
            }
            None => {
                shell.env.remove(&key);
                unsafe { std::env::remove_var(&key); }
            }
        }
    }
}

// ── Simple command execution ──────────────────────────────────────────────────

fn run_simple(
    shell: &mut Shell,
    mut args: Vec<String>,
    redirects: Vec<Redirect>,
    background: bool,
) -> Result<i32> {
    if args.is_empty() { return Ok(0); }

    // Expand variables and arithmetic in all arguments
    for arg in &mut args {
        *arg = expand_arithmetic(shell, arg);
        *arg = expand_vars(shell, arg);
    }
    args = crate::glob::expand_args(args);

    // Special case: echo with redirects bypasses the normal builtin path
    if args[0] == "echo" && !redirects.is_empty() {
        return run_echo_redirect(&args, &redirects);
    }

    // Expand alias if one exists (but don't recurse on the same name)
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

    // User-defined function
    if shell.functions.contains_key(&args[0]) {
        let name      = args[0].clone();
        let func_args = args[1..].to_vec();
        return run_function(shell, &name, &func_args);
    }

    // Shell builtin
    if let Some(code) = builtin::run_builtin(shell, &args) {
        return Ok(code);
    }

    // External command
    run_external(shell, &args, &redirects, background)
}

/// Handle `echo` when its output is being redirected (> or >>).
fn run_echo_redirect(args: &[String], redirects: &[Redirect]) -> Result<i32> {
    let mut start      = 1;
    let mut no_newline = false;

    if args.get(1).map(|s| s.as_str()) == Some("-n") {
        no_newline = true;
        start = 2;
    }

    let output = args[start..].join(" ")
        .replace("\\n", "\n")
        .replace("\\t", "\t");

    for redirect in redirects {
        match redirect {
            Redirect::StdoutTo(file) => {
                let content = if no_newline { output.clone() } else { format!("{}\n", output) };
                return Ok(std::fs::write(file, content).map(|_| 0).unwrap_or(1));
            }
            Redirect::StdoutAppend(file) => {
                use std::io::Write;
                let content = if no_newline { output.clone() } else { format!("{}\n", output) };
                let mut f = OpenOptions::new().append(true).create(true).open(file)?;
                f.write_all(content.as_bytes())?;
                return Ok(0);
            }
            _ => {}
        }
    }

    Ok(0)
}

// ── External command execution ────────────────────────────────────────────────

fn run_external(
    shell: &Shell,
    args: &[String],
    redirects: &[Redirect],
    background: bool,
) -> Result<i32> {
    crossterm::terminal::disable_raw_mode().ok();

    let mut cmd = build_command(args, redirects)?;
    cmd.envs(&shell.env);

    let result = if background {
        spawn_background(cmd, &args[0])
    } else {
        run_foreground(cmd, &args[0])
    };

    crossterm::terminal::enable_raw_mode().ok();
    result
}

fn spawn_background(mut cmd: Proc, name: &str) -> Result<i32> {
    match cmd.spawn() {
        Ok(child) => { println!("[bg] pid {}", child.id()); Ok(0) }
        Err(e)    => { report_exec_error(name, &e); Ok(127) }
    }
}

fn run_foreground(mut cmd: Proc, name: &str) -> Result<i32> {
    match cmd.status() {
        Ok(status) => Ok(status.code().unwrap_or(0)),
        Err(e)     => { report_exec_error(name, &e); Ok(127) }
    }
}

fn report_exec_error(name: &str, e: &std::io::Error) {
    if e.kind() == std::io::ErrorKind::NotFound {
        builtin::command_not_found(name);
    } else {
        eprintln!("myshell: {}: {}", name, e);
    }
}

// ── Command building ──────────────────────────────────────────────────────────

pub fn build_command(args: &[String], redirects: &[Redirect]) -> Result<Proc> {
    let mut cmd = platform_command(&args[0]);
    cmd.args(&args[1..]);

    for redirect in redirects {
        match redirect {
            Redirect::StdoutTo(file) => {
                let f = OpenOptions::new().write(true).create(true).truncate(true).open(file)?;
                cmd.stdout(Stdio::from(f));
            }
            Redirect::StdoutAppend(file) => {
                let f = OpenOptions::new().write(true).create(true).append(true).open(file)?;
                cmd.stdout(Stdio::from(f));
            }
            Redirect::StdinFrom(file) => {
                let f = OpenOptions::new().read(true).open(file)?;
                cmd.stdin(Stdio::from(f));
            }
            Redirect::StderrTo(file) => {
                let f = OpenOptions::new().write(true).create(true).truncate(true).open(file)?;
                cmd.stderr(Stdio::from(f));
            }
            Redirect::StderrToStdout => {
                cmd.stderr(Stdio::inherit());
            }
        }
    }

    Ok(cmd)
}

/// On Windows, route known cmd.exe builtins through `cmd /C`.
fn platform_command(program: &str) -> Proc {
    #[cfg(windows)]
    {
        const CMD_BUILTINS: &[&str] = &[
            "dir", "cls", "type", "copy", "del",
            "move", "ren", "md", "rd", "ver", "vol",
        ];
        if CMD_BUILTINS.contains(&program.to_lowercase().as_str()) {
            let mut cmd = Proc::new("cmd");
            cmd.args(["/C", program]);
            return cmd;
        }
    }
    Proc::new(program)
}