// src/shell/prompt.rs
//
// Builds the prompt string shown before each input line.
// Handles path shortening and git branch display.

use super::Shell;

impl Shell {
    /// Build the prompt string for the current shell state.
    pub fn build_prompt(&self) -> String {
        let home = dirs::home_dir()
            .map(|h| h.display().to_string())
            .unwrap_or_default();

        let cwd = self.cwd.display().to_string();
        let cwd = cwd.trim_start_matches("\\\\?\\").to_string();

        let cwd = if cwd.starts_with(&home) {
            cwd.replacen(&home, "~", 1)
        } else {
            cwd
        };

        let short = shorten_path(&cwd);

        let code_indicator = if self.last_exit_code == 0 {
            "\x1b[32m❯\x1b[0m"
        } else {
            "\x1b[31m❯\x1b[0m"
        };

        let git_branch = get_git_branch()
            .map(|b| format!(" \x1b[35m({})\x1b[0m", b))
            .unwrap_or_default();

        format!("\x1b[34m{}\x1b[0m{} {} ", short, git_branch, code_indicator)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Show only the last two path components, e.g. "projects/rshell".
fn shorten_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 2 {
        return path.trim_start_matches('/').to_string();
    }
    parts[parts.len() - 2..].join("/")
}

/// Return the current git branch name, or None if not in a repo.
fn get_git_branch() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if branch.is_empty() { None } else { Some(branch) }
    } else {
        None
    }
}