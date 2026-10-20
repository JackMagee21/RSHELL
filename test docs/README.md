# RSHELL Tests

This folder documents the test suite. The tests themselves live next to the code they cover, under `src/`.

- [failure-cases.md](failure-cases.md): what each failure-case test checks, and the two bugs they found.

## Running the tests

```sh
make test            # runs cargo test
cargo test           # same thing, directly
cargo test parser::  # only tests whose path starts with parser::
cargo test rm_       # only tests whose name contains rm_
```

There is no CI. Tests only run when you run them.

## Where the tests live

Unit tests use Rust's built-in test framework. Most modules keep their tests in a `tests.rs` file next to the source, pulled in with `#[cfg(test)] mod tests;`.

| Area | Source | Tests |
|------|--------|-------|
| Parser | `src/parser/mod.rs` | `src/parser/tests.rs` |
| Tokenizer | `src/parser/tokenizer.rs` | `src/parser/tokenizer/tests.rs` |
| Block parsing (`if`/`for`/`while`) | `src/parser/block.rs` | `src/parser/block/tests.rs` |
| Variable and arithmetic expansion | `src/executor/expand.rs` | `src/executor/expand/tests.rs` |
| `test` / `[` builtin | `src/executor/builtin/test.rs` | `src/executor/builtin/test/tests.rs` |
| Filesystem builtins | `src/executor/builtin/fs.rs` | `src/executor/builtin/fs/tests.rs` |
| Other builtins | `src/executor/builtin/*` | `*/tests.rs` next to each one |

## Conventions

- **Happy path and failure cases sit side by side.** Failure-case tests have a `// ── Failure cases` divider so they are easy to find.
- **Failure means one of two things.** Either the function returns `Err` (for parsers and expansion), or it returns a non-zero exit code (for builtins). Tests assert which one.
- **Filesystem tests use the system temp dir.** Each test gets its own scratch directory under `std::env::temp_dir()`, named after the test, and removes it at the end. Tests never write inside the repo.
- **Platform gates.** `parse_chmod_mode` only exists on Unix, so the `chmod` tests are gated with `#[cfg(unix)]`. Everything else runs on Windows, Linux, and macOS.

## Known gaps

These are behaviours that are not tested yet, or that look wrong but are not asserted:

- **Unterminated quotes are accepted.** `echo 'abc` tokenizes as `echo` and `abc` with no error. It is unclear whether this should fail, so no test asserts either way.
- **Trailing garbage in arithmetic is accepted.** `eval_arithmetic("1 2")` returns `1` and ignores the `2`. Not asserted.
- **Possible overflow panic.** `9223372036854775807 * 2` likely overflows `i64` in arithmetic, which panics in debug builds. Found by reading the code, not by running it. No test yet.
- **Untested modules.** `src/executor/builtin/pkg/`, `src/executor/mod.rs`, and `src/shell/` have many error branches with no failure-case tests.
