// src/main.rs
mod shell;
mod parser;
mod executor;
mod readline;
mod completion;
mod glob;

use shell::{Shell, JobStatus};
use readline::{ShellReadline, ReadlineError};

fn main() {
    print_banner();
    setup_platform();

    let mut shell = Shell::new();
    let mut readline = ShellReadline::new();

    shell.load_history();
    shell.load_rc().unwrap_or_else(|e| {
        eprintln!("rshell: warning: failed to load .myshellrc: {e}");
    });

    std::process::exit(run_repl(&mut shell, &mut readline));
}

// ── Platform setup ────────────────────────────────────────────────────────────

fn setup_platform() {
    // Unix signal handling
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::signal(libc::SIGTTIN, libc::SIG_IGN);
    }

    // Windows — enable ANSI escape processing so colours work correctly
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            GetConsoleMode, SetConsoleMode, GetStdHandle,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
        };
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            let mut mode = 0u32;
            if GetConsoleMode(handle, &mut mode) != 0 {
                SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }

    // Restore terminal on panic — without this a crash leaves the user's
    // terminal in raw mode and they have to type 'reset' blind to fix it
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crossterm::terminal::disable_raw_mode().ok();
        default_hook(info);
    }));
}

// ── Banner ────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!(
        "\x1b[36m
    ██████╗ ███████╗██╗  ██╗███████╗██╗     ██╗     
    ██╔══██╗██╔════╝██║  ██║██╔════╝██║     ██║     
    ██████╔╝███████╗███████║█████╗  ██║     ██║     
    ██╔══██╗╚════██║██╔══██║██╔══╝  ██║     ██║     
    ██║  ██║███████║██║  ██║███████╗███████╗███████╗
    ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝
\x1b[0m  \x1b[90mCtrl+C cancel  Ctrl+D exit  Ctrl+Z suspend  Ctrl+L clear\x1b[0m
"
    );
}

// ── REPL ──────────────────────────────────────────────────────────────────────

fn run_repl(shell: &mut Shell, readline: &mut ShellReadline) -> i32 {
    loop {
        check_background_jobs(shell);

        match read_command(shell, readline) {
            ReadResult::Input(input) => execute_input(shell, input),
            ReadResult::Interrupted  => {
                shell.last_exit_code = 130;
            }
            ReadResult::Eof => {
                println!("exit");
                return shell.last_exit_code;
            }
            ReadResult::Error(e) => {
                eprintln!("rshell: readline error: {e}");
                // Try to recover rather than killing the shell entirely
                crossterm::terminal::disable_raw_mode().ok();
                crossterm::terminal::enable_raw_mode().ok();
            }
        }
    }
}

// ── Input reading ─────────────────────────────────────────────────────────────

enum ReadResult {
    Input(String),
    Interrupted,
    Eof,
    Error(String),
}

/// Read a complete, possibly multiline, command from the user.
/// Keeps prompting with '...' until the input is syntactically complete.
fn read_command(shell: &Shell, readline: &mut ShellReadline) -> ReadResult {
    let mut input = String::new();

    loop {
        let prompt = if input.is_empty() {
            shell.build_prompt()
        } else {
            "\x1b[90m... \x1b[0m".to_string()
        };

        match readline.readline(&prompt) {
            Ok(line) => {
                let line = line.trim_end().to_string();

                if !input.is_empty() {
                    input.push('\n');
                }
                input.push_str(&line);

                if is_incomplete(&input) {
                    continue;
                }

                return ReadResult::Input(input);
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                return ReadResult::Interrupted;
            }
            Err(ReadlineError::Eof) => {
                return ReadResult::Eof;
            }
            Err(ReadlineError::Other(e)) => {
                return ReadResult::Error(e);
            }
        }
    }
}

// ── Execution ─────────────────────────────────────────────────────────────────

fn execute_input(shell: &mut Shell, input: String) {
    // Expand history references before anything else
    let input = shell.expand_history(&input);
    if input.is_empty() {
        return;
    }

    // Save to history in one place — removes the duplication between
    // shell.history and the file that existed before
    shell.add_history(input.clone());

    if let Err(e) = shell.eval(&input) {
        eprintln!("\x1b[31mrshell: {e}\x1b[0m");
        shell.last_exit_code = 1;
    }
}

// ── Background job reporting ──────────────────────────────────────────────────

fn check_background_jobs(shell: &mut Shell) {
    shell.reap_jobs();

    let done: Vec<_> = shell.jobs
        .iter()
        .filter(|(_, j)| j.status == JobStatus::Done)
        .map(|(id, j)| (*id, j.command.clone()))
        .collect();

    for (id, cmd) in done {
        println!("[{}] Done  {}", id, cmd);
        shell.jobs.remove(&id);
    }
}

// ── Incomplete input detection ────────────────────────────────────────────────

/// Returns true if the input is incomplete and we should keep reading.
/// Handles trailing operators, unclosed quotes, and open block keywords.
fn is_incomplete(input: &str) -> bool {
    let trimmed = input.trim_end();

    // Trailing operator means more input is expected
    if trimmed.ends_with('|')
        || trimmed.ends_with("&&")
        || trimmed.ends_with("||")
        || trimmed.ends_with('\\')
    {
        return true;
    }

    // Unclosed quotes
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_double => { chars.next(); } // skip escaped char
            '\'' if !in_double => in_single = !in_single,
            '"'  if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    if in_single || in_double {
        return true;
    }

    // Open block keywords — count opens vs closes so nested blocks work
    let open_blocks = count_keyword(trimmed, "do")
        + count_keyword(trimmed, "then")
        + count_keyword(trimmed, "function");

    let close_blocks = count_keyword(trimmed, "done")
        + count_keyword(trimmed, "fi")
        + brace_depth(trimmed);

    open_blocks > close_blocks
}

/// Count how many times a word-boundary keyword appears in a string.
fn count_keyword(s: &str, keyword: &str) -> usize {
    let mut count = 0;
    let mut pos   = 0;
    while pos + keyword.len() <= s.len() {
        if s[pos..].starts_with(keyword) {
            let before = pos == 0
                || s.as_bytes()[pos - 1].is_ascii_whitespace()
                || s.as_bytes()[pos - 1] == b';';
            let after_pos = pos + keyword.len();
            let after = after_pos == s.len()
                || s.as_bytes()[after_pos].is_ascii_whitespace()
                || s.as_bytes()[after_pos] == b';';
            if before && after {
                count += 1;
            }
        }
        pos += 1;
    }
    count
}

/// Returns negative depth if there are more } than {, zero if balanced.
/// We use this to detect open brace blocks.
fn brace_depth(s: &str) -> usize {
    let mut depth: i32 = 0;
    let mut in_single  = false;
    let mut in_double  = false;
    for c in s.chars() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"'  if !in_single => in_double = !in_double,
            '{'  if !in_single && !in_double => depth += 1,
            '}'  if !in_single && !in_double => depth -= 1,
            _ => {}
        }
    }
    // If depth > 0 there are unclosed braces — treat each as an open block
    depth.max(0) as usize
}