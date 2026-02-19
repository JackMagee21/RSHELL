// src/executor/builtin/mod.rs
mod core;
mod find;
mod fs;
mod grep;
mod jobs;
mod test;
mod text;
mod util;

pub use util::command_not_found;

use crate::shell::Shell;

pub fn run_builtin(shell: &mut Shell, args: &[String]) -> Option<i32> {
    crossterm::terminal::disable_raw_mode().ok();

    match args[0].as_str() {
        // ── Core ──────────────────────────────────────────────
        "cd"              => Some(core::builtin_cd(shell, args)),
        "pwd"             => Some(core::builtin_pwd(shell)),
        "echo"            => Some(core::builtin_echo(args)),
        "export" | "set"  => Some(core::builtin_export(shell, args)),
        "unset"           => Some(core::builtin_unset(shell, args)),
        "alias"           => Some(core::builtin_alias(shell, args)),
        "unalias"         => Some(core::builtin_unalias(shell, args)),
        "history"         => Some(core::builtin_history(shell)),
        "source" | "."    => Some(core::builtin_source(shell, args)),
        "clear" | "cls"   => Some(core::builtin_clear()),
        "sleep"           => Some(core::builtin_sleep(args)),
        "functions"       => Some(core::builtin_functions(shell)),
        "help"            => Some(core::builtin_help()),
        "which"           => Some(core::builtin_which(args)),
        "pushd"           => Some(core::builtin_pushd(shell, args)),
        "popd"            => Some(core::builtin_popd(shell)),
        "dirs"            => Some(core::builtin_dirs(shell)),

        // ── Filesystem ────────────────────────────────────────
        "ls"              => Some(fs::builtin_ls(shell, args)),
        "mkdir"           => Some(fs::builtin_mkdir(args)),
        "rm"              => Some(fs::builtin_rm(args)),
        "cp"              => Some(fs::builtin_cp(args)),
        "mv"              => Some(fs::builtin_mv(args)),
        "cat"             => Some(fs::builtin_cat(args)),
        "touch"           => Some(fs::builtin_touch(args)),

        // ── Search ────────────────────────────────────────────
        "grep"            => Some(grep::builtin_grep(args)),
        "find"            => Some(find::builtin_find(args)),

        // ── Text processing ───────────────────────────────────
        "head"            => Some(text::builtin_head(args)),
        "tail"            => Some(text::builtin_tail(args)),
        "wc"              => Some(text::builtin_wc(args)),
        "env"             => Some(text::builtin_env(args)),

        // ── Job control ───────────────────────────────────────
        "jobs"            => Some(jobs::builtin_jobs(shell)),
        "fg"              => Some(jobs::builtin_fg(shell, args)),
        "bg"              => Some(jobs::builtin_bg(shell, args)),
        "kill"            => Some(jobs::builtin_kill(shell, args)),

        // ── Test / conditionals ───────────────────────────────
        "test" | "["      => Some(test::builtin_test(shell, args)),

        // ── Shell primitives ──────────────────────────────────
        "true"            => Some(0),
        "false"           => Some(1),
        "exit" | "quit"   => std::process::exit(shell.last_exit_code),

        _                 => None,
    }
}