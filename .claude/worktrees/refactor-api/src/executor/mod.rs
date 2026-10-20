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
            // Expand vars then globs in each item, flattening glob matches
            let expanded_items: Vec<String> = items
                .iter()
                .flat_map(|item| {
                    let item = expand_vars(shell, item);
                    crate::glob::expand(&item)
                })
                .collect();
            for item in expanded_items {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::Shell;

    // ── expand_arithmetic ──────────────────────────────────────────────────

    #[test]
    fn arithmetic_basic_ops() {
        let shell = Shell::new();
        assert_eq!(expand_arithmetic(&shell, "$((2 + 3))"), "5");
        assert_eq!(expand_arithmetic(&shell, "$((10 - 4))"), "6");
        assert_eq!(expand_arithmetic(&shell, "$((3 * 4))"), "12");
        assert_eq!(expand_arithmetic(&shell, "$((10 / 3))"), "3");
        assert_eq!(expand_arithmetic(&shell, "$((10 % 3))"), "1");
    }

    #[test]
    fn arithmetic_precedence_and_parens() {
        let shell = Shell::new();
        assert_eq!(expand_arithmetic(&shell, "$((2 + 3 * 4))"), "14");
        assert_eq!(expand_arithmetic(&shell, "$(((2 + 3) * 4))"), "20");
    }

    #[test]
    fn arithmetic_variable_substitution() {
        // Unlike bash, bare identifiers inside $(( )) are NOT auto-expanded
        // here -- expand_arithmetic runs expand_vars (which only replaces
        // $-prefixed tokens) over the inner text before evaluating, so a
        // variable reference needs its own $ even inside the parens.
        let mut shell = Shell::new();
        shell.env.insert("x".to_string(), "7".to_string());
        assert_eq!(expand_arithmetic(&shell, "$(($x * 2))"), "14");
    }

    #[test]
    fn arithmetic_division_by_zero_degrades_to_zero_without_panicking() {
        // Documents current behavior: an arithmetic error is logged to
        // stderr and the expression expands to "0" rather than propagating
        // as an error or panicking.
        let shell = Shell::new();
        assert_eq!(expand_arithmetic(&shell, "$((1 / 0))"), "0");
    }

    #[test]
    fn arithmetic_unclosed_passes_through_literally() {
        let shell = Shell::new();
        assert_eq!(expand_arithmetic(&shell, "$((1 + 2"), "$((1 + 2");
    }

    // ── expand_vars ────────────────────────────────────────────────────────

    #[test]
    fn expands_named_variable() {
        let mut shell = Shell::new();
        shell.env.insert("FOO".to_string(), "bar".to_string());
        assert_eq!(expand_vars(&shell, "$FOO"), "bar");
        assert_eq!(expand_vars(&shell, "${FOO}"), "bar");
        assert_eq!(expand_vars(&shell, "x${FOO}y"), "xbary");
    }

    #[test]
    fn expands_unset_variable_to_empty() {
        let shell = Shell::new();
        assert_eq!(expand_vars(&shell, "$DEFINITELY_NOT_SET_XYZ"), "");
    }

    #[test]
    fn expands_last_exit_code() {
        let mut shell = Shell::new();
        shell.last_exit_code = 42;
        assert_eq!(expand_vars(&shell, "$?"), "42");
    }

    #[test]
    fn expands_positional_params_and_count() {
        let mut shell = Shell::new();
        shell.env.insert("1".to_string(), "a".to_string());
        shell.env.insert("2".to_string(), "b".to_string());
        assert_eq!(expand_vars(&shell, "$1"), "a");
        assert_eq!(expand_vars(&shell, "$2"), "b");
        assert_eq!(expand_vars(&shell, "$#"), "2");
        assert_eq!(expand_vars(&shell, "$@"), "a b");
    }

    #[test]
    fn expands_command_substitution() {
        let shell = Shell::new();
        // `echo hi` is valid under both cmd.exe (/C) and sh (-c), so this
        // is portable across the two branches in expand_command_substitution.
        // Only the trailing \n is stripped by the implementation, so on
        // Windows (CRLF) a trailing \r survives -- trim_end() here so the
        // test asserts "output was captured", not the CRLF-stripping detail.
        assert_eq!(expand_vars(&shell, "$(echo hi)").trim_end(), "hi");
    }

    // ── Shell::eval integration: control flow, set -e, aliases ──────────────

    /// Runs `script` through Shell::eval line by line and returns the final
    /// exit code, for tests that only need to observe pass/fail rather than
    /// captured stdout.
    fn run_script(shell: &mut Shell, script: &str) {
        for line in script.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let _ = shell.eval(line);
        }
    }

    #[test]
    fn if_executes_matching_branch_via_exit_code() {
        let mut shell = Shell::new();
        // `true`/`false` here are resolved as external commands by the OS
        // (both cmd.exe and POSIX shells provide them), so this exercises
        // the real If-command execution path end-to-end.
        run_script(&mut shell, "if true { export IF_TEST=yes }");
        assert_eq!(shell.env.get("IF_TEST").map(String::as_str), Some("yes"));
    }

    #[test]
    fn set_e_stops_block_body_on_first_failure() {
        // exit_on_error (set -e) is checked in run_block, which only
        // executes if/for/while bodies -- a bare top-level `;` sequence
        // (Command::Sequence) does not consult it at all, so the behavior
        // has to be exercised through a block body to be observed.
        let mut shell = Shell::new();
        shell.exit_on_error = true;

        let body: Vec<Command> = vec![
            crate::parser::parse("false").unwrap(),
            crate::parser::parse("export AFTER_FAILURE=should_not_run").unwrap(),
        ];
        let _ = run_block(&mut shell, body);
        assert_eq!(shell.env.get("AFTER_FAILURE"), None);

        shell.exit_on_error = false;
        let body: Vec<Command> = vec![
            crate::parser::parse("false").unwrap(),
            crate::parser::parse("export AFTER_FAILURE=should_run_now").unwrap(),
        ];
        let _ = run_block(&mut shell, body);
        assert_eq!(shell.env.get("AFTER_FAILURE").map(String::as_str), Some("should_run_now"));
    }

    #[test]
    fn set_dash_e_flag_is_recognized() {
        // `set`/`export` with -e / +e toggles Shell::exit_on_error -- this
        // just confirms that wiring, separately from run_block's enforcement
        // of it (covered above).
        let mut shell = Shell::new();
        run_script(&mut shell, "export -e");
        assert!(shell.exit_on_error);
        run_script(&mut shell, "export +e");
        assert!(!shell.exit_on_error);
    }

    #[test]
    fn alias_expands_before_dispatch() {
        let mut shell = Shell::new();
        shell.aliases.insert("myalias".to_string(), "export ALIAS_TEST=hit".to_string());
        shell.eval("myalias").expect("aliased command should run");
        assert_eq!(shell.env.get("ALIAS_TEST").map(String::as_str), Some("hit"));
    }
}