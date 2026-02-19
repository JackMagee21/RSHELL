// src/executor/builtin/core.rs
use std::path::PathBuf;
use crate::shell::Shell;

pub fn builtin_cd(shell: &mut Shell, args: &[String]) -> i32 {
    let target: PathBuf = match args.get(1).map(|s| s.as_str()) {
        None | Some("~") => match dirs::home_dir() {
            Some(h) => h,
            None => { eprintln!("cd: cannot find home directory"); return 1; }
        },
        Some("-") => match &shell.prev_dir {
            Some(p) => p.clone(),
            None => { eprintln!("cd: no previous directory"); return 1; }
        },
        Some(path) => {
            if path.starts_with("~/") || path.starts_with("~\\") {
                dirs::home_dir().unwrap_or_default().join(&path[2..])
            } else {
                shell.cwd.join(path)
            }
        }
    };

    let target = match target.canonicalize() {
        Ok(p) => p,
        Err(e) => { eprintln!("cd: {}: {}", args.get(1).unwrap_or(&String::new()), e); return 1; }
    };

    match std::env::set_current_dir(&target) {
        Ok(_) => { shell.prev_dir = Some(shell.cwd.clone()); shell.cwd = target; 0 }
        Err(e) => { eprintln!("cd: {e}"); 1 }
    }
}

pub fn builtin_pwd(shell: &Shell) -> i32 {
    println!("{}", shell.cwd.display());
    0
}

pub fn builtin_echo(args: &[String]) -> i32 {
    let mut no_newline = false;
    let mut start = 1;
    if args.get(1).map(|s| s.as_str()) == Some("-n") { no_newline = true; start = 2; }
    let output = args[start..].join(" ").replace("\\n", "\n").replace("\\t", "\t");
    if no_newline { print!("{}", output); } else { println!("{}", output); }
    0
}

pub fn builtin_export(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() == 1 {
        for (k, v) in &shell.env { println!("{}={}", k, v); }
        return 0;
    }
    for arg in &args[1..] {
        if let Some((k, v)) = arg.split_once('=') {
            let v = v.trim_matches('"').trim_matches('\'').to_string();
            shell.env.insert(k.to_string(), v.clone());
            unsafe { std::env::set_var(k, &v); }
        }
    }
    0
}

pub fn builtin_unset(shell: &mut Shell, args: &[String]) -> i32 {
    for arg in &args[1..] {
        shell.env.remove(arg);
        unsafe { std::env::remove_var(arg); }
    }
    0
}

pub fn builtin_alias(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() == 1 {
        for (k, v) in &shell.aliases { println!("alias {}='{}'", k, v); }
        return 0;
    }
    for arg in &args[1..] {
        if let Some((k, v)) = arg.split_once('=') {
            shell.aliases.insert(k.to_string(), v.trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = shell.aliases.get(arg.as_str()) {
            println!("alias {}='{}'", arg, v);
        } else {
            eprintln!("alias: {}: not found", arg);
        }
    }
    0
}

pub fn builtin_unalias(shell: &mut Shell, args: &[String]) -> i32 {
    for arg in &args[1..] { shell.aliases.remove(arg.as_str()); }
    0
}

pub fn builtin_history(shell: &Shell) -> i32 {
    for (i, line) in shell.history.iter().enumerate() {
        println!("{:4}  {}", i + 1, line);
    }
    0
}

pub fn builtin_source(shell: &mut Shell, args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("source: filename required"); return 1; }
    let path = shell.cwd.join(&args[1]);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Err(e) = shell.eval(line) { eprintln!("source: {e}"); }
            }
            0
        }
        Err(e) => { eprintln!("source: {}: {e}", args[1]); 1 }
    }
}

pub fn builtin_clear() -> i32 {
    print!("\x1B[2J\x1B[H");
    use std::io::Write;
    std::io::stdout().flush().ok();
    0
}

pub fn builtin_sleep(args: &[String]) -> i32 {
    if args.len() < 2 { eprintln!("usage: sleep <seconds>"); return 1; }
    match args[1].parse::<f64>() {
        Ok(secs) => { std::thread::sleep(std::time::Duration::from_secs_f64(secs)); 0 }
        Err(_) => { eprintln!("sleep: invalid time: {}", args[1]); 1 }
    }
}

pub fn builtin_functions(shell: &Shell) -> i32 {
    if shell.functions.is_empty() { println!("No functions defined."); return 0; }
    for (name, func) in &shell.functions {
        println!("function {}() {{", name);
        for line in &func.body { println!("  {}", line); }
        println!("}}");
    }
    0
}

pub fn builtin_help() -> i32 {
    println!(r#"
╔══════════════════════════════════════════════╗
║         myshell  —  Built-in Commands        ║
╚══════════════════════════════════════════════╝

  cd [dir]           Change directory (- for previous, ~ for home)
  pwd                Print working directory
  ls [-la] [dir]     List directory contents
  echo [-n] [args]   Print text
  export [VAR=VAL]   Set or show environment variables
  unset VAR          Remove environment variable
  alias [k=v]        Set or show aliases
  unalias NAME       Remove alias
  history            Show command history
  source FILE        Execute commands from a file
  clear / cls        Clear the screen
  help               Show this help
  exit               Exit myshell

  Job Control:
    jobs             List background jobs
    fg [%id]         Bring job to foreground
    bg [%id]         Resume stopped job in background
    kill [%id|pid]   Kill a job or process
    cmd &            Run command in background
    Ctrl+Z           Suspend current command (Linux)

  Scripting:
    if test -f file {{ cmd }} else {{ cmd }}
    for x in a b c; do cmd; done
    while test $X -lt 10; do cmd; done
    function foo() {{ cmd; }}
    echo $((2 + 2 * 3))

  Operators:
    |  pipe   &&  and   ||  or   ;  sequence   &  background
    >  stdout  >>  append  <  stdin  2>  stderr
"#);
    0
}