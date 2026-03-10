// src/shell/history.rs
//
// History loading, saving, and expansion (!!, !n).
// History is persisted to ~/.myshell_history across sessions.

use super::Shell;

const MAX_HISTORY: usize = 1000;

impl Shell {
    /// Load history from ~/.myshell_history into memory on startup.
    pub fn load_history(&mut self) {
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshell_history");

        if let Ok(content) = std::fs::read_to_string(&path) {
            self.history = content
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
        }
    }

    /// Append a single command to ~/.myshell_history.
    /// Trims the file to MAX_HISTORY lines when the limit is reached.
    pub fn save_history_line(&self, line: &str) {
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshell_history");

        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(file, "{}", line);
        }

        // Trim to MAX_HISTORY lines, keeping the most recent
        if self.history.len() >= MAX_HISTORY {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let trimmed: Vec<&str> = content
                    .lines()
                    .rev()
                    .take(MAX_HISTORY)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let _ = std::fs::write(&path, trimmed.join("\n") + "\n");
            }
        }
    }

    /// Expand history references (!!, !n) in an input string.
    pub fn expand_history(&self, input: &str) -> String {
        let input = input.trim();

        // !! — repeat last command
        if input == "!!" || input.starts_with("!! ") {
            if let Some(last) = self.history.iter().rev()
                .find(|h| h.as_str() != "!!" && !h.starts_with("!!"))
            {
                let suffix = input.strip_prefix("!!").unwrap_or("").trim();
                let expanded = if suffix.is_empty() {
                    last.clone()
                } else {
                    format!("{} {}", last, suffix)
                };
                eprintln!("{}", expanded);
                return expanded;
            }
            eprintln!("myshell: !!: event not found");
            return input.to_string();
        }

        // !n — repeat command n
        if input.starts_with('!') && input.len() > 1 {
            let rest = &input[1..];
            let (num_str, suffix) = rest.split_once(' ').unwrap_or((rest, ""));
            if let Ok(n) = num_str.parse::<usize>() {
                if n >= 1 && n <= self.history.len() {
                    let cmd = &self.history[n - 1];
                    let expanded = if suffix.is_empty() {
                        cmd.clone()
                    } else {
                        format!("{} {}", cmd, suffix)
                    };
                    eprintln!("{}", expanded);
                    return expanded;
                }
                eprintln!("myshell: !{}: event not found", n);
                return input.to_string();
            }
        }

        input.to_string()
    }
}