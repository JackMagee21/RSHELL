// src/executor/builtin/test.rs
use crate::shell::Shell;

pub fn builtin_test(shell: &Shell, args: &[String]) -> i32 {
    use crate::executor::{expand_vars, expand_arithmetic};
    let expanded: Vec<String> = args.iter()
        .map(|a| { let a = expand_arithmetic(shell, a); expand_vars(shell, &a) })
        .collect();
    let args: Vec<&str> = expanded.iter()
        .skip(1)
        .map(|s: &String| s.as_str())
        .filter(|&s| s != "]")
        .collect();
    if args.is_empty() { return 1; }
    if args[0] == "!" { return if eval_test(&args[1..]) == 0 { 1 } else { 0 }; }
    eval_test(&args)
}

fn eval_test(args: &[&str]) -> i32 {
    match args {
        ["-n", s]     => if s.is_empty() { 1 } else { 0 },
        ["-z", s]     => if s.is_empty() { 0 } else { 1 },
        [a, "=",  b]  => if a == b { 0 } else { 1 },
        [a, "==", b]  => if a == b { 0 } else { 1 },
        [a, "!=", b]  => if a != b { 0 } else { 1 },
        [a, "-eq", b] => compare_nums(a, b, |x, y| x == y),
        [a, "-ne", b] => compare_nums(a, b, |x, y| x != y),
        [a, "-lt", b] => compare_nums(a, b, |x, y| x <  y),
        [a, "-le", b] => compare_nums(a, b, |x, y| x <= y),
        [a, "-gt", b] => compare_nums(a, b, |x, y| x >  y),
        [a, "-ge", b] => compare_nums(a, b, |x, y| x >= y),
        ["-f", p]     => if std::path::Path::new(p).is_file()  { 0 } else { 1 },
        ["-d", p]     => if std::path::Path::new(p).is_dir()   { 0 } else { 1 },
        ["-e", p]     => if std::path::Path::new(p).exists()   { 0 } else { 1 },
        ["-s", p]     => if std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false) { 0 } else { 1 },
        [s]           => if s.is_empty() { 1 } else { 0 },
        _             => { eprintln!("test: unsupported expression: {:?}", args); 1 }
    }
}

fn compare_nums(a: &str, b: &str, f: impl Fn(i64, i64) -> bool) -> i32 {
    match (a.parse::<i64>(), b.parse::<i64>()) {
        (Ok(x), Ok(y)) => if f(x, y) { 0 } else { 1 },
        _ => { eprintln!("test: '{}' or '{}' is not a number", a, b); 1 }
    }
}