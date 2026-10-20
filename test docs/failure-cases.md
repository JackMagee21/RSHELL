# Failure-case tests

What each failure-case test checks and why it matters. Each section names the file, then lists the tests in it.

## Parser: `src/parser/tests.rs`

These tests check that malformed input returns `Err` instead of producing a wrong command tree.

- **`pipe_missing_command_on_either_side_is_an_error`**: `| b`, `a |`, `a | | b`. A pipe needs a command on both sides.
- **`and_or_missing_right_side_is_an_error`**: `a &&`, `a ||`. The right side of `&&` and `||` must exist.
- **`lone_operators_are_errors`**: `;`, `&`, `&&` on their own. Each one has nothing to act on.
- **`redirect_followed_by_operator_is_an_error`**: `cmd > | b`, `cmd >> && b`, `cmd < ;`, `cmd 2> |`. A redirect must be followed by a filename, not another operator.
- **`for_loop_missing_parts_is_an_error`**: `for`, `for i`, `for i a b; do echo; done`, `for i in a b c`, `for i in`. Covers a missing variable, a missing `in`, a missing `do`/`{`, and missing items.
- **`while_missing_do_or_brace_is_an_error`**: `while`, `while true`. Needs a `do` or `{`.
- **`if_missing_then_or_brace_is_an_error`**: `if`, `if true`. Needs a `then` or `{`.
- **`unclosed_brace_block_is_an_error_not_a_panic`**: `if true { echo hi`, `while true { echo hi`, `for i in a { echo $i`. Each needs a closing `}`.
  - **Bug found here:** this used to panic. See [Bug 1](#bug-1-unclosed--panicked).
- **`brace_block_with_multibyte_chars_keeps_else`**: `if true { echo é } else { echo b }`. The `else` must still be found after a non-ASCII character.
  - **Bug found here:** this used to panic, and before that it silently dropped the `else`. See [Bug 2](#bug-2-non-ascii-text-in-a-block-panicked).

## Tokenizer: `src/parser/tokenizer/tests.rs`

The tokenizer never returns `Err`, so these tests check edge cases that must not crash.

- **`dangling_redirect_tokenizes_without_a_filename`**: `cmd 2>` tokenizes to `Word("cmd")` and `RedirectErr`. The tokenizer accepts it. The parser is what rejects a redirect with no target, as tested above.
- **`trailing_backslash_does_not_panic`**: `echo \`. A lone backslash at the end of input must not index past the end.

## `test` / `[` builtin: `src/executor/builtin/test/tests.rs`

Every failure here returns exit code `1`, which is what `test` returns for false.

- **`unknown_operator_returns_failure`**: `test a -x b`. An operator the builtin doesn't know must not be treated as true.
- **`missing_operand_returns_failure`**: `test 1 -eq`, `test a =`. A binary operator with no right-hand side.
- **`too_many_operands_returns_failure`**: `test a = b c`. Extra operands are not silently ignored.
- **`non_numeric_operand_fails_closed`**: `test 3 -eq x`, `test 3 -lt ""`. Numeric comparison with a non-number is false, not an error.
- **`file_checks_on_missing_paths_fail`**: `-f`, `-d`, `-s` on a path that does not exist.
- **`size_check_fails_on_empty_file`**: `-s` on an existing empty file is false, while `-e` on the same file is true. This is the one case that tells `-s` apart from `-e`.

## Expansion: `src/executor/expand/tests.rs`

- **`arithmetic_unclosed_paren_is_error`**: `(1 + 2`. The closing `)` is missing.
- **`arithmetic_operator_without_operand_is_error`**: empty input, `1 +`, `* 3`. An operator needs operands on the correct side.
- **`expand_arithmetic_failure_becomes_zero`**: `x=$((1/0))` and `x=$((abc))` both expand to `x=0`. A failed `$((...))` is replaced with `0` rather than left in the output. The error is printed to stderr.

## Filesystem builtins: `src/executor/builtin/fs/tests.rs`

Each test uses a scratch directory under the system temp dir. The directory is removed at the end.

- **`missing_arguments_print_usage_and_fail`**: `mkdir`, `rm`, `cp a`, `mv a`, `cat`, `touch` with too few arguments. Each returns `1`.
- **`mkdir_existing_dir_fails`**: `mkdir` on a directory that already exists.
- **`rm_missing_file_fails_unless_forced`**: `rm` on a missing file returns `1`. `rm -f` on the same file returns `0`. This pins down what `-f` is for.
- **`rm_directory_without_recursive_flag_fails_and_keeps_it`**: `rm dir` returns `1` and leaves the directory in place.
- **`cp_missing_source_fails_and_writes_nothing`**: `cp` from a missing source returns `1` and creates no destination.
- **`cp_directory_without_recursive_flag_fails`**: `cp dir dest` without `-r` returns `1` and creates no destination.
- **`mv_missing_source_fails_and_writes_nothing`**: `mv` from a missing source returns `1` and creates no destination.
- **`cat_missing_file_fails`**: `cat` on a missing file returns `1`.
- **`cat_with_one_missing_file_fails_overall`**: `cat good.txt missing.txt` returns `1`. One bad file makes the whole command fail, even though the good file still prints.
- **`touch_into_missing_directory_fails`**: `touch` into a directory that doesn't exist returns `1`.

## Bugs the tests found

### Bug 1: unclosed `{` panicked

**Where:** `extract_block` in `src/parser/block.rs`.

**What happened:** the brace scan searched for a matching `}` and started its end index at `0`. When no `}` was found, it sliced `s[1..0]`, which panics.

**Example:** `if true { echo hi` crashed the shell.

**Fix:** the scan now records `Option<usize>`, and a missing closing brace returns an error (`unclosed '{'`).

### Bug 2: non-ASCII text in a block panicked

**Where:** `find_keyword` in `src/parser/block.rs`.

**What happened:** the search stepped one byte at a time and sliced the string at each step. Slicing inside a multibyte character such as `é` panics.

**Also:** the brace scan used character indices where it needed byte indices. For `if true { echo é } else { echo b }`, the `else` after the closing brace was lost.

**Fix:** `find_keyword` now uses `match_indices`, which only yields valid character boundaries. The brace scan uses `char_indices`, which yields byte offsets.
