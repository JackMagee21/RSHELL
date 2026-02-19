// src/completion/mod.rs
// Tab completion engine - completes file paths and command names

use std::path::PathBuf;

/// Given a partial word, return a list of completions
pub fn complete(partial: &str, is_first_word: bool) -> Vec<String> {
    if partial.is_empty() {
        return vec![];
    }

    let looks_like_path = partial.contains('/')
        || partial.contains('\\')
        || partial.starts_with('.')
        || partial.starts_with('~');

    if looks_like_path || !is_first_word {
        complete_path(partial)
    } else {
        let mut results = complete_commands(partial);
        results.extend(complete_path(partial));
        results.dedup();
        results
    }
}

/// Complete file and directory names
pub fn complete_path(partial: &str) -> Vec<String> {
    let expanded = if partial.starts_with('~') {
        let home = dirs::home_dir()
            .map(|h| h.display().to_string())
            .unwrap_or_else(|| "~".to_string());
        partial.replacen('~', &home, 1)
    } else {
        partial.to_string()
    };

    let (dir, prefix) = if expanded.contains('/') {
        let p = std::path::Path::new(&expanded);
        let dir = p.parent().unwrap_or(std::path::Path::new("."));
        let prefix = p.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (dir.to_path_buf(), prefix)
    } else {
        (PathBuf::from("."), expanded.clone())
    };

    let mut matches = Vec::new();

    let read_dir = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return matches,
    };

    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            let completion = if expanded.contains('/') {
                let base = dir.display().to_string();
                let sep = if base.ends_with('/') { "" } else { "/" };
                let trail = if is_dir { "/" } else { "" };
                let full = format!("{}{}{}{}", base, sep, name, trail);
                if partial.starts_with('~') {
                    let home = dirs::home_dir()
                        .map(|h| h.display().to_string())
                        .unwrap_or_default();
                    full.replacen(&home, "~", 1)
                } else {
                    full
                }
            } else {
                if is_dir { format!("{}/", name) } else { name }
            };

            matches.push(completion);
        }
    }

    matches.sort();
    matches
}

/// Complete command names from PATH
pub fn complete_commands(partial: &str) -> Vec<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    let mut commands = Vec::new();

    for dir in path_var.split(':') {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(partial) {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = entry.metadata() {
                        if meta.permissions().mode() & 0o111 != 0 {
                            commands.push(name);
                        }
                    }
                }
                #[cfg(windows)]
                {
                    commands.push(name);
                }
            }
        }
    }

    commands.sort();
    commands.dedup();
    commands
}

/// Shell builtin names for completion
pub fn builtin_names() -> &'static [&'static str] {
    &[
        "cd", "pwd", "echo", "export", "unset", "alias", "unalias",
        "history", "source", "help", "jobs", "fg", "bg", "kill",
        "clear", "cls", "exit", "quit", "ls", "true", "false",
        "test", "functions", "sleep", "mkdir",  // ← add here
    ]
}