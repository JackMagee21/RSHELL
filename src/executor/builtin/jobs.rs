// src/executor/builtin/jobs.rs
use crate::shell::{Shell, JobStatus};

pub fn builtin_jobs(shell: &mut Shell) -> i32 {
    shell.reap_jobs();
    if shell.jobs.is_empty() { println!("No jobs"); return 0; }
    let mut job_list: Vec<_> = shell.jobs.values().collect();
    job_list.sort_by_key(|j| j.id);
    for job in job_list {
        let marker = if job.status == JobStatus::Running { "+" } else { "-" };
        println!("[{}] {} {:10} {}", job.id, marker, job.status.to_string(), job.command);
    }
    0
}

pub fn builtin_fg(shell: &mut Shell, args: &[String]) -> i32 {
    let job_id = get_job_id(shell, args);
    let (pid, command) = match job_id.and_then(|id| shell.jobs.get(&id)) {
        Some(job) => (job.pid, job.command.clone()),
        None => { eprintln!("fg: no such job"); return 1; }
    };
    println!("{}", command);
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, libc::SIGCONT); }
        let mut status = 0i32;
        unsafe { libc::waitpid(pid as i32, &mut status, 0); }
        if let Some(id) = job_id { shell.jobs.remove(&id); }
        if libc::WIFEXITED(status) { libc::WEXITSTATUS(status) } else { 1 }
    }
    #[cfg(windows)]
    { eprintln!("fg: job control not fully supported on Windows"); 1 }
}

pub fn builtin_bg(shell: &mut Shell, args: &[String]) -> i32 {
    let job_id = get_job_id(shell, args);
    let (pid, command) = match job_id.and_then(|id| shell.jobs.get_mut(&id)) {
        Some(job) => { job.status = JobStatus::Running; (job.pid, job.command.clone()) }
        None => { eprintln!("bg: no such job"); return 1; }
    };
    #[cfg(unix)]
    unsafe { libc::kill(pid as i32, libc::SIGCONT); }
    println!("[{}] {}", job_id.unwrap_or(0), command);
    0
}

pub fn builtin_kill(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("usage: kill [%jobid | pid]"); return 1; }
    let target = &args[1];
    if target.starts_with('%') {
        let id: usize = match target[1..].parse() {
            Ok(n) => n,
            Err(_) => { eprintln!("kill: invalid job id"); return 1; }
        };
        if let Some(job) = shell.jobs.get(&id) {
            #[cfg(unix)] unsafe { libc::kill(job.pid as i32, libc::SIGTERM); }
            #[cfg(windows)] eprintln!("kill: not fully supported on Windows");
            shell.jobs.remove(&id);
        } else { eprintln!("kill: no such job: {}", id); return 1; }
    } else {
        let pid: i32 = match target.parse() {
            Ok(n) => n,
            Err(_) => { eprintln!("kill: invalid pid"); return 1; }
        };
        #[cfg(unix)] unsafe { libc::kill(pid, libc::SIGTERM); }
        #[cfg(windows)] { std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output().ok(); }
    }
    0
}

pub fn get_job_id(shell: &Shell, args: &[String]) -> Option<usize> {
    if let Some(arg) = args.get(1) {
        arg.trim_start_matches('%').parse().ok()
    } else {
        shell.jobs.keys().max().copied()
    }
}