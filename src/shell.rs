// src/shell.rs
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

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

/// A user-defined function
#[derive(Debug, Clone)]
pub struct ShellFunction {
    pub body: Vec<String>,
}

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

        shell.aliases.insert("ll".to_string(),  "ls -la".to_string());
        shell.aliases.insert("la".to_string(),  "ls -a".to_string());
        shell.aliases.insert("..".to_string(),  "cd ..".to_string());
        shell.aliases.insert("...".to_string(), "cd ../..".to_string());

        shell
    }

    pub fn reap_jobs(&mut self) {
        let mut done = Vec::new();
        for (id, job) in &self.jobs {
            #[cfg(unix)]
            {
                let alive = unsafe { libc::kill(job.pid as i32, 0) } == 0;
                if !alive { done.push(*id); }
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
                        self.functions.insert(name.clone(), ShellFunction { body });
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

    fn parse_inline_function(&mut self, input: &str, name: String) -> Result<()> {
    let open = match input.find('{') {
        Some(i) => i,
        None => {
            self.functions.insert(name.clone(), ShellFunction { body: vec![] });
            self.save_functions(); // ← add this
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
    self.functions.insert(name.clone(), ShellFunction { body });
    self.save_functions(); // ← add this
    Ok(())
    }

    pub fn expand_history(&self, input: &str) -> String {
        let input = input.trim();

        if input == "!!" || input.starts_with("!! ") {
            if let Some(last) = self.history.iter().rev()
                .find(|h| h.as_str() != "!!" && !h.starts_with("!!"))
            {
                let suffix = input.strip_prefix("!!").unwrap_or("").trim();
                let expanded = if suffix.is_empty() {
                    last.clone()
                } else {
                    format!("{} {}", last, suffix)
                };
                eprintln!("{}", expanded);
                return expanded;
            }
            eprintln!("myshell: !!: event not found");
            return input.to_string();
        }

        if input.starts_with('!') && input.len() > 1 {
            let rest = &input[1..];
            let (num_str, suffix) = rest.split_once(' ').unwrap_or((rest, ""));
            if let Ok(n) = num_str.parse::<usize>() {
                if n >= 1 && n <= self.history.len() {
                    let cmd = &self.history[n - 1];
                    let expanded = if suffix.is_empty() {
                        cmd.clone()
                    } else {
                        format!("{} {}", cmd, suffix)
                    };
                    eprintln!("{}", expanded);
                    return expanded;
                }
                eprintln!("myshell: !{}: event not found", n);
                return input.to_string();
            }
        }

        input.to_string()
    }

    pub fn build_prompt(&self) -> String {
        let home = dirs::home_dir()
            .map(|h| h.display().to_string())
            .unwrap_or_default();

        let cwd = self.cwd.display().to_string();
        let cwd = cwd.trim_start_matches("\\\\?\\").to_string();

        let cwd = if cwd.starts_with(&home) {
            cwd.replacen(&home, "~", 1)
        } else {
            cwd
        };

        let short = shorten_path(&cwd);

        let code_indicator = if self.last_exit_code == 0 {
            "\x1b[32m❯\x1b[0m"
        } else {
            "\x1b[31m❯\x1b[0m"
        };

        let git_branch = get_git_branch()
            .map(|b| format!(" \x1b[35m({})\x1b[0m", b))
            .unwrap_or_default();

        format!("\x1b[34m{}\x1b[0m{} {} ", short, git_branch, code_indicator)
    }

    /// Save aliases to ~/.myshellrc (replaces existing alias lines)
    pub fn save_aliases(&self) {
        let rc_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshellrc");

        let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();

        // Keep all lines that aren't alias lines
        let mut lines: Vec<String> = existing
            .lines()
            .filter(|l| !l.trim_start().starts_with("alias "))
            .map(|l| l.to_string())
            .collect();

        // Append current aliases sorted
        if !self.aliases.is_empty() {
            lines.push(String::new());
            lines.push("# aliases".to_string());
            let mut sorted: Vec<(&String, &String)> = self.aliases.iter().collect();
            sorted.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in sorted {
                lines.push(format!("alias {}='{}'", k, v));
            }
        }

        let content = lines.join("\n") + "\n";
        if let Err(e) = std::fs::write(&rc_path, content) {
            eprintln!("myshell: warning: could not save aliases: {}", e);
        }
    }

    /// Save functions to ~/.myshellrc (replaces existing function definitions)
    pub fn save_functions(&self) {
        let rc_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".myshellrc");

        let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();

        // Strip old function blocks
        let mut lines: Vec<String> = Vec::new();
        let mut in_func = false;
        for line in existing.lines() {
            if !in_func && (line.starts_with("function ") || line.contains("() {")) {
                in_func = true;
                continue;
            }
            if in_func {
                if line.trim() == "}" { in_func = false; }
                continue;
            }
            lines.push(line.to_string());
        }

        // Append current functions sorted
        if !self.functions.is_empty() {
            lines.push(String::new());
            let mut sorted: Vec<(&String, &ShellFunction)> = self.functions.iter().collect();
            sorted.sort_by_key(|(k, _)| k.as_str());
            for (name, func) in sorted {
                lines.push(format!("function {}() {{", name));
                for line in &func.body {
                    lines.push(format!("    {}", line));
                }
                lines.push("}".to_string());
                lines.push(String::new());
            }
        }

        let content = lines.join("\n") + "\n";
        if let Err(e) = std::fs::write(&rc_path, content) {
            eprintln!("myshell: warning: could not save functions: {}", e);
        }
    }
}

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

fn shorten_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 2 {
        return path.trim_start_matches('/').to_string();
    }
    parts[parts.len() - 2..].join("/")
}