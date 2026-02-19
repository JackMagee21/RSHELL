// src/parser/ast.rs

#[derive(Debug, Clone)]
pub enum Command {
    Simple {
        args: Vec<String>,
        redirects: Vec<Redirect>,
        background: bool,
    },
    Pipeline(Vec<Command>),
    And(Box<Command>, Box<Command>),
    Or(Box<Command>, Box<Command>),
    Sequence(Box<Command>, Box<Command>),
    If {
        condition: Box<Command>,
        body: Vec<Command>,
        else_body: Option<Vec<Command>>,
    },
    For {
        var: String,
        items: Vec<String>,
        body: Vec<Command>,
    },
    While {
        condition: Box<Command>,
        body: Vec<Command>,
    },
    /// User-defined function call
    FunctionCall {
        name: String,
        args: Vec<String>,
    },
    /// Function definition
    FunctionDef {
        name: String,
        body: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub enum Redirect {
    StdoutTo(String),
    StdoutAppend(String),
    StdinFrom(String),
    StderrTo(String),
    StderrToStdout,
}