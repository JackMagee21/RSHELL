// src/shell/persist.rs
//
// Persists aliases and user-defined functions to ~/.myshellrc
// so they survive across shell sessions.

use super::Shell;

impl Shell {
    /// Write all current aliases back to ~/.myshellrc.
    pub fn save_aliases(&self) {
        let rc_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshellrc");

        let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();

        // Strip old alias lines, then re-append all current aliases
        let mut lines: Vec<String> = existing
            .lines()
            .filter(|l| !l.trim_start().starts_with("alias "))
            .map(|l| l.to_string())
            .collect();

        if !self.aliases.is_empty() {
            lines.push(String::new());
            lines.push("# aliases".to_string());
            let mut sorted: Vec<(&String, &String)> = self.aliases.iter().collect();
            sorted.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in sorted {
                lines.push(format!("alias {}='{}'", k, v));
            }
        }

        let content = lines.join("\n") + "\n";
        if let Err(e) = std::fs::write(&rc_path, content) {
            eprintln!("myshell: warning: could not save aliases: {}", e);
        }
    }

    /// Write all current user-defined functions back to ~/.myshellrc.
    pub fn save_functions(&self) {
        let rc_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshellrc");

        let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();

        // Strip old function blocks, then re-append all current functions
        let mut lines: Vec<String> = Vec::new();
        let mut in_func = false;
        for line in existing.lines() {
            if !in_func && (line.starts_with("function ") || line.contains("() {")) {
                in_func = true;
                continue;
            }
            if in_func {
                if line.trim() == "}" { in_func = false; }
                continue;
            }
            lines.push(line.to_string());
        }

        if !self.functions.is_empty() {
            lines.push(String::new());
            let mut sorted: Vec<(&String, &super::ShellFunction)> = self.functions.iter().collect();
            sorted.sort_by_key(|(k, _)| k.as_str());
            for (name, func) in sorted {
                lines.push(format!("function {}() {{", name));
                for line in &func.body {
                    lines.push(format!("    {}", line));
                }
                lines.push("}".to_string());
                lines.push(String::new());
            }
        }

        let content = lines.join("\n") + "\n";
        if let Err(e) = std::fs::write(&rc_path, content) {
            eprintln!("myshell: warning: could not save functions: {}", e);
        }
    }
}