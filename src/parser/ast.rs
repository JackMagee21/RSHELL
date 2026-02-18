// src/parser/ast.rs

#[derive(Debug, Clone)]
pub enum Command {
    /// ls -la /home
    Simple {
        args: Vec<String>,
        redirects: Vec<Redirect>,
        background: bool,
    },
    /// cmd1 | cmd2 | cmd3
    Pipeline(Vec<Command>),
    /// cmd1 && cmd2
    And(Box<Command>, Box<Command>),
    /// cmd1 || cmd2
    Or(Box<Command>, Box<Command>),
    /// cmd1 ; cmd2
    Sequence(Box<Command>, Box<Command>),

    /// if <condition> { <body> } [else { <else_body> }]
    If {
        condition: Box<Command>,
        body: Vec<Command>,
        else_body: Option<Vec<Command>>,
    },

    /// for <var> in <items> { <body> }
    For {
        var: String,
        items: Vec<String>,
        body: Vec<Command>,
    },

    /// while <condition> { <body> }
    While {
        condition: Box<Command>,
        body: Vec<Command>,
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