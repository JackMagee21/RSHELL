// src/executor/mod.rs
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
            shell.functions.insert(name.clone(), crate::shell::ShellFunction { name, body });
            Ok(0)
        }
        Command::FunctionCall { name, args } => {
            run_function(shell, &name, &args)
        }
    }
}

fn run_block(shell: &mut Shell, cmds: Vec<Command>) -> Result<i32> {
    let mut last_code = 0;
    for cmd in cmds {
        last_code = run(shell, cmd)?;
    }
    Ok(last_code)
}

fn run_function(shell: &mut Shell, name: &str, args: &[String]) -> Result<i32> {
    let func = match shell.functions.get(name).cloned() {
        Some(f) => f,
        None => { builtin::command_not_found(name); return Ok(127); }
    };

    let old_args: Vec<(String, Option<String>)> = (1..=9).map(|i| {
        let key = i.to_string();
        let old = shell.env.get(&key).cloned();
        (key, old)
    }).collect();

    for (i, arg) in args.iter().enumerate() {
        let key = (i + 1).to_string();
        shell.env.insert(key.clone(), arg.clone());
        unsafe { std::env::set_var(&key, arg); }
    }

    let mut last_code = 0;
    for line in &func.body {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        match shell.eval(line) {
            Ok(_) => last_code = shell.last_exit_code,
            Err(e) => { eprintln!("myshell: function {}: {}", name, e); last_code = 1; }
        }
    }

    for (key, old_val) in old_args {
        match old_val {
            Some(v) => { shell.env.insert(key.clone(), v.clone()); unsafe { std::env::set_var(&key, v); } }
            None => { shell.env.remove(&key); unsafe { std::env::remove_var(&key); } }
        }
    }

    Ok(last_code)
}

fn run_simple(
    shell: &mut Shell,
    mut args: Vec<String>,
    redirects: Vec<Redirect>,
    background: bool,
) -> Result<i32> {
    if args.is_empty() { return Ok(0); }

    for arg in &mut args {
        *arg = expand_arithmetic(shell, arg);
        *arg = expand_vars(shell, arg);
    }

    args = crate::glob::expand_args(args);

    if args[0] == "echo" && !redirects.is_empty() {
        let mut start = 1;
        let mut no_newline = false;
        if args.get(1).map(|s| s.as_str()) == Some("-n") { no_newline = true; start = 2; }
        let output = args[start..].join(" ").replace("\\n", "\n").replace("\\t", "\t");
        for redirect in &redirects {
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
    }

    if let Some(alias_val) = shell.aliases.get(&args[0]).cloned() {
        let alias_args: Vec<String> = alias_val.split_whitespace().map(String::from).collect();
        if alias_args[0] != args[0] {
            let mut new_args = alias_args;
            new_args.extend(args.into_iter().skip(1));
            args = new_args;
        }
    }

    if shell.functions.contains_key(&args[0]) {
        let name = args[0].clone();
        let func_args = args[1..].to_vec();
        return run_function(shell, &name, &func_args);
    }

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
    crossterm::terminal::disable_raw_mode().ok();

    let mut cmd = build_command(args, redirects)?;
    cmd.envs(&shell.env);

    let result = if background {
        match cmd.spawn() {
            Ok(child) => { println!("[bg] pid {}", child.id()); Ok(0) }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound { builtin::command_not_found(&args[0]); }
                else { eprintln!("myshell: {}: {}", args[0], e); }
                Ok(127)
            }
        }
    } else {
        match cmd.status() {
            Ok(status) => Ok(status.code().unwrap_or(0)),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound { builtin::command_not_found(&args[0]); }
                else { eprintln!("myshell: {}: {}", args[0], e); }
                Ok(127)
            }
        }
    };

    crossterm::terminal::enable_raw_mode().ok();
    result
}

fn build_command(args: &[String], redirects: &[Redirect]) -> Result<Proc> {
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
            Redirect::StderrToStdout => { cmd.stderr(Stdio::inherit()); }
        }
    }
    Ok(cmd)
}

fn run_pipeline(shell: &mut Shell, cmds: Vec<Command>) -> Result<i32> {
    if cmds.len() == 1 {
        return run(shell, cmds.into_iter().next().unwrap());
    }

    let n = cmds.len();
    let mut prev_stdout: Option<Stdio> = None;
    let mut children = Vec::new();

    crossterm::terminal::disable_raw_mode().ok();

    for (i, cmd) in cmds.into_iter().enumerate() {
        let (args, redirects) = match cmd {
            Command::Simple { args, redirects, .. } => (args, redirects),
            _ => continue,
        };
        if args.is_empty() { continue; }

        let mut cmd = match build_command(&args, &redirects) {
            Ok(c) => c,
            Err(e) => { eprintln!("myshell: {e}"); continue; }
        };
        cmd.envs(&shell.env);

        if let Some(prev) = prev_stdout.take() { cmd.stdin(prev); }

        let is_last = i == n - 1;
        if !is_last { cmd.stdout(Stdio::piped()); }

        match cmd.spawn() {
            Ok(mut child) => {
                if !is_last {
                    if let Some(stdout) = child.stdout.take() {
                        prev_stdout = Some(Stdio::from(stdout));
                    }
                }
                children.push(child);
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound { builtin::command_not_found(&args[0]); }
                else { eprintln!("myshell: {}: {}", args[0], e); }
            }
        }
    }

    let mut last_code = 0;
    for mut child in children {
        if let Ok(status) = child.wait() {
            last_code = status.code().unwrap_or(0);
        }
    }

    crossterm::terminal::enable_raw_mode().ok();
    Ok(last_code)
}

fn platform_command(program: &str) -> Proc {
    #[cfg(target_os = "windows")]
    {
        let cmd_builtins = ["dir","cls","type","copy","del","move","ren","md","rd","ver","vol"];
        if cmd_builtins.contains(&program.to_lowercase().as_str()) {
            let mut cmd = Proc::new("cmd");
            cmd.args(["/C", program]);
            return cmd;
        }
        Proc::new(program)
    }
    #[cfg(not(target_os = "windows"))]
    Proc::new(program)
}

pub fn expand_arithmetic_str(shell: &Shell, s: &str) -> String {
    expand_arithmetic(shell, s)
}

pub fn expand_arithmetic(shell: &Shell, s: &str) -> String {
    let mut result = String::new();
    let mut rest = s;

    while let Some(start) = rest.find("$((") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        if let Some(end) = after.find("))") {
            let expr = expand_vars(shell, &after[..end]);
            match eval_arithmetic(&expr) {
                Ok(val) => result.push_str(&val.to_string()),
                Err(e) => { eprintln!("myshell: arithmetic: {}", e); result.push_str("0"); }
            }
            rest = &after[end + 2..];
        } else {
            result.push_str("$((");
            rest = after;
        }
    }
    result.push_str(rest);
    result
}

fn eval_arithmetic(expr: &str) -> Result<i64> {
    parse_additive(expr.trim()).map(|(v, _)| v)
}

fn parse_additive(s: &str) -> Result<(i64, &str)> {
    let (mut left, mut rest) = parse_multiplicative(s)?;
    loop {
        let r = rest.trim_start();
        if r.starts_with('+') {
            let (right, new_rest) = parse_multiplicative(r[1..].trim_start())?;
            left += right; rest = new_rest;
        } else if r.starts_with('-') {
            let (right, new_rest) = parse_multiplicative(r[1..].trim_start())?;
            left -= right; rest = new_rest;
        } else { break; }
    }
    Ok((left, rest))
}

fn parse_multiplicative(s: &str) -> Result<(i64, &str)> {
    let (mut left, mut rest) = parse_unary(s)?;
    loop {
        let r = rest.trim_start();
        if r.starts_with('*') {
            let (right, new_rest) = parse_unary(r[1..].trim_start())?;
            left *= right; rest = new_rest;
        } else if r.starts_with('/') {
            let (right, new_rest) = parse_unary(r[1..].trim_start())?;
            if right == 0 { anyhow::bail!("division by zero"); }
            left /= right; rest = new_rest;
        } else if r.starts_with('%') {
            let (right, new_rest) = parse_unary(r[1..].trim_start())?;
            if right == 0 { anyhow::bail!("modulo by zero"); }
            left %= right; rest = new_rest;
        } else { break; }
    }
    Ok((left, rest))
}

fn parse_unary(s: &str) -> Result<(i64, &str)> {
    let s = s.trim_start();
    if s.starts_with('-') {
        let (val, rest) = parse_primary(s[1..].trim_start())?;
        Ok((-val, rest))
    } else if s.starts_with('+') {
        parse_primary(s[1..].trim_start())
    } else {
        parse_primary(s)
    }
}

fn parse_primary(s: &str) -> Result<(i64, &str)> {
    let s = s.trim_start();
    if s.starts_with('(') {
        let (val, rest) = parse_additive(s[1..].trim_start())?;
        let rest = rest.trim_start();
        if rest.starts_with(')') { Ok((val, &rest[1..])) }
        else { anyhow::bail!("expected closing )"); }
    } else {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if end == 0 { anyhow::bail!("expected number, got: {}", s); }
        Ok((s[..end].parse()?, &s[end..]))
    }
}

pub fn expand_vars(shell: &Shell, s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '$' { result.push(c); continue; }
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
            Some(&'?') => { chars.next(); result.push_str(&shell.last_exit_code.to_string()); }
            Some(&ch) if ch.is_alphanumeric() || ch == '_' => {
                let mut var = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' { var.push(ch); chars.next(); }
                    else { break; }
                }
                result.push_str(&lookup_var(shell, &var));
            }
            _ => result.push('$'),
        }
    }
    result
}

fn lookup_var(shell: &Shell, name: &str) -> String {
    shell.env.get(name).cloned()
        .or_else(|| std::env::var(name).ok())
        .unwrap_or_default()
}