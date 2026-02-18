// src/parser/ast.rs
// The Abstract Syntax Tree for our shell language

#[derive(Debug, Clone)]
pub enum Command {
    /// A simple command: ls -la /home
    Simple {
        args: Vec<String>,
        redirects: Vec<Redirect>,
        background: bool,
    },
    /// A pipeline: cmd1 | cmd2 | cmd3
    Pipeline(Vec<Command>),
    /// Logical AND: cmd1 && cmd2
    And(Box<Command>, Box<Command>),
    /// Logical OR: cmd1 || cmd2
    Or(Box<Command>, Box<Command>),
    /// Semicolon: cmd1 ; cmd2
    Sequence(Box<Command>, Box<Command>),
}

#[derive(Debug, Clone)]
pub enum Redirect {
    /// > file
    StdoutTo(String),
    /// >> file
    StdoutAppend(String),
    /// < file
    StdinFrom(String),
    /// 2> file
    StderrTo(String),
    /// 2>&1
    StderrToStdout,
}