// src/shell.rs
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;
use crate::parser::parse;
use crate::executor::execute;

pub struct Job {
    pub pid: u32,
    pub command: String,
}

pub struct Shell {
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub prev_dir: Option<PathBuf>,
    pub history: Vec<String>,
    pub aliases: HashMap<String, String>,
    pub last_exit_code: i32,
    pub jobs: HashMap<usize, Job>,
    job_counter: usize,
}

impl Shell {
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let env: HashMap<String, String> = std::env::vars().collect();

        let mut shell = Shell {
            env,
            cwd,
            prev_dir: None,
            history: Vec::new(),
            aliases: HashMap::new(),
            last_exit_code: 0,
            jobs: HashMap::new(),
            job_counter: 0,
        };

        // Default aliases
        shell.aliases.insert("ll".to_string(), "ls -alF".to_string());
        shell.aliases.insert("la".to_string(), "ls -A".to_string());
        shell.aliases.insert("l".to_string(), "ls -CF".to_string());

        shell
    }

    pub fn load_rc(&mut self) -> Result<()> {
        let rc_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshellrc");

        if rc_path.exists() {
            let content = std::fs::read_to_string(&rc_path)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Err(e) = self.eval(line) {
                    eprintln!("myshell: rc error: {e}");
                }
            }
        }
        Ok(())
    }

    pub fn eval(&mut self, input: &str) -> Result<()> {
        let input = input.trim();
        if input.is_empty() || input.starts_with('#') {
            return Ok(());
        }
        let ast = parse(input)?;
        execute(self, ast)
    }

    pub fn build_prompt(&self) -> String {
        let home = dirs::home_dir()
            .map(|h| h.display().to_string())
            .unwrap_or_default();

        let cwd = self.cwd.display().to_string();
        let cwd = if cwd.starts_with(&home) {
            cwd.replacen(&home, "~", 1)
        } else {
            cwd
        };

        let code_indicator = if self.last_exit_code == 0 {
            "\x1b[32m❯\x1b[0m" // green
        } else {
            "\x1b[31m❯\x1b[0m" // red
        };

        // Show git branch if in a git repo
        let git_branch = get_git_branch()
            .map(|b| format!(" \x1b[35m({})\x1b[0m", b))
            .unwrap_or_default();

        format!("\x1b[34m{}\x1b[0m{} {} ", cwd, git_branch, code_indicator)
    }
}

fn get_git_branch() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
        if branch.is_empty() { None } else { Some(branch) }
    } else {
        None
    }
}