// src/parser/mod.rs
//
// Entry point for the parser. Delegates to submodules:
//
//   ast.rs        — Command and Redirect enums
//   tokenizer.rs  — raw text → Token list
//   block.rs      — if / for / while parsers + block extraction helpers

pub mod ast;
mod block;
mod tokenizer;

use ast::{Command, Redirect};
use anyhow::{Result, bail};
use tokenizer::Token;

/// Parse a complete input string into a Command AST.
pub fn parse(input: &str) -> Result<Command> {
    let input = input.trim();
    if input.is_empty() {
        bail!("empty input");
    }

    // Block-level keywords are handled before tokenising
    if input.starts_with("if ") || input == "if" {
        return block::parse_if(input);
    }
    if input.starts_with("for ") || input == "for" {
        return block::parse_for(input);
    }
    if input.starts_with("while ") || input == "while" {
        return block::parse_while(input);
    }

    let tokens = tokenizer::tokenize(input)?;
    if tokens.is_empty() {
        bail!("empty input");
    }
    parse_sequence(&tokens)
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
    let mut args      = Vec::new();
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
            Token::Ampersand     => { background = true; i += 1; break; }
            _ => break,
        }
    }

    if args.is_empty() { bail!("expected command"); }

    Ok((Command::Simple { args, redirects, background }, &tokens[i..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        match parse("echo hello world").unwrap() {
            Command::Simple { args, redirects, background } => {
                assert_eq!(args, vec!["echo", "hello", "world"]);
                assert!(redirects.is_empty());
                assert!(!background);
            }
            other => panic!("expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn parses_pipeline() {
        match parse("cat file.txt | grep pattern").unwrap() {
            Command::Pipeline(cmds) => {
                assert_eq!(cmds.len(), 2);
                assert!(matches!(&cmds[0], Command::Simple { args, .. } if args == &vec!["cat".to_string(), "file.txt".to_string()]));
                assert!(matches!(&cmds[1], Command::Simple { args, .. } if args == &vec!["grep".to_string(), "pattern".to_string()]));
            }
            other => panic!("expected Pipeline, got {:?}", other),
        }
    }

    #[test]
    fn parses_and_or() {
        assert!(matches!(parse("true && echo yes").unwrap(), Command::And(_, _)));
        assert!(matches!(parse("false || echo no").unwrap(), Command::Or(_, _)));
    }

    #[test]
    fn parses_sequence() {
        assert!(matches!(parse("echo a; echo b").unwrap(), Command::Sequence(_, _)));
    }

    #[test]
    fn parses_background() {
        match parse("sleep 10 &").unwrap() {
            Command::Simple { background, .. } => assert!(background),
            other => panic!("expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn parses_redirects() {
        match parse("echo hi > out.txt").unwrap() {
            Command::Simple { redirects, .. } => assert_eq!(redirects, vec![Redirect::StdoutTo("out.txt".into())]),
            other => panic!("expected Simple, got {:?}", other),
        }
        match parse("echo hi >> out.txt").unwrap() {
            Command::Simple { redirects, .. } => assert_eq!(redirects, vec![Redirect::StdoutAppend("out.txt".into())]),
            other => panic!("expected Simple, got {:?}", other),
        }
        match parse("cat < in.txt").unwrap() {
            Command::Simple { redirects, .. } => assert_eq!(redirects, vec![Redirect::StdinFrom("in.txt".into())]),
            other => panic!("expected Simple, got {:?}", other),
        }
        match parse("cmd 2> err.txt").unwrap() {
            Command::Simple { redirects, .. } => assert_eq!(redirects, vec![Redirect::StderrTo("err.txt".into())]),
            other => panic!("expected Simple, got {:?}", other),
        }
        match parse("cmd 2>&1").unwrap() {
            Command::Simple { redirects, .. } => assert_eq!(redirects, vec![Redirect::StderrToStdout]),
            other => panic!("expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn parses_if_brace_style() {
        match parse("if true { echo yes }").unwrap() {
            Command::If { condition, body, else_body } => {
                assert!(matches!(*condition, Command::Simple { .. }));
                assert_eq!(body.len(), 1);
                assert!(else_body.is_none());
            }
            other => panic!("expected If, got {:?}", other),
        }
    }

    #[test]
    fn parses_if_else_keyword_style() {
        match parse("if true then echo yes else echo no fi").unwrap() {
            Command::If { body, else_body, .. } => {
                assert_eq!(body.len(), 1);
                let else_body = else_body.expect("expected an else body");
                assert_eq!(else_body.len(), 1);
            }
            other => panic!("expected If, got {:?}", other),
        }
    }

    #[test]
    fn parses_for_loop() {
        match parse("for x in a b c do echo $x done").unwrap() {
            Command::For { var, items, body } => {
                assert_eq!(var, "x");
                assert_eq!(items, vec!["a", "b", "c"]);
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected For, got {:?}", other),
        }
    }

    #[test]
    fn parses_while_loop() {
        match parse("while true do echo loop done").unwrap() {
            Command::While { condition, body } => {
                assert!(matches!(*condition, Command::Simple { .. }));
                assert_eq!(body.len(), 1);
            }
            other => panic!("expected While, got {:?}", other),
        }
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
    }
}