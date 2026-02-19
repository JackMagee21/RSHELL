// src/glob.rs
// Glob expansion engine
// Handles: * ? [abc] [a-z] ** (recursive)
// If a pattern matches nothing, it is returned as-is (bash behaviour)

use std::path::Path;

/// Expand a single argument that may contain glob characters.
/// Returns a sorted list of matches, or the original string if no matches.
pub fn expand(pattern: &str) -> Vec<String> {
    // Normalise separators first so Windows paths work cleanly
    let pattern = normalise_path(pattern);

    // Expand ~ at the start
    let expanded = expand_tilde(&pattern);

    // If no glob characters present, return as-is immediately
    if !has_glob_chars(&expanded) {
        return vec![expanded];
    }

    let matches = glob_expand(&expanded);

    if matches.is_empty() {
        vec![pattern.to_string()]
    } else {
        matches
    }
}

/// Expand a full argument list, replacing any glob patterns with their matches
pub fn expand_args(args: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for arg in args {
        result.extend(expand(&arg));
    }
    result
}

fn has_glob_chars(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Normalise a path — strip \\?\ long path prefix, unify separators to /
fn normalise_path(s: &str) -> String {
    let s = s.trim_start_matches("\\\\?\\");
    s.replace('\\', "/")
}

/// Expand ~ to home directory
fn expand_tilde(s: &str) -> String {
    if s == "~" {
        return dirs::home_dir()
            .map(|h| normalise_path(&h.display().to_string()))
            .unwrap_or_else(|| "~".to_string());
    }
    if s.starts_with("~/") {
        let home = dirs::home_dir()
            .map(|h| normalise_path(&h.display().to_string()))
            .unwrap_or_else(|| "~".to_string());
        return format!("{}/{}", home, &s[2..]);
    }
    s.to_string()
}

/// Core glob expansion
fn glob_expand(pattern: &str) -> Vec<String> {
    if pattern.contains("**/") || pattern == "**" {
        return expand_recursive(pattern);
    }

    let path = Path::new(pattern);

    let (dir, file_pat) = match path.parent() {
        Some(parent) if parent != Path::new("") => {
            let parent_str = normalise_path(&parent.display().to_string());
            let file = path.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent_str, file)
        }
        _ => (".".to_string(), pattern.to_string()),
    };

    let mut matches = Vec::new();

    let read_dir = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return matches,
    };

    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && !file_pat.starts_with('.') {
            continue;
        }
        if matches_pattern(&name, &file_pat) {
            let full_path = if dir == "." { name } else { format!("{}/{}", dir, name) };
            matches.push(full_path);
        }
    }

    matches.sort();
    matches
}

fn expand_recursive(pattern: &str) -> Vec<String> {
    let mut matches = Vec::new();

    let (start_dir, file_pat) = if let Some(pos) = pattern.find("**/") {
        let prefix = &pattern[..pos];
        let suffix = &pattern[pos + 3..];
        let start = if prefix.is_empty() { ".".to_string() }
                    else { prefix.trim_end_matches('/').to_string() };
        (start, suffix.to_string())
    } else {
        (".".to_string(), "*".to_string())
    };

    walk_dir(&start_dir, &file_pat, &mut matches);
    matches.sort();
    matches
}

fn walk_dir(dir: &str, file_pat: &str, matches: &mut Vec<String>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }

        let full = if dir == "." { name.clone() } else { format!("{}/{}", dir, name) };
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if matches_pattern(&name, file_pat) {
            matches.push(full.clone());
        }
        if is_dir {
            walk_dir(&full, file_pat, matches);
        }
    }
}

pub fn matches_pattern(name: &str, pattern: &str) -> bool {
    match_glob(name.as_bytes(), pattern.as_bytes())
}

fn match_glob(text: &[u8], pattern: &[u8]) -> bool {
    match (text, pattern) {
        ([], []) => true,
        (_, []) => false,
        (t, [b'*', rest @ ..]) => {
            for i in 0..=t.len() {
                if match_glob(&t[i..], rest) { return true; }
            }
            false
        }
        ([], _) => false,
        (t, [b'[', rest @ ..]) => {
            let (matched, after_bracket) = match_bracket(t[0], rest);
            if matched { match_glob(&t[1..], after_bracket) } else { false }
        }
        ([_, t_rest @ ..], [b'?', p_rest @ ..]) => match_glob(t_rest, p_rest),
        ([tc, t_rest @ ..], [pc, p_rest @ ..]) => {
            if tc == pc { match_glob(t_rest, p_rest) } else { false }
        }
    }
}

fn match_bracket(ch: u8, pattern: &[u8]) -> (bool, &[u8]) {
    let (negate, pat) = if pattern.first() == Some(&b'!') {
        (true, &pattern[1..])
    } else {
        (false, pattern)
    };

    let mut matched = false;
    let mut i = 0;

    while i < pat.len() && pat[i] != b']' {
        if i + 2 < pat.len() && pat[i + 1] == b'-' && pat[i + 2] != b']' {
            if ch >= pat[i] && ch <= pat[i + 2] { matched = true; }
            i += 3;
        } else {
            if ch == pat[i] { matched = true; }
            i += 1;
        }
    }

    let remaining = if i < pat.len() { &pat[i + 1..] } else { &pat[i..] };
    (if negate { !matched } else { matched }, remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_star() {
        assert!(matches_pattern("hello.rs", "*.rs"));
        assert!(!matches_pattern("main.py", "*.rs"));
    }

    #[test]
    fn test_question_mark() {
        assert!(matches_pattern("file1.rs", "file?.rs"));
        assert!(!matches_pattern("file10.rs", "file?.rs"));
    }

    #[test]
    fn test_bracket() {
        assert!(matches_pattern("file1.rs", "file[123].rs"));
        assert!(!matches_pattern("file4.rs", "file[123].rs"));
        assert!(matches_pattern("filea.rs", "file[a-z].rs"));
    }

    #[test]
    fn test_negate_bracket() {
        assert!(matches_pattern("file4.rs", "file[!123].rs"));
        assert!(!matches_pattern("file1.rs", "file[!123].rs"));
    }

    #[test]
    fn test_normalise() {
        assert_eq!(normalise_path("\\\\?\\C:\\Users\\foo"), "C:/Users/foo");
        assert_eq!(normalise_path("C:\\Users\\foo"), "C:/Users/foo");
    }
}