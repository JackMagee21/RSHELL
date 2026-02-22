// src/parser/ast.rs

// When compling, it doesnt construct these variants,
// so it thinks they are unused. But they are needed for the parser to construct the AST,
// so we need to keep them around, hense the #[allow(dead_code)].
#[allow(dead_code)]
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