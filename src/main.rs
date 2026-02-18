// src/main.rs
// myshell - CLI entry point

mod shell;
mod parser;
mod executor;
mod readline;

use shell::Shell;
use readline::{ShellReadline, ReadlineError};

fn main() {
    // Print welcome banner
    println!(
        "\x1b[36m
  ███╗   ███╗██╗   ██╗███████╗██╗  ██╗███████╗██╗     ██╗
  ████╗ ████║╚██╗ ██╔╝██╔════╝██║  ██║██╔════╝██║     ██║
  ██╔████╔██║ ╚████╔╝ ███████╗███████║█████╗  ██║     ██║
  ██║╚██╔╝██║  ╚██╔╝  ╚════██║██╔══██║██╔══╝  ██║     ██║
  ██║ ╚═╝ ██║   ██║   ███████║██║  ██║███████╗███████╗███████╗
  ╚═╝     ╚═╝   ╚═╝   ╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝
\x1b[0m  \x1b[90mType 'help' for commands. Ctrl+D or 'exit' to quit.\x1b[0m
"
    );

    let mut shell = Shell::new();
    let mut readline = ShellReadline::new();

    // Load config
    if let Err(e) = shell.load_rc() {
        eprintln!("myshell: warning: failed to load .myshellrc: {e}");
    }

    // REPL
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
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C - just show a new prompt
                println!();
                shell.last_exit_code = 130;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D
                println!("exit");
                break;
            }
            Err(ReadlineError::Other(e)) => {
                eprintln!("myshell: readline error: {e}");
                break;
            }
        }
    }
}