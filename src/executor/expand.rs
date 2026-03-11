// src/executor/expand.rs
//
// Variable expansion, arithmetic expansion, and related helpers.
// These are called throughout the executor and are public so shell.rs
// can also call expand_arithmetic directly.

use crate::shell::Shell;
use anyhow::Result;

// ── Public API ────────────────────────────────────────────────────────────────

/// Expand all $((expr)) arithmetic in a string.
pub fn expand_arithmetic(shell: &Shell, s: &str) -> String {
    let mut result = String::new();
    let mut rest = s;

    while let Some(start) = rest.find("$((") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 3..];

        if let Some(end) = after.find("))") {
            let expr = expand_vars(shell, &after[..end]);
            match eval_arithmetic(&expr) {
                Ok(val)  => result.push_str(&val.to_string()),
                Err(e)   => {
                    eprintln!("myshell: arithmetic: {}", e);
                    result.push_str("0");
                }
            }
            rest = &after[end + 2..];
        } else {
            // No closing )) — pass through literally
            result.push_str("$((");
            rest = after;
        }
    }

    result.push_str(rest);
    result
}

/// Expand all $VAR, ${VAR}, $?, $@, $#, $*, $$, and $(cmd) references in a string.
pub fn expand_vars(shell: &Shell, s: &str) -> String {
    // First handle command substitution $(...) — must be done before char-by-char pass
    let s = expand_command_substitution(shell, s);
    let mut result = String::new();
    let mut chars  = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '$' {
            result.push(c);
            continue;
        }

        match chars.peek() {
            // ${VAR} — braced variable
            Some(&'{') => {
                chars.next();
                let mut var = String::new();
                for ch in chars.by_ref() {
                    if ch == '}' { break; }
                    var.push(ch);
                }
                result.push_str(&lookup_var(shell, &var));
            }

            // $? — last exit code
            Some(&'?') => {
                chars.next();
                result.push_str(&shell.last_exit_code.to_string());
            }

            // $$ — current process id
            Some(&'$') => {
                chars.next();
                result.push_str(&std::process::id().to_string());
            }

            // $# — number of positional args
            Some(&'#') => {
                chars.next();
                let count = (1..=9)
                    .filter(|i| shell.env.contains_key(&i.to_string()))
                    .count();
                result.push_str(&count.to_string());
            }

            // $@ and $* — all positional args space-separated
            Some(&'@') | Some(&'*') => {
                chars.next();
                let args: Vec<String> = (1..=9)
                    .filter_map(|i| shell.env.get(&i.to_string()).cloned())
                    .collect();
                result.push_str(&args.join(" "));
            }

            // $VAR / $1..$9 — unbraced variable name or positional param
            Some(&ch) if ch.is_alphanumeric() || ch == '_' => {
                let mut var = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        var.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                result.push_str(&lookup_var(shell, &var));
            }

            // Bare $ with no variable — pass through
            _ => result.push('$'),
        }
    }

    result
}

/// Expand $(command) substitutions by running the command and capturing output.
fn expand_command_substitution(shell: &Shell, s: &str) -> String {
    let mut result = String::new();
    let mut rest = s;

    while let Some(start) = rest.find("$(") {
        // Make sure it's not $(( arithmetic — that's handled separately
        if rest[start..].starts_with("$((") {
            // Copy up to and including $((  and skip ahead so we don't loop on it
            let end = start + 3;
            result.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }

        result.push_str(&rest[..start]);
        let inner_start = start + 2;

        // Find the matching closing ) — handle nesting
        let inner = &rest[inner_start..];
        let mut depth = 1;
        let mut end = 0;
        for (i, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 { end = i; break; }
                }
                _ => {}
            }
        }

        if depth != 0 {
            // Unmatched $( — pass through literally
            result.push_str("$(");
            rest = &rest[inner_start..];
            continue;
        }

        let cmd_str = &inner[..end];
        rest = &rest[inner_start + end + 1..];

        // Run the command and capture stdout
        let output = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
            .args(if cfg!(windows) { vec!["/C", cmd_str] } else { vec!["-c", cmd_str] })
            .envs(&shell.env)
            .output();

        match output {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                result.push_str(text.trim_end_matches('\n'));
            }
            Err(_) => {} // silently expand to empty on failure
        }
    }

    result.push_str(rest);
    result
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn lookup_var(shell: &Shell, name: &str) -> String {
    shell.env.get(name).cloned()
        .or_else(|| std::env::var(name).ok())
        .unwrap_or_default()
}

fn eval_arithmetic(expr: &str) -> Result<i64> {
    parse_additive(expr.trim()).map(|(v, _)| v)
}

fn parse_additive(s: &str) -> Result<(i64, &str)> {
    let (mut left, mut rest) = parse_multiplicative(s)?;
    loop {
        let r = rest.trim_start();
        if r.starts_with('+') {
            let (right, new_rest) = parse_multiplicative(r[1..].trim_start())?;
            left += right;
            rest = new_rest;
        } else if r.starts_with('-') {
            let (right, new_rest) = parse_multiplicative(r[1..].trim_start())?;
            left -= right;
            rest = new_rest;
        } else {
            break;
        }
    }
    Ok((left, rest))
}

fn parse_multiplicative(s: &str) -> Result<(i64, &str)> {
    let (mut left, mut rest) = parse_unary(s)?;
    loop {
        let r = rest.trim_start();
        if r.starts_with('*') {
            let (right, new_rest) = parse_unary(r[1..].trim_start())?;
            left *= right;
            rest = new_rest;
        } else if r.starts_with('/') {
            let (right, new_rest) = parse_unary(r[1..].trim_start())?;
            if right == 0 { anyhow::bail!("division by zero"); }
            left /= right;
            rest = new_rest;
        } else if r.starts_with('%') {
            let (right, new_rest) = parse_unary(r[1..].trim_start())?;
            if right == 0 { anyhow::bail!("modulo by zero"); }
            left %= right;
            rest = new_rest;
        } else {
            break;
        }
    }
    Ok((left, rest))
}

fn parse_unary(s: &str) -> Result<(i64, &str)> {
    let s = s.trim_start();
    if s.starts_with('-') {
        let (val, rest) = parse_primary(s[1..].trim_start())?;
        Ok((-val, rest))
    } else if s.starts_with('+') {
        parse_primary(s[1..].trim_start())
    } else {
        parse_primary(s)
    }
}

fn parse_primary(s: &str) -> Result<(i64, &str)> {
    let s = s.trim_start();
    if s.starts_with('(') {
        let (val, rest) = parse_additive(s[1..].trim_start())?;
        let rest = rest.trim_start();
        if rest.starts_with(')') {
            Ok((val, &rest[1..]))
        } else {
            anyhow::bail!("expected closing )");
        }
    } else {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if end == 0 { anyhow::bail!("expected number, got: {}", s); }
        Ok((s[..end].parse()?, &s[end..]))
    }
}