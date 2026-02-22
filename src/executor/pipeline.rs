// src/executor/pipeline.rs
//
// Pipeline execution — runs a sequence of commands connected by pipes,
// threading data between stages via either OS pipes (external commands)
// or in-memory buffers (builtins).

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
    let mut last_code = 0;
    let     n         = stages.len();

    for (i, (args, redirects)) in stages.into_iter().enumerate() {
        if args.is_empty() { continue; }
        let is_last = i == n - 1;

        if is_builtin_cmd(&args[0]) {
            // Write input to temp file for builtins like xargs that read it by path
            if let Some(ref buf) = input_buf {
                write_pipe_tmp(buf);
            }

            if is_last {
                last_code = match input_buf {
                    Some(ref buf) => run_builtin_with_input(shell, &args, buf),
                    None          => builtin::run_builtin(shell, &args).unwrap_or(0),
                };
            } else {
                // Capture this builtin's output in memory for the next stage
                input_buf = Some(capture_builtin_output(shell, &args, input_buf.as_deref()));
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

/// Capture a builtin's stdout into an in-memory Vec<u8>.
/// Uses OS pipes so no temp files are written for the capture itself.
fn capture_builtin_output(shell: &mut Shell, args: &[String], input: Option<&[u8]>) -> Vec<u8> {
    // cat with no file args is a pure pass-through — no need to run anything
    if args[0] == "cat" && args.len() == 1 {
        return input.unwrap_or_default().to_vec();
    }

    // For builtins that take file arguments, write input to temp file and
    // append the path as an argument (e.g. sort, grep, wc, uniq, head, tail)
    let mut new_args = args.to_vec();
    if let Some(data) = input {
        let tmp = pipe_in_tmp();
        let _ = std::fs::write(&tmp, data);
        new_args.push(tmp.to_string_lossy().to_string());
    }

    // Capture stdout using an OS pipe — stays entirely in memory
    capture_stdout_pipe(shell, &new_args)
}

/// Run the final builtin in a pipeline, feeding input via temp file.
fn run_builtin_with_input(shell: &mut Shell, args: &[String], input: &[u8]) -> i32 {
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

/// Capture a builtin's stdout using an OS pipe (in-memory, no disk I/O).
fn capture_stdout_pipe(shell: &mut Shell, args: &[String]) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::io::FromRawFd;

        unsafe {
            // Create a pipe: read_fd → write_fd
            let mut fds = [0i32; 2];
            if libc::pipe(fds.as_mut_ptr()) != 0 {
                return Vec::new();
            }
            let (read_fd, write_fd) = (fds[0], fds[1]);

            // Save stdout, replace it with the write end of the pipe
            let old_stdout = libc::dup(1);
            libc::dup2(write_fd, 1);
            libc::close(write_fd);

            // Run the builtin — its output goes into the pipe
            builtin::run_builtin(shell, args);

            // Flush and restore stdout
            libc::dup2(old_stdout, 1);
            libc::close(old_stdout);

            // Read everything from the pipe into a Vec<u8>
            let mut file = std::fs::File::from_raw_fd(read_fd);
            let mut buf  = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf).ok();
            buf
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::{FromRawHandle, IntoRawHandle};
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
        use windows_sys::Win32::System::Console::{
            GetStdHandle, SetStdHandle, STD_OUTPUT_HANDLE,
        };
        use windows_sys::Win32::System::Pipes::CreatePipe;

        unsafe {
            let mut sa = SECURITY_ATTRIBUTES {
                nLength:              std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: std::ptr::null_mut(),
                bInheritHandle:       1,
            };

            let mut read_handle  = INVALID_HANDLE_VALUE;
            let mut write_handle = INVALID_HANDLE_VALUE;

            if CreatePipe(&mut read_handle, &mut write_handle, &mut sa, 0) == 0 {
                return Vec::new();
            }

            // Save stdout and redirect it to the write end of the pipe
            let old_stdout = GetStdHandle(STD_OUTPUT_HANDLE);
            SetStdHandle(STD_OUTPUT_HANDLE, write_handle);

            // Run the builtin
            builtin::run_builtin(shell, args);

            // Restore stdout and close the write end so reads don't block
            SetStdHandle(STD_OUTPUT_HANDLE, old_stdout);
            windows_sys::Win32::Foundation::CloseHandle(write_handle);

            // Read from the pipe into a Vec<u8>
            let mut file = std::fs::File::from_raw_handle(read_handle as _);
            let mut buf  = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf).ok();
            buf
        }
    }
}

// ── External stage execution ──────────────────────────────────────────────────

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
        Err(e) => { report_spawn_error(&e); None }
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
        Err(e) => { report_spawn_error(&e); None }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns true if the command name is a shell builtin.
pub fn is_builtin_cmd(name: &str) -> bool {
    matches!(name,
        "cd"  | "pwd"   | "echo"  | "export" | "unset"  | "alias"  |
        "unalias" | "history" | "source" | "clear" | "cls"   | "sleep"  |
        "functions" | "help" | "which" | "pushd" | "popd"  | "dirs"   |
        "ls"  | "mkdir" | "rm"   | "cp"    | "mv"    | "cat"    |
        "touch" | "chmod" | "ln" | "grep"  | "find"  | "head"   |
        "tail"  | "wc"   | "env" | "sort"  | "uniq"  | "xargs"  |
        "jobs"  | "fg"   | "bg"  | "kill"  | "test"  | "["      |
        "true"  | "false"| "exit"| "quit"
    )
}

/// Write data to the pipe input temp file (used by xargs and similar).
fn write_pipe_tmp(data: &[u8]) {
    let _ = std::fs::write(pipe_in_tmp(), data);
}

fn pipe_in_tmp() -> std::path::PathBuf {
    std::env::temp_dir().join("rshell_pipe_in.tmp")
}

fn report_spawn_error(e: &std::io::Error) {
    if e.kind() == std::io::ErrorKind::NotFound {
        eprintln!("myshell: command not found");
    } else {
        eprintln!("myshell: {}", e);
    }
}