// src/executor/pipeline.rs
//
// Pipeline execution — runs a sequence of commands connected by pipes,
// threading data between stages via either OS pipes (external commands)
// or temp files (builtins).

use crate::parser::ast::{Command, Redirect};
use crate::shell::Shell;
use anyhow::Result;
use std::process::Stdio;

use super::builtin;
use super::expand::{expand_arithmetic, expand_vars};

// ── Public API ────────────────────────────────────────────────────────────────

/// Run a pipeline of commands, connecting stdout of each to stdin of the next.
pub fn run_pipeline(shell: &mut Shell, cmds: Vec<Command>) -> Result<i32> {
    // Single command — no piping needed
    if cmds.len() == 1 {
        return super::run(shell, cmds.into_iter().next().unwrap());
    }

    let stages = collect_stages(shell, cmds);
    if stages.is_empty() { return Ok(0); }

    let mut input_buf: Option<Vec<u8>> = None;
    let mut last_code  = 0;
    let     n          = stages.len();

    for (i, (args, redirects)) in stages.into_iter().enumerate() {
        if args.is_empty() { continue; }
        let is_last = i == n - 1;

        if is_builtin_cmd(&args[0]) {
            last_code = run_builtin_stage(shell, &args, input_buf.as_deref(), is_last);
            if is_last {
                input_buf = None;
            } else {
                input_buf = Some(capture_builtin(shell, &args, input_buf.as_deref()));
            }
        } else {
            input_buf = run_external_stage(
                shell, &args, &redirects, input_buf, is_last, &mut last_code,
            );
        }
    }

    Ok(last_code)
}

// ── Stage collection ──────────────────────────────────────────────────────────

/// Expand and collect all pipeline stages into (args, redirects) pairs.
fn collect_stages(
    shell: &mut Shell,
    cmds: Vec<Command>,
) -> Vec<(Vec<String>, Vec<Redirect>)> {
    let mut stages = Vec::new();
    for cmd in cmds {
        if let Command::Simple { args, redirects, .. } = cmd {
            let mut expanded = args;
            for arg in &mut expanded {
                *arg = expand_arithmetic(shell, arg);
                *arg = expand_vars(shell, arg);
            }
            expanded = crate::glob::expand_args(expanded);
            stages.push((expanded, redirects));
        }
    }
    stages
}

// ── Builtin stage execution ───────────────────────────────────────────────────

/// Run a builtin in a pipeline, feeding it input from the previous stage.
fn run_builtin_stage(
    shell: &mut Shell,
    args: &[String],
    input: Option<&[u8]>,
    is_last: bool,
) -> i32 {
    // Always write input to temp file so builtins like xargs can read it
    if let Some(buf) = input {
        write_pipe_tmp(buf);
    }

    if is_last {
        match input {
            Some(buf) => run_builtin_with_input(shell, args, buf),
            None      => builtin::run_builtin(shell, args).unwrap_or(0),
        }
    } else {
        0 // capture_builtin handles the actual run for non-last stages
    }
}

/// Capture a builtin's stdout into a Vec<u8> for the next pipeline stage.
pub fn capture_builtin(shell: &mut Shell, args: &[String], input: Option<&[u8]>) -> Vec<u8> {
    // cat with no file args is a pure pass-through
    if args[0] == "cat" && args.len() == 1 {
        return input.unwrap_or_default().to_vec();
    }

    let mut new_args = args.to_vec();

    // Write input to temp file and pass it as a file argument
    if let Some(data) = input {
        let tmp = pipe_in_tmp();
        let _ = std::fs::write(&tmp, data);
        new_args.push(tmp.to_string_lossy().to_string());
    }

    capture_stdout(shell, &new_args)
}

/// Run the final builtin stage of a pipeline, feeding input via temp file.
fn run_builtin_with_input(shell: &mut Shell, args: &[String], input: &[u8]) -> i32 {
    // cat with no file just prints the buffer directly
    if args[0] == "cat" && args.len() == 1 {
        use std::io::Write;
        std::io::stdout().write_all(input).ok();
        return 0;
    }

    let tmp = pipe_in_tmp();
    let _ = std::fs::write(&tmp, input);

    let mut new_args = args.to_vec();
    new_args.push(tmp.to_string_lossy().to_string());

    builtin::run_builtin(shell, &new_args).unwrap_or(0)
}

/// Redirect stdout to a temp file, run a builtin, restore stdout, return captured bytes.
fn capture_stdout(shell: &mut Shell, args: &[String]) -> Vec<u8> {
    let tmp = pipe_out_tmp();

    #[cfg(unix)]
    unsafe {
        use std::os::unix::io::IntoRawFd;
        let Ok(file) = std::fs::File::create(&tmp) else { return Vec::new() };
        let fd = file.into_raw_fd();
        let old = libc::dup(1);
        libc::dup2(fd, 1);
        libc::close(fd);
        builtin::run_builtin(shell, args);
        libc::dup2(old, 1);
        libc::close(old);
    }

    #[cfg(windows)]
    unsafe {
        use std::os::windows::io::IntoRawHandle;
        use windows_sys::Win32::System::Console::{
            GetStdHandle, SetStdHandle, STD_OUTPUT_HANDLE,
        };
        let Ok(file) = std::fs::File::create(&tmp) else { return Vec::new() };
        let handle = file.into_raw_handle();
        let old    = GetStdHandle(STD_OUTPUT_HANDLE);
        SetStdHandle(STD_OUTPUT_HANDLE, handle as *mut std::ffi::c_void);
        builtin::run_builtin(shell, args);
        SetStdHandle(STD_OUTPUT_HANDLE, old);
    }

    std::fs::read(&tmp).unwrap_or_default()
}

// ── External stage execution ──────────────────────────────────────────────────

/// Run an external command as one stage of a pipeline.
/// Returns the new input_buf for the next stage (or None if this is the last).
fn run_external_stage(
    shell: &Shell,
    args: &[String],
    redirects: &[Redirect],
    input_buf: Option<Vec<u8>>,
    is_last: bool,
    last_code: &mut i32,
) -> Option<Vec<u8>> {
    crossterm::terminal::disable_raw_mode().ok();

    let mut cmd = match super::build_command(args, redirects) {
        Ok(c)  => c,
        Err(e) => { eprintln!("myshell: {e}"); return None; }
    };
    cmd.envs(&shell.env);

    let result = match input_buf {
        Some(buf) => run_external_with_input(cmd, buf, is_last, last_code),
        None      => run_external_no_input(cmd, is_last, last_code),
    };

    crossterm::terminal::enable_raw_mode().ok();
    result
}

fn run_external_with_input(
    mut cmd: std::process::Command,
    buf: Vec<u8>,
    is_last: bool,
    last_code: &mut i32,
) -> Option<Vec<u8>> {
    cmd.stdin(Stdio::piped());
    if !is_last { cmd.stdout(Stdio::piped()); }

    match cmd.spawn() {
        Ok(mut child) => {
            // Write buffer to the child's stdin
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(&buf);
            }
            if !is_last {
                child.wait_with_output().ok().map(|o| o.stdout)
            } else {
                *last_code = child.wait().map(|s| s.code().unwrap_or(0)).unwrap_or(0);
                None
            }
        }
        Err(e) => { report_spawn_error(&e, &[]); None }
    }
}

fn run_external_no_input(
    mut cmd: std::process::Command,
    is_last: bool,
    last_code: &mut i32,
) -> Option<Vec<u8>> {
    if !is_last { cmd.stdout(Stdio::piped()); }

    match cmd.spawn() {
        Ok(mut child) => {
            if !is_last {
                child.wait_with_output().ok().map(|o| o.stdout)
            } else {
                *last_code = child.wait().map(|s| s.code().unwrap_or(0)).unwrap_or(0);
                None
            }
        }
        Err(e) => { report_spawn_error(&e, &[]); None }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns true if the command name is a shell builtin.
pub fn is_builtin_cmd(name: &str) -> bool {
    matches!(name,
        "cd"  | "pwd"   | "echo"    | "export"  | "unset"  | "alias"  |
        "unalias" | "history" | "source" | "clear"  | "cls"   | "sleep"  |
        "functions" | "help" | "which"  | "pushd"  | "popd"  | "dirs"   |
        "ls"  | "mkdir" | "rm"     | "cp"      | "mv"    | "cat"    |
        "touch" | "chmod" | "ln"   | "grep"    | "find"  | "head"   |
        "tail"  | "wc"   | "env"   | "sort"    | "uniq"  | "xargs"  |
        "jobs"  | "fg"   | "bg"    | "kill"    | "test"  | "["      |
        "true"  | "false"| "exit"  | "quit"
    )
}

fn write_pipe_tmp(data: &[u8]) {
    let _ = std::fs::write(pipe_in_tmp(), data);
}

fn pipe_in_tmp()  -> std::path::PathBuf {
    std::env::temp_dir().join("rshell_pipe_in.tmp")
}

fn pipe_out_tmp() -> std::path::PathBuf {
    std::env::temp_dir().join("rshell_pipe_out.tmp")
}

fn report_spawn_error(e: &std::io::Error, _args: &[String]) {
    if e.kind() == std::io::ErrorKind::NotFound {
        eprintln!("myshell: command not found");
    } else {
        eprintln!("myshell: {}", e);
    }
}