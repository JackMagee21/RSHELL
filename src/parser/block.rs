// src/parser/block.rs
//
// Parsers for block-level control flow: if, for, while.
// Also contains the shared helpers for extracting block bodies
// (brace style { } and keyword style then...fi / do...done).

use anyhow::{Result, bail};
use super::ast::Command;

// ── Control flow parsers ──────────────────────────────────────────────────────

/// Parse: if <condition>; then <body> [else <else_body>] fi
/// Or:    if <condition> { <body> } [else { <else_body> }]
pub fn parse_if(input: &str) -> Result<Command> {
    let rest = input[2..].trim();

    let (cond_str, remainder) = if let Some(then_pos) = find_keyword(rest, "then") {
        (rest[..then_pos].trim(), rest[then_pos + 4..].trim())
    } else if rest.contains('{') {
        let brace = rest.find('{').unwrap();
        (rest[..brace].trim(), rest[brace..].trim())
    } else {
        bail!("if: expected 'then' or '{{'");
    };

    let condition = super::parse(cond_str)?;
    let (body_str, else_str) = extract_block(remainder)?;
    let body = parse_block_lines(&body_str)?;

    let else_body = if let Some(else_content) = else_str {
        let else_content = else_content.trim();
        let else_content = if else_content.starts_with('{') {
            let inner = else_content.trim_start_matches('{').trim_end_matches('}').trim();
            inner.to_string()
        } else {
            else_content.to_string()
        };
        Some(parse_block_lines(&else_content)?)
    } else {
        None
    };

    Ok(Command::If {
        condition: Box::new(condition),
        body,
        else_body,
    })
}

/// Parse: for <var> in <items...>; do <body> done
/// Or:    for <var> in <items...> { <body> }
pub fn parse_for(input: &str) -> Result<Command> {
    let rest = input[3..].trim();

    let var_end = rest.find(|c: char| c.is_whitespace())
        .ok_or_else(|| anyhow::anyhow!("for: expected variable name"))?;
    let var = rest[..var_end].to_string();
    let rest = rest[var_end..].trim();

    if !rest.starts_with("in ") && rest != "in" {
        bail!("for: expected 'in' after variable");
    }
    let rest = rest[2..].trim();

    let (items_str, body_str) = if let Some(do_pos) = find_keyword(rest, "do") {
        let items = rest[..do_pos].trim();
        let body_and_done = rest[do_pos + 2..].trim();
        let (body, _) = extract_block_done(body_and_done)?;
        (items.to_string(), body)
    } else if let Some(brace_pos) = rest.find('{') {
        let items = rest[..brace_pos].trim();
        let (body, _) = extract_block(&rest[brace_pos..])?;
        (items.to_string(), body)
    } else {
        bail!("for: expected 'do' or '{{'");
    };

    let items: Vec<String> = items_str
        .split_whitespace()
        .flat_map(|item| {
            if item.contains('*') || item.contains('?') {
                expand_glob(item)
            } else {
                vec![item.to_string()]
            }
        })
        .collect();

    let body = parse_block_lines(&body_str)?;

    Ok(Command::For { var, items, body })
}

/// Parse: while <condition>; do <body> done
pub fn parse_while(input: &str) -> Result<Command> {
    let rest = input[5..].trim();

    let (cond_str, remainder) = if let Some(do_pos) = find_keyword(rest, "do") {
        (rest[..do_pos].trim(), rest[do_pos + 2..].trim())
    } else if rest.contains('{') {
        let brace = rest.find('{').unwrap();
        (rest[..brace].trim(), rest[brace..].trim())
    } else {
        bail!("while: expected 'do' or '{{'");
    };

    let condition = super::parse(cond_str)?;
    let (body_str, _) = extract_block(remainder)?;
    let body = parse_block_lines(&body_str)?;

    Ok(Command::While {
        condition: Box::new(condition),
        body,
    })
}

// ── Block extraction helpers ──────────────────────────────────────────────────

/// Extract content between { } or then...fi, returning (body, optional_else).
pub fn extract_block(s: &str) -> Result<(String, Option<String>)> {
    let s = s.trim();

    if s.starts_with('{') {
        // Brace style: count depth to find matching }
        let mut depth = 0;
        let mut end = 0;
        for (i, ch) in s.chars().enumerate() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 { end = i; break; }
                }
                _ => {}
            }
        }
        let body = s[1..end].trim().to_string();
        let after = s[end + 1..].trim();
        let else_part = if after.starts_with("else") {
            Some(after[4..].trim().to_string())
        } else {
            None
        };
        Ok((body, else_part))
    } else {
        // Keyword style: then...fi
        if let Some(fi_pos) = find_keyword(s, "fi") {
            let content = s[..fi_pos].trim();
            if let Some(else_pos) = find_keyword(content, "else") {
                let body = content[..else_pos].trim().to_string();
                let else_body = content[else_pos + 4..].trim().to_string();
                Ok((body, Some(else_body)))
            } else {
                Ok((content.to_string(), None))
            }
        } else {
            Ok((s.to_string(), None))
        }
    }
}

/// Extract content between do...done.
pub fn extract_block_done(s: &str) -> Result<(String, &str)> {
    let s = s.trim();
    if let Some(done_pos) = find_keyword(s, "done") {
        Ok((s[..done_pos].trim().to_string(), &s[done_pos + 4..]))
    } else {
        Ok((s.to_string(), ""))
    }
}

/// Parse multiple semicolon/newline separated lines into a Vec of Commands.
pub fn parse_block_lines(block: &str) -> Result<Vec<Command>> {
    let mut cmds = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        for part in line.split(';') {
            let part = part.trim();
            if part.is_empty() { continue; }
            cmds.push(super::parse(part)?);
        }
    }
    Ok(cmds)
}

// ── Shared utility ────────────────────────────────────────────────────────────

/// Find a keyword at a word boundary within s, returning its byte offset.
pub fn find_keyword(s: &str, keyword: &str) -> Option<usize> {
    let mut pos = 0;
    while pos + keyword.len() <= s.len() {
        if s[pos..].starts_with(keyword) {
            let before_ok = pos == 0 || s.as_bytes()[pos - 1].is_ascii_whitespace();
            let after_ok  = pos + keyword.len() == s.len()
                || s.as_bytes()[pos + keyword.len()].is_ascii_whitespace();
            if before_ok && after_ok {
                return Some(pos);
            }
        }
        pos += 1;
    }
    None
}

fn expand_glob(pattern: &str) -> Vec<String> {
    match glob::glob(pattern) {
        Ok(paths) => {
            let results: Vec<String> = paths
                .filter_map(|p| p.ok())
                .map(|p| p.display().to_string())
                .collect();
            if results.is_empty() { vec![pattern.to_string()] } else { results }
        }
        Err(_) => vec![pattern.to_string()],
    }
}