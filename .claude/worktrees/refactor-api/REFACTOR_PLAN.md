# RSHELL Refactor Plan

Phase 1 survey — read-only, nothing in the tree has been changed. Goal: reduce code size where it genuinely improves readability, with zero behavior change. Clarity wins every tie.

## Baseline

- **Build**: `cargo build` — clean, zero warnings (no `#![allow(...)]` suppressing anything either).
- **Lint**: `cargo clippy --all-targets` — 33 warnings, all mechanical style suggestions (no correctness issues). Folded into the relevant sections below.
- **Tests**: `cargo test` — **5 tests total, all in `src/glob.rs`**, all passing. Nothing else in the ~5,900-line codebase has any test coverage at all. This matters a lot for risk ratings below: anything touching parser/executor/builtins is being changed with no automated safety net, so those items are rated higher-risk than their mechanical difficulty would otherwise suggest, and most recommend a manual smoke-test pass immediately after.

---

## Dead dependencies (remove from `Cargo.toml`)

Verified by grepping for both `use <crate>::` and fully-qualified `<crate>::` calls across all of `src/` — the first pass missed fully-qualified calls without a `use`, so this list was re-verified a second way before finalizing.

| Dependency | Status | Evidence |
|---|---|---|
| `thiserror` | **Unused** | Zero references, either form. |
| `colored` | **Unused** | Zero references, either form. |
| `toml` | **Unused** | Zero references, either form. |
| `glob` | Used | `glob::glob(...)` called directly in `parser/tokenizer.rs:167` and `parser/block.rs:207` — see "Two glob engines" below, this is not a simple removal. |

**Action**: delete the `thiserror`, `colored`, `toml` lines from `Cargo.toml`, run `cargo build` to confirm it still compiles, done. **~3 lines, risk: low** — a dependency with zero references anywhere cannot be load-bearing.

---

## Dead code (low-risk/high-value first)

1. **`Command::FunctionCall` / `Command::FunctionDef` AST variants are entirely unreachable.** `parser/ast.rs:33-41` defines them and `executor/mod.rs:96-107` matches them, but the parser never constructs either (grepped all of `src/`). Functions actually work through a completely separate path: `shell.functions: HashMap` populated by text-parsing in `shell/mod.rs` (lines 162, 208, 220) and invoked via a plain string check in `executor/mod.rs:217`. The existing `#[allow(dead_code)]` on the enum (`ast.rs:3-6`) correctly notices these are unused but suppresses instead of removing.
   **Remove**: `ast.rs:33-41`, `executor/mod.rs:96-107`, the now-unneeded `#[allow(dead_code)]`. **~25 lines. Risk: medium** (no test coverage; genuinely-dead code can't change reachable behavior by definition, but do a manual smoke test of defining/calling a function before and after).

No other dead code found anywhere else in the crate — every other `pub fn` is reachable through a dispatch table or direct call (`cargo build`'s zero `dead_code` warnings corroborate this).

---

## Purposeless files

2. **`src/gui_main.rs` (329 lines) — entirely dead, non-compiling code.** Imports `eframe::egui` and `portable_pty`, neither of which is in `Cargo.toml`. `Cargo.toml`'s only `[[bin]]` is `rSHELL` → `src/main.rs`; `gui_main.rs` is registered nowhere and nothing in `src/` references it. It cannot build as-is and isn't part of any build.
   **Action**: `git rm src/gui_main.rs` in its own commit. **−329 lines, risk: low** (unreachable by definition). If the GUI is still wanted, that's a separate "finish building it" task, not a refactor — flagging rather than deciding for you.

---

## Duplication (low-risk/high-value first)

3. **`core.rs:254-266` (`builtin_dirs`) duplicates `core.rs:268-279` (`print_dir_stack`) almost verbatim.** Have `builtin_dirs` call `print_dir_stack(shell); 0` instead of repeating the body. **~11 lines. Risk: low** — byte-for-byte identical logic either way.
4. **Home-tilde-collapse one-liner repeated 4×** in core.rs (2× in `builtin_dirs`, 2× in `print_dir_stack`) — folds away for free with #3, but also worth a named `fn collapse_home(path: &str, home: &str) -> String` since the idiom (`if x.starts_with(&home) { x.replacen(&home, "~", 1) } else { x }`) isn't self-explanatory at the call site. **~6 lines, risk: low.**
5. **Quote-stripping `.trim_matches('"').trim_matches('\'')` repeated 3×** (`core.rs` — `builtin_export:66`, `builtin_alias:93,94`). Extract `fn strip_quotes(s: &str) -> String`. **~3 lines, risk: low.**
6. **`text.rs`: `builtin_head`/`builtin_tail` duplicate their entire arg-parsing loop** (`-n N` / `-nN` / `-N` forms) and multi-file header-printing skeleton — differ only in the final line-selection step. Extract shared `parse_line_count_args(args, default) -> (usize, Vec<String>)`; keep per-file selection logic separate since it genuinely differs between the two. **~15-18 lines, risk: low.**
7. **`persist.rs`: `save_aliases`/`save_functions` share an identical write tail** (`persist.rs:34-37`, `:77-80`) — join lines, write, warn-on-error. Extract `fn write_rc(path: &Path, lines: &[String])`. Don't merge further — the filtering logic genuinely differs between the two. **~6 lines, risk: low.**
8. **Dotfile path resolution repeated 5×**: `dirs::home_dir().unwrap_or_default().join(".myshellrc"/".myshell_history")` independently in `shell/mod.rs:147-149`, `shell/history.rs:13-15,29-31`, `shell/persist.rs:11-13,42-44`. Extract `fn rc_path() -> PathBuf` and `fn history_path() -> PathBuf` once, reuse everywhere. **~15 lines, risk: low.**
9. **`MAX_HISTORY` defined twice with the same value** (`shell/history.rs:8` = 1000, `readline/mod.rs:112` hardcodes 1000 again for `FileBackedHistory::with_file`). Pull into one constant both files reference, so the two numbers can't silently drift apart. **Small, risk: low.**
10. **`parser/tokenizer.rs:166-177` and `parser/block.rs:206-217`: `expand_glob()` duplicated verbatim** (identical 12-line bodies, both calling the external `glob` crate). Extract to one shared location. **~12 lines, risk: low.**
11. **`parser/block.rs:190-204` (`find_keyword`) and `main.rs:234-253` (`count_keyword`)** implement the same word-boundary keyword scan — one returns first-match position, the other counts all matches. Could share a core predicate. **Risk: low-medium**, pure functions but no tests — mechanically simple to hand-verify.

**Considered and rejected** (not duplication, despite looking similar at a glance): `collect_files` (pkg/install.rs), `copy_dir_all` (fs.rs), `walk_find` (find.rs), `grep_dir` (grep.rs) all superficially look like "recursive directory walk," but each does meaningfully different per-entry work with different signatures and symlink/error handling. Forcing a shared visitor abstraction onto 4 different shapes would add code and indirection for no real gain.

---

## Over-engineering

None found. One thing explicitly checked and ruled out: `MyPrompt`'s four near-trivial `Prompt` trait method bodies (`readline/mod.rs:20-46`) look boilerplate-y but are mandated by reedline's own `Prompt` trait signature — a third-party interface contract, not a project abstraction to simplify.

---

## Reinvented wheels / architectural redundancy (flag for your decision — not auto-safe)

These are real, valuable findings, but either touch untested behavior-sensitive code or are judgment calls the ground rules say should come back to you rather than being bundled into an "obviously safe" pass.

12. **Two parallel glob engines exist side by side, and they don't even agree with each other.** `src/glob.rs` is RSHELL's own hand-rolled, tested engine (the project's only 5 tests), used by the executor for real argument/for-loop expansion at execute time. Separately, `parser/tokenizer.rs` and `parser/block.rs` each call the **external** `glob` crate at tokenize time to expand bare words and `for...in` lists early. By execute time the custom engine runs again over already-expanded text (a no-op by then in the common case). Worse: **the two disagree on dotfile handling** — the custom engine hides dotfiles unless the pattern itself starts with `.` (`glob.rs:94-96,133`); the external crate's default `MatchOptions` does not. This inconsistency pre-dates this survey.
    Consolidating onto the custom engine (dropping the external `glob` dependency entirely, plus both duplicate `expand_glob()` call sites from #10) would cut real code, remove a dependency, and fix a genuine bug — but it changes user-visible behavior in dotfile-glob edge cases. README.md explicitly advertises "its own glob engine" as a stated project feature, so the custom engine is the one to keep; the external crate's calls should move onto it. **High value, medium-high risk — needs your explicit sign-off before touching, not a default-yes item.**
13. **Two independent history systems write the exact same file.** `readline/mod.rs:111-116` wires up reedline's own `FileBackedHistory` against `~/.myshell_history` (drives interactive up/down-arrow, Ctrl+R). Separately, `shell/history.rs` hand-rolls a second, fully independent load/append/trim/`!!`/`!n` system against the *same file* (`Shell.history: Vec<String>`, used by the `history` builtin and `!!`/`!n` expansion). Both read and write the same file through completely separate code paths — real duplication of a whole subsystem, and a latent correctness risk (two in-memory views that can drift). Migrating `Shell`'s logic onto reedline's `History` trait (which does support indexed access via `iter_chronologic`) is a genuine behavior-preserving rewrite of ~90 lines, not a mechanical extraction, and this area has zero test coverage. **Flagging for your decision — risk: high, value: medium-high. Needs characterization tests written first if you want to pursue it.**
14. **`shell/mod.rs:103-109` manually branches `;` vs `:` for the `$PATH` separator** (`#[cfg(windows)]`/`#[cfg(not(windows))]`) when it could use `std::env::split_paths`/`join_paths` (stdlib, platform-correct already, not a new dependency). **Risk: medium** — correct-looking stdlib swap, but PATH-mutation logic has zero test coverage and affects whether installed packages are found at all; needs manual verification before/after, not just a glance.
15. **`executor/pipeline.rs:277-288` (`is_builtin_cmd`)'s static name list is a second, independently-maintained copy** of the real dispatch table in `executor/builtin/mod.rs`. Not a bug today, but a duplication-of-truth risk — a new builtin added to one list and forgotten in the other silently breaks pipeline detection for it. Fixing this properly means restructuring so one list is the source of truth, which is more than a line-trim. **Risk: medium**, flagging rather than including in the mechanical pass.
16. **`util.rs:73-86` hand-rolled Levenshtein distance** (used for "did you mean?" suggestions). A crate like `strsim` would delete ~14 lines but is a new dependency for a small, correct, self-contained function. **Recommendation: leave as-is** — not worth a new dependency per the ground rules. Noted per instructions, not proposed as a change.
17. **`pkg/mod.rs:195,246`: `read_dir(...).unwrap_or_else(|_| panic!(...))`, duplicated twice.** Changing panic → clean error return is a **behavior change** (crash vs. printed error + exit code), so flagging rather than folding into the mechanical pass — your call whether a behavior tweak like this belongs in a size-reduction pass at all.

---

## Verbose patterns

18. **Clippy-sourced mechanical fixes, 18 sites total, all risk: low, no behavior change, ~20-25 lines combined:**
    - "Stripping a prefix manually" → `.strip_prefix()`: `parser/block.rs:139-140`, `executor/expand.rs:197,201-202,216-217,220-221,225-226,239-240,242-243,251-252,254-255` (7 spots), `glob.rs:57-61`, `executor/builtin/jobs.rs:50-51`. (9 sites total across the crate.)
    - `executor/expand.rs:27` — `push_str()` with a 1-char literal → `push()`.
    - `executor/expand.rs:168` — single-arm `match` → `if let`.
    - `executor/pipeline.rs:176` — `CreatePipe` doesn't need `&mut`.
    - `executor/mod.rs:324` — redundant `.write(true)` alongside `.append(true)`.
    - `readline/mod.rs:57` — manual char-comparison chain → `matches!(c, ' ' | '|' | ';' | '&')`.
    - `executor/builtin/fs.rs:92` — manual `% == 0` → `.is_multiple_of()`.
    - `executor/builtin/fs.rs:212` — unneeded `return`.
    - `executor/builtin/pkg/install.rs:60,64` — `Iterator::last()` on a `DoubleEndedIterator` → `.next_back()`.
    - `executor/builtin/pkg/install.rs:70,125`, `pkg/meta.rs:22,29` — `&PathBuf` params → `&Path`.
    - `executor/builtin/pkg/mod.rs:206,289` — empty format-string literal.
19. **`history.rs:43-55`**: trimming history to `MAX_HISTORY` lines does reverse→take→collect→reverse→collect just to get "last N lines in original order." Simpler: `let total = content.lines().count(); content.lines().skip(total.saturating_sub(MAX_HISTORY))`. **~4 lines, clearer intent, risk: low** — same output for all inputs.
20. **`grep.rs`: `grep_file`/`grep_dir`/`builtin_grep` pass 7-8 positional booleans between each other** (clippy flags both at "8/7 args"). A small `GrepOptions { ignore_case, invert, line_nums, count_only }` struct would cut call sites to 2 args and remove the risk of silently transposing two bools. **Risk: medium** — grep.rs has zero test coverage, so a transposition bug wouldn't be caught automatically; do a manual smoke-test pass (`grep`, `grep -i`, `grep -v`, `grep -c`, `grep -n`) immediately after. `find.rs:walk_find` has the same 7-arg shape; same treatment would be consistent, lower priority since clippy doesn't flag it.

---

## File splits

21. **`mini.rs` (409 lines) → split out syntax highlighting.** Lines 34-197 (`highlight_line`, `highlight_rust/python/js/shell`, `colorize`, `find_comment_pos`) are ~164 lines of pure string-in/string-out functions with zero editor state — fully separable from the undo-stack/render/event-loop in the rest of the file. New file: `src/executor/builtin/mini/highlight.rs` ("how a line of code gets colorized"), with `mini.rs` becoming `mini/mod.rs` ("the editor's state and input loop"). **Risk: low** — pure move, no logic touched.
22. **`core.rs` (458 lines) → split out help text.** Lines 281-459 (`builtin_help_topic`, `help_topic`, ~178 lines, ~40% of the file) are static `println!` text blocks — a different concept from the cd/alias/export/pushd shell-state logic occupying the rest of the file. New file: `src/executor/builtin/help.rs` ("what `help <topic>` prints"). **Risk: low** — static text content, trivially diffable to confirm identical output.
23. **`main.rs` (271 lines) → split out multiline-continuation detection.** The trio `is_incomplete`/`count_keyword`/`brace_depth` (~80 lines) is fully self-contained pure logic for "does this input need another line before it's a complete command" — unrelated to the REPL loop, platform setup, and banner-printing that make up the rest of `main.rs`. Natural home: alongside `parser/block.rs`'s own keyword-scanning (`find_keyword`, #11 above — these two could even share logic), or its own small file. **Value: medium, risk: low** — pure functions, move verbatim.

**Considered and rejected**: `shell/mod.rs` (251 lines, already cleanly delegated per its own header comment), `readline/mod.rs` (193), `completion/mod.rs` (126), `fs.rs` (323, one cohesive concept with clear section markers), `pkg/mod.rs` (310, same), `executor/mod.rs` (358, three related concerns but reads cleanly top-to-bottom) — none recommended for splitting.

---

## Noise

24. Doc-comment typos: `pkg/install.rs:4` ("impliments", "Exectuable"), similar in `pkg/meta.rs`. Trivial — fix opportunistically alongside any other change touching those files, not worth a dedicated commit.
25. `gui_main.rs:35,93` — leftover scaffold comments. Moot once the file is removed per #2 above.

No other noise (restate-the-code comments, stale TODOs, leftover debug prints) found anywhere in the surveyed codebase.

---

## Suggested execution order

Grouped so each commit is independently revertable, low-risk/mechanical first:

1. Remove 3 dead dependencies (`thiserror`, `colored`, `toml`) — #Dead dependencies.
2. `git rm src/gui_main.rs` — #2.
3. Remove dead `FunctionCall`/`FunctionDef` AST variants — #1.
4. Mechanical duplication extractions — #3-#11 (one commit each, or small logical groups).
5. Clippy-sourced verbose-pattern fixes — #18-19 (can likely be one or two commits, all mechanical).
6. File splits — #21-23 (one commit each: move-only, then a separate cleanup commit only if needed).
7. **Stop and get your sign-off** before: the `GrepOptions`/`FindOptions` struct refactor (#20), the two-glob-engines consolidation (#12), the two-history-systems consolidation (#13), the `$PATH`-separator stdlib swap (#14), the `is_builtin_cmd` source-of-truth fix (#15), and the pkg panic→error-return change (#17) — these either change behavior, touch untested behavior-sensitive code, or are genuine architectural judgment calls.

## Test-coverage gap

Worth calling out on its own: **the entire parser, executor, and builtins layer has zero test coverage** (5 tests total, all in `glob.rs`). This refactor can proceed on the mechanical items because their outputs are trivially diffable by eye, but items #12, #13, #20 in particular would benefit from a handful of characterization tests (e.g., a few `grep`/`find` invocations with known output, a glob-expansion case with dotfiles, a function-definition-and-call case) written *before* touching that code, so "no behavior change" is actually verifiable rather than eyeballed. Flagging this as a prerequisite for the higher-risk items rather than a blocker for the whole plan.
