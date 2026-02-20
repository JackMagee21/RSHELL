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

/// Expand all $VAR, ${VAR}, and $? references in a string.
pub fn expand_vars(shell: &Shell, s: &str) -> String {
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

            // $VAR — unbraced variable name
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