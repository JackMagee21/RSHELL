// src/main.rs
mod shell;
mod parser;
mod executor;
mod readline;
mod completion;

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
\x1b[0m  \x1b[90mType 'help' for commands.  Ctrl+C to cancel  Ctrl+D to exit  Ctrl+L to clear\x1b[0m
"
    );

    let mut shell = Shell::new();

    if let Err(e) = shell.load_rc() {
        eprintln!("myshell: warning: failed to load .myshellrc: {e}");
    }

    let mut readline = ShellReadline::new();

    loop {
        let prompt = shell.build_prompt();
        let mut input = String::new();

        // ── Multiline input loop ───────────────────────────────
        loop {
            let line_prompt = if input.is_empty() {
                prompt.clone()
            } else {
                // Continuation prompt
                "\x1b[90m... \x1b[0m".to_string()
            };

            match readline.readline(&line_prompt) {
                Ok(line) => {
                    let line: String = line.trim_end().to_string();

                    if !input.is_empty() {
                        input.push('\n');
                    }
                    input.push_str(&line);

                    // Check if input is incomplete (ends with | && || \ )
                    if is_incomplete(&input) {
                        continue; // show ... prompt and keep reading
                    } else {
                        break; // input is complete, run it
                    }
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

/// Returns true if the input looks incomplete and needs more lines
fn is_incomplete(input: &str) -> bool {
    let trimmed = input.trim_end();

    // Ends with a pipe or logical operator
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
    for ch in trimmed.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"'  if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    if in_single || in_double {
        return true;
    }

    false
}