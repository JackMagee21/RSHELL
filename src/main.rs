// src/main.rs
mod shell;
mod parser;
mod executor;
mod readline;
mod completion;
mod glob;

use shell::Shell;
use readline::{ShellReadline, ReadlineError};

fn main() {
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

    // Set up Ctrl+Z signal handler on Unix
    #[cfg(unix)]
    setup_signals();

    let mut shell = Shell::new();

    if let Err(e) = shell.load_rc() {
        eprintln!("myshell: warning: failed to load .myshellrc: {e}");
    }

    let mut readline = ShellReadline::new();

    loop {
        // Check and report any completed background jobs
        check_background_jobs(&mut shell);

        let prompt = shell.build_prompt();
        let mut input = String::new();

        loop {
            let line_prompt = if input.is_empty() {
                prompt.clone()
            } else {
                "\x1b[90m... \x1b[0m".to_string()
            };

            match readline.readline(&line_prompt) {
                Ok(line) => {
                    let line: String = line.trim_end().to_string();
                    if !input.is_empty() { input.push('\n'); }
                    input.push_str(&line);
                    if is_incomplete(&input) { continue; }
                    else { break; }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    shell.last_exit_code = 130;
                    input.clear();
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("exit");
                    std::process::exit(shell.last_exit_code);
                }
                Err(ReadlineError::Other(e)) => {
                    eprintln!("myshell: readline error: {e}");
                    std::process::exit(1);
                }
            }
        }

        let input = input.trim().to_string();
        if input.is_empty() { continue; }

        shell.history.push(input.clone());

        if let Err(e) = shell.eval(&input) {
            eprintln!("\x1b[31mmyshell: {e}\x1b[0m");
            shell.last_exit_code = 1;
        }
    }
}

/// Check for completed background jobs and notify user
fn check_background_jobs(shell: &mut Shell) {
    shell.reap_jobs();
    let done: Vec<_> = shell.jobs.iter()
        .filter(|(_, j)| j.status == shell::JobStatus::Done)
        .map(|(id, j)| (*id, j.command.clone()))
        .collect();

    for (id, cmd) in done {
        println!("[{}] Done  {}", id, cmd);
        shell.jobs.remove(&id);
    }
}

/// Set up Unix signal handlers
#[cfg(unix)]
fn setup_signals() {
    unsafe {
        // Ignore SIGTTOU so we can write to terminal from background
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::signal(libc::SIGTTIN, libc::SIG_IGN);
    }
}

fn is_incomplete(input: &str) -> bool {
    let trimmed = input.trim_end();
    if trimmed.ends_with('|')
        || trimmed.ends_with("&&")
        || trimmed.ends_with("||")
        || trimmed.ends_with('\\')
    {
        return true;
    }
    let mut in_single = false;
    let mut in_double = false;
    for ch in trimmed.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"'  if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    in_single || in_double
}