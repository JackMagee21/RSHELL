// src/parser/mod.rs
pub mod ast;

use ast::{Command, Redirect};
use anyhow::{Result, bail};

pub fn parse(input: &str) -> Result<Command> {
    // Handle multiline - join lines ending with \ or operators
    let input = input.trim();
    if input.is_empty() {
        bail!("empty input");
    }

    // Check for block-level keywords first
    if input.starts_with("if ") || input == "if" {
        return parse_if(input);
    }
    if input.starts_with("for ") || input == "for" {
        return parse_for(input);
    }
    if input.starts_with("while ") || input == "while" {
        return parse_while(input);
    }

    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        bail!("empty input");
    }
    parse_sequence(&tokens)
}

// ── Block parsers ─────────────────────────────────────────────────────────────

/// Parse: if <condition>; then <body> [else <else_body>] fi
/// Or:    if <condition> { <body> } [else { <else_body> }]
fn parse_if(input: &str) -> Result<Command> {
    // Strip leading "if"
    let rest = input[2..].trim();

    // Find "then" or "{"
    let (cond_str, remainder) = if let Some(then_pos) = find_keyword(rest, "then") {
        (rest[..then_pos].trim(), rest[then_pos + 4..].trim())
    } else if rest.contains('{') {
        let brace = rest.find('{').unwrap();
        (rest[..brace].trim(), rest[brace..].trim())
    } else {
        bail!("if: expected 'then' or '{{'");
    };

    let condition = parse(cond_str)?;

    // Find body between then...fi or { ... }
    let (body_str, else_str) = extract_block(remainder)?;

    // Parse body lines
    let body = parse_block_lines(&body_str)?;

    // Parse optional else
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
fn parse_for(input: &str) -> Result<Command> {
    let rest = input[3..].trim();

    // Get variable name
    let var_end = rest.find(|c: char| c.is_whitespace())
        .ok_or_else(|| anyhow::anyhow!("for: expected variable name"))?;
    let var = rest[..var_end].to_string();
    let rest = rest[var_end..].trim();

    // Expect "in"
    if !rest.starts_with("in ") && rest != "in" {
        bail!("for: expected 'in' after variable");
    }
    let rest = rest[2..].trim();

    // Find "do" or "{"
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

    // Items are space-separated words, with glob expansion
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
fn parse_while(input: &str) -> Result<Command> {
    let rest = input[5..].trim();

    let (cond_str, remainder) = if let Some(do_pos) = find_keyword(rest, "do") {
        (rest[..do_pos].trim(), rest[do_pos + 2..].trim())
    } else if rest.contains('{') {
        let brace = rest.find('{').unwrap();
        (rest[..brace].trim(), rest[brace..].trim())
    } else {
        bail!("while: expected 'do' or '{{'");
    };

    let condition = parse(cond_str)?;
    let (body_str, _) = extract_block(remainder)?;
    let body = parse_block_lines(&body_str)?;

    Ok(Command::While {
        condition: Box::new(condition),
        body,
    })
}

/// Find a keyword at word boundary
fn find_keyword(s: &str, keyword: &str) -> Option<usize> {
    let mut pos = 0;
    while pos + keyword.len() <= s.len() {
        if s[pos..].starts_with(keyword) {
            let before_ok = pos == 0 || s.as_bytes()[pos - 1].is_ascii_whitespace();
            let after_ok = pos + keyword.len() == s.len()
                || s.as_bytes()[pos + keyword.len()].is_ascii_whitespace();
            if before_ok && after_ok {
                return Some(pos);
            }
        }
        pos += 1;
    }
    None
}

/// Extract content between { } or then...fi, returning (body, optional_else)
fn extract_block(s: &str) -> Result<(String, Option<String>)> {
    let s = s.trim();

    if s.starts_with('{') {
        // Brace style
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
        // then...fi style
        if let Some(fi_pos) = find_keyword(s, "fi") {
            let content = s[..fi_pos].trim();
            // Check for else
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

/// Extract content between do...done
fn extract_block_done(s: &str) -> Result<(String, &str)> {
    let s = s.trim();
    if let Some(done_pos) = find_keyword(s, "done") {
        Ok((s[..done_pos].trim().to_string(), &s[done_pos + 4..]))
    } else {
        Ok((s.to_string(), ""))
    }
}

/// Parse multiple lines/semicolons into a list of Commands
fn parse_block_lines(block: &str) -> Result<Vec<Command>> {
    let mut cmds = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        // Handle semicolon-separated commands on one line
        for part in line.split(';') {
            let part = part.trim();
            if part.is_empty() { continue; }
            cmds.push(parse(part)?);
        }
    }
    Ok(cmds)
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

// ── Tokenizer ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Pipe,
    And,
    Or,
    Semicolon,
    Ampersand,
    RedirectOut,
    RedirectAppend,
    RedirectIn,
    RedirectErr,
    RedirectErrOut,
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => { chars.next(); }

            '\'' => {
                chars.next();
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '\'' { break; }
                    word.push(ch);
                }
                tokens.push(Token::Word(word));
            }

            '"' => {
                chars.next();
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '"' { break; }
                    if ch == '\\' {
                        if let Some(&next) = chars.peek() {
                            chars.next();
                            word.push(next);
                        }
                    } else {
                        word.push(ch);
                    }
                }
                tokens.push(Token::Word(word));
            }

            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    tokens.push(Token::Pipe);
                }
            }

            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::And);
                } else {
                    tokens.push(Token::Ampersand);
                }
            }

            ';' => { chars.next(); tokens.push(Token::Semicolon); }

            '>' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RedirectAppend);
                } else {
                    tokens.push(Token::RedirectOut);
                }
            }

            '<' => { chars.next(); tokens.push(Token::RedirectIn); }

            '2' => {
                let s: String = chars.clone().take(4).collect();
                if s.starts_with("2>&1") {
                    for _ in 0..4 { chars.next(); }
                    tokens.push(Token::RedirectErrOut);
                } else if s.starts_with("2>") {
                    chars.next(); chars.next();
                    tokens.push(Token::RedirectErr);
                } else {
                    let word = read_word(&mut chars);
                    tokens.push(Token::Word(word));
                }
            }

            '#' => break, // comment

            '\\' => {
                chars.next();
                // backslash-newline = line continuation, skip
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }

            _ => {
                let word = read_word(&mut chars);
                let word = if word.starts_with('~') {
                    let home = dirs::home_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "~".to_string());
                    word.replacen('~', &home, 1)
                } else {
                    word
                };
                if word.contains('*') || word.contains('?') {
                    for w in expand_glob(&word) {
                        tokens.push(Token::Word(w));
                    }
                } else {
                    tokens.push(Token::Word(word));
                }
            }
        }
    }

    Ok(tokens)
}

fn read_word(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut word = String::new();
    while let Some(&c) = chars.peek() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '|' | '&' | ';' | '>' | '<' | '"' | '\'') {
            break;
        }
        chars.next();
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                chars.next();
                word.push(next);
            }
        } else {
            word.push(c);
        }
    }
    word
}

// ── Recursive descent parser ──────────────────────────────────────────────────

fn parse_sequence(tokens: &[Token]) -> Result<Command> {
    let (left, rest) = parse_and_or(tokens)?;
    if rest.first() == Some(&Token::Semicolon) {
        let rest = &rest[1..];
        if rest.is_empty() { return Ok(left); }
        let right = parse_sequence(rest)?;
        return Ok(Command::Sequence(Box::new(left), Box::new(right)));
    }
    Ok(left)
}

fn parse_and_or(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let (mut left, mut rest) = parse_pipeline(tokens)?;
    loop {
        match rest.first() {
            Some(Token::And) => {
                let (right, new_rest) = parse_pipeline(&rest[1..])?;
                left = Command::And(Box::new(left), Box::new(right));
                rest = new_rest;
            }
            Some(Token::Or) => {
                let (right, new_rest) = parse_pipeline(&rest[1..])?;
                left = Command::Or(Box::new(left), Box::new(right));
                rest = new_rest;
            }
            _ => break,
        }
    }
    Ok((left, rest))
}

fn parse_pipeline(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let (cmd, mut rest) = parse_simple(tokens)?;
    if rest.first() != Some(&Token::Pipe) {
        return Ok((cmd, rest));
    }
    let mut cmds = vec![cmd];
    while rest.first() == Some(&Token::Pipe) {
        let (next_cmd, new_rest) = parse_simple(&rest[1..])?;
        cmds.push(next_cmd);
        rest = new_rest;
    }
    Ok((Command::Pipeline(cmds), rest))
}

fn parse_simple(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let mut args = Vec::new();
    let mut redirects = Vec::new();
    let mut background = false;
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(w) => { args.push(w.clone()); i += 1; }
            Token::RedirectOut => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StdoutTo(file.clone()));
                    i += 2;
                } else { bail!("expected filename after >"); }
            }
            Token::RedirectAppend => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StdoutAppend(file.clone()));
                    i += 2;
                } else { bail!("expected filename after >>"); }
            }
            Token::RedirectIn => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StdinFrom(file.clone()));
                    i += 2;
                } else { bail!("expected filename after <"); }
            }
            Token::RedirectErr => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StderrTo(file.clone()));
                    i += 2;
                } else { bail!("expected filename after 2>"); }
            }
            Token::RedirectErrOut => { redirects.push(Redirect::StderrToStdout); i += 1; }
            Token::Ampersand => { background = true; i += 1; break; }
            _ => break,
        }
    }

    if args.is_empty() { bail!("expected command"); }

    Ok((Command::Simple { args, redirects, background }, &tokens[i..]))
}