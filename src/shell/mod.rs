// src/shell/mod.rs
//
// Core shell state and lifecycle. Delegates to submodules:
//
//   prompt.rs   — build_prompt(), shorten_path(), get_git_branch()
//   history.rs  — load_history(), save_history_line(), expand_history()
//   persist.rs  — save_aliases(), save_functions()

mod history;
mod persist;
mod prompt;

use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

// ── Types ─────────────────────────────────────────────────────────────────────

pub struct Job {
    pub id: usize,
    pub pid: u32,
    pub command: String,
    pub status: JobStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Running,
    Done,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Running => write!(f, "Running"),
            JobStatus::Done    => write!(f, "Done"),
        }
    }
}

/// A user-defined shell function.
#[derive(Debug, Clone)]
pub struct ShellFunction {
    pub body: Vec<String>,
}

// ── Shell struct ──────────────────────────────────────────────────────────────

pub struct Shell {
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub prev_dir: Option<PathBuf>,
    pub history: Vec<String>,
    pub aliases: HashMap<String, String>,
    pub functions: HashMap<String, ShellFunction>,
    pub last_exit_code: i32,
    pub jobs: HashMap<usize, Job>,
    pub dir_stack: Vec<PathBuf>,
    pub exit_on_error: bool,
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
            functions: HashMap::new(),
            last_exit_code: 0,
            jobs: HashMap::new(),
            dir_stack: Vec::new(),
            exit_on_error: false,
        };

        // Default aliases
        shell.aliases.insert("ll".to_string(),  "ls -la".to_string());
        shell.aliases.insert("la".to_string(),  "ls -a".to_string());
        shell.aliases.insert("..".to_string(),  "cd ..".to_string());
        shell.aliases.insert("...".to_string(), "cd ../..".to_string());

        // Add ~/.rshell/bin to PATH so installed packages are available
        let rshell_bin = crate::executor::builtin::pkg::rshell_bin_dir();
        let rshell_bin_str = rshell_bin.to_string_lossy().to_string();
        let current_path = std::env::var("PATH").unwrap_or_default();
        if !current_path.contains(&rshell_bin_str) {
            #[cfg(windows)]
            let sep = ";";
            #[cfg(not(windows))]
            let sep = ":";
            let new_path = format!("{}{}{}", rshell_bin_str, sep, current_path);
            unsafe { std::env::set_var("PATH", &new_path); }
            shell.env.insert("PATH".to_string(), new_path);
        }
        let _ = std::fs::create_dir_all(&rshell_bin);

        shell
    }

    /// Check for finished background jobs and mark them Done.
    pub fn reap_jobs(&mut self) {
        let mut done = Vec::new();
        for (id, job) in &self.jobs {
            #[cfg(unix)]
            {
                let result = unsafe {
                    libc::waitpid(job.pid as i32, std::ptr::null_mut(), libc::WNOHANG)
                };
                if result > 0 { done.push(*id); }
            }
            #[cfg(windows)]
            {
                let alive = std::process::Command::new("tasklist")
                    .args(["/FI", &format!("PID eq {}", job.pid)])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).contains(&job.pid.to_string()))
                    .unwrap_or(false);
                if !alive { done.push(*id); }
            }
        }
        for id in done {
            if let Some(job) = self.jobs.get_mut(&id) {
                job.status = JobStatus::Done;
            }
        }
    }

    /// Load and execute ~/.myshellrc on startup.
    pub fn load_rc(&mut self) -> Result<()> {
        let rc_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshellrc");

        if rc_path.exists() {
            let content = std::fs::read_to_string(&rc_path)?;
            let mut func_buffer: Option<(String, Vec<String>)> = None;

            for line in content.lines() {
                let trimmed = line.trim();

                if let Some((ref name, ref mut body)) = func_buffer {
                    if trimmed == "}" {
                        let name = name.clone();
                        let body = body.clone();
                        self.functions.insert(name, ShellFunction { body });
                        func_buffer = None;
                    } else {
                        body.push(trimmed.to_string());
                    }
                    continue;
                }

                if trimmed.is_empty() || trimmed.starts_with('#') { continue; }

                if let Some(func_name) = parse_function_start(trimmed) {
                    func_buffer = Some((func_name, Vec::new()));
                    continue;
                }

                if let Err(e) = self.eval(trimmed) {
                    eprintln!("myshell: rc error: {e}");
                }
            }
        }
        Ok(())
    }

    /// Parse and execute a single input string.
    pub fn eval(&mut self, input: &str) -> Result<()> {
        let input = input.trim();
        if input.is_empty() || input.starts_with('#') {
            return Ok(());
        }

        if let Some(func_name) = parse_function_start(input) {
            return self.parse_inline_function(input, func_name);
        }

        let input = crate::executor::expand_arithmetic(self, input);
        let input = input.trim().to_string();

        let ast = crate::parser::parse(&input)?;
        crate::executor::execute(self, ast)
    }

    /// Parse a function defined on a single line: `name() { cmd; cmd }`.
    fn parse_inline_function(&mut self, input: &str, name: String) -> Result<()> {
        let open = match input.find('{') {
            Some(i) => i,
            None => {
                self.functions.insert(name, ShellFunction { body: vec![] });
                self.save_functions();
                return Ok(());
            }
        };
        let close = input.rfind('}').unwrap_or(input.len());
        let body_str = &input[open + 1..close];
        let body: Vec<String> = body_str
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        self.functions.insert(name, ShellFunction { body });
        self.save_functions();
        Ok(())
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Detect if a line starts a function definition, returning the function name.
pub fn parse_function_start(line: &str) -> Option<String> {
    let line = line.trim();

    if let Some(rest) = line.strip_prefix("function ") {
        let name = rest
            .split(|c: char| c == '(' || c == '{' || c.is_whitespace())
            .next()?
            .trim()
            .to_string();
        if !name.is_empty() { return Some(name); }
    }

    if let Some(paren) = line.find("()") {
        let name = line[..paren].trim().to_string();
        if !name.is_empty()
            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            && line[paren + 2..].trim().starts_with('{')
        {
            return Some(name);
        }
    }

    None
}