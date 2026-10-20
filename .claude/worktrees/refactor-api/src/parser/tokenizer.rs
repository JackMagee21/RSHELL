// src/parser/tokenizer.rs
//
// Converts a raw input string into a flat list of tokens.
// Handles quoting, escapes, redirects, operators, tilde expansion, and globs.

use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
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

pub fn tokenize(input: &str) -> Result<Vec<Token>> {
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

            '#' => break, // rest of line is a comment

            '\\' => {
                chars.next();
                // backslash-newline = line continuation, skip both
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }

            _ => {
                let word = read_word(&mut chars);

                // Expand leading tilde
                let word = if word.starts_with('~') {
                    let home = dirs::home_dir()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "~".to_string());
                    word.replacen('~', &home, 1)
                } else {
                    word
                };

                // Expand globs inline
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

/// Read a plain (unquoted) word, stopping at shell metacharacters.
pub fn read_word(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
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