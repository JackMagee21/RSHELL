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
    Stopped,
    Done,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Running => write!(f, "Running"),
            JobStatus::Stopped => write!(f, "Stopped"),
            JobStatus::Done    => write!(f, "Done"),
        }
    }
}

/// A user-defined function
#[derive(Debug, Clone)]
pub struct ShellFunction {
    pub name: String,
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
    pub job_counter: usize,
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
            job_counter: 0,
        };

        shell.aliases.insert("ll".to_string(),  "ls -la".to_string());
        shell.aliases.insert("la".to_string(),  "ls -a".to_string());
        shell.aliases.insert("..".to_string(),  "cd ..".to_string());
        shell.aliases.insert("...".to_string(), "cd ../..".to_string());

        shell
    }

    /// Add a background job, returns its job id
    pub fn add_job(&mut self, pid: u32, command: String) -> usize {
        self.job_counter += 1;
        let id = self.job_counter;
        self.jobs.insert(id, Job {
            id,
            pid,
            command,
            status: JobStatus::Running,
        });
        id
    }

    /// Remove completed jobs from the jobs table
    pub fn reap_jobs(&mut self) {
        let mut done = Vec::new();
        for (id, job) in &self.jobs {
            // Check if process is still alive by sending signal 0
            #[cfg(unix)]
            {
                let alive = unsafe { libc::kill(job.pid as i32, 0) } == 0;
                if !alive { done.push(*id); }
            }
            #[cfg(windows)]
            {
                // On Windows check via OpenProcess
                use std::process::Command;
                let alive = Command::new("tasklist")
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
                        self.functions.insert(name.clone(), ShellFunction { name, body });
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

        let input = crate::executor::expand_arithmetic_str(self, input);
        let input = input.trim().to_string();

        let ast = crate::parser::parse(&input)?;
        crate::executor::execute(self, ast)
    }

    fn parse_inline_function(&mut self, input: &str, name: String) -> Result<()> {
        let open = match input.find('{') {
            Some(i) => i,
            None => {
                self.functions.insert(name.clone(), ShellFunction { name, body: vec![] });
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
        self.functions.insert(name.clone(), ShellFunction { name, body });
        Ok(())
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
            "\x1b[32m❯\x1b[0m"
        } else {
            "\x1b[31m❯\x1b[0m"
        };

        let git_branch = get_git_branch()
            .map(|b| format!(" \x1b[35m({})\x1b[0m", b))
            .unwrap_or_default();

        format!("\x1b[34m{}\x1b[0m{} {} ", cwd, git_branch, code_indicator)
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