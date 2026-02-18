// src/parser/mod.rs
pub mod ast;

use ast::{Command, Redirect};
use anyhow::{Result, bail};

/// Tokenize and parse a shell input string into a Command AST
pub fn parse(input: &str) -> Result<Command> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        bail!("empty input");
    }
    parse_sequence(&tokens)
}

/// A shell token
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Pipe,           // |
    And,            // &&
    Or,             // ||
    Semicolon,      // ;
    Ampersand,      // &
    RedirectOut,    // >
    RedirectAppend, // >>
    RedirectIn,     // <
    RedirectErr,    // 2>
    RedirectErrOut, // 2>&1
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' => { chars.next(); }

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

            ';' => {
                chars.next();
                tokens.push(Token::Semicolon);
            }

            '>' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RedirectAppend);
                } else {
                    tokens.push(Token::RedirectOut);
                }
            }

            '<' => {
                chars.next();
                tokens.push(Token::RedirectIn);
            }

            '2' => {
                // Peek ahead for 2> or 2>&1
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

            '#' => {
                // Comment - skip rest of line
                break;
            }

            _ => {
                let word = read_word(&mut chars);
                // Expand ~ to home dir
                let word = if word.starts_with('~') {
                    let home = dirs::home_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "~".to_string());
                    word.replacen('~', &home, 1)
                } else {
                    word
                };
                // Glob expansion
                if word.contains('*') || word.contains('?') {
                    let expanded = expand_glob(&word);
                    for w in expanded {
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
        if matches!(c, ' ' | '\t' | '|' | '&' | ';' | '>' | '<' | '"' | '\'') {
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

fn expand_glob(pattern: &str) -> Vec<String> {
    match glob::glob(pattern) {
        Ok(paths) => {
            let results: Vec<String> = paths
                .filter_map(|p| p.ok())
                .map(|p| p.display().to_string())
                .collect();
            if results.is_empty() {
                vec![pattern.to_string()]
            } else {
                results
            }
        }
        Err(_) => vec![pattern.to_string()],
    }
}

// --- Recursive descent parser ---

fn parse_sequence(tokens: &[Token]) -> Result<Command> {
    let (left, rest) = parse_and_or(tokens)?;
    if rest.first() == Some(&Token::Semicolon) {
        let rest = &rest[1..];
        if rest.is_empty() {
            return Ok(left);
        }
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
            Token::Word(w) => {
                args.push(w.clone());
                i += 1;
            }
            Token::RedirectOut => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StdoutTo(file.clone()));
                    i += 2;
                } else {
                    bail!("expected filename after >");
                }
            }
            Token::RedirectAppend => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StdoutAppend(file.clone()));
                    i += 2;
                } else {
                    bail!("expected filename after >>");
                }
            }
            Token::RedirectIn => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StdinFrom(file.clone()));
                    i += 2;
                } else {
                    bail!("expected filename after <");
                }
            }
            Token::RedirectErr => {
                if let Some(Token::Word(file)) = tokens.get(i + 1) {
                    redirects.push(Redirect::StderrTo(file.clone()));
                    i += 2;
                } else {
                    bail!("expected filename after 2>");
                }
            }
            Token::RedirectErrOut => {
                redirects.push(Redirect::StderrToStdout);
                i += 1;
            }
            Token::Ampersand => {
                background = true;
                i += 1;
                break;
            }
            _ => break,
        }
    }

    if args.is_empty() {
        bail!("expected command");
    }

    Ok((Command::Simple { args, redirects, background }, &tokens[i..]))
}