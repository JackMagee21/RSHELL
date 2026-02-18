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

    // Build readline with completion aware of our shell
    let mut readline = ShellReadline::new();

    loop {
        let prompt = shell.build_prompt();

        match readline.readline(&prompt) {
            Ok(line) => {
                let line: String = line.trim().to_string();
                if line.is_empty() { continue; }

                shell.history.push(line.clone());

                if let Err(e) = shell.eval(&line) {
                    eprintln!("\x1b[31mmyshell: {e}\x1b[0m");
                    shell.last_exit_code = 1;
                }
            }

            // ── Ctrl+C ────────────────────────────────────────────
            // Cancel current input, print a new prompt - do NOT exit
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                shell.last_exit_code = 130;
                // loop continues → new prompt is shown
            }

            // ── Ctrl+D ────────────────────────────────────────────
            // EOF - this is the intentional exit
            Err(ReadlineError::Eof) => {
                println!("exit");
                std::process::exit(shell.last_exit_code);
            }

            Err(ReadlineError::Other(e)) => {
                eprintln!("myshell: readline error: {e}");
                break;
            }
        }
    }
}