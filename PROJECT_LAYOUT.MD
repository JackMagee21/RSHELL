myshell/
├── Cargo.toml
├── .myshellrc.example      ← Copy to ~/.myshellrc
├── src/
│   ├── main.rs             ← CLI shell entry point
│   ├── gui_main.rs         ← GUI terminal emulator entry point
│   ├── shell.rs            ← Shell state (env, cwd, aliases, history)
│   ├── parser/
│   │   ├── mod.rs          ← Tokenizer + recursive descent parser
│   │   └── ast.rs          ← Command AST types
│   ├── executor/
│   │   ├── mod.rs          ← Fork/exec, pipes, redirects
│   │   └── builtin.rs      ← cd, echo, alias, export, history...
│   └── readline/
│       └── mod.rs          ← reedline line editor integration