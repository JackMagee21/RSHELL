## What is RSHELL?

up with Windows PowerShell and wanted something that felt like a real Linux terminal, so I built one.

RShell is a shell written from scratch in Rust with no bloat (not 100% bout that), no config files to wrestle with, and built-in commands that actually work the way you'd expect coming from bash.

**Platform support:**
| Platform | Status |
|----------|--------|
| Windows | ✅ Works great |
| Linux (Ubuntu tested, others should be fine) | ✅ Works great |
| macOS | ¯\\\_(ツ)\_/¯ probably works, untested |

| Category | Commands |
|----------|----------|
| **Navigation** | `cd`, `pwd`, `pushd`, `popd`, `dirs` |
| **Files** | `ls`, `mkdir`, `rm`, `cp`, `mv`, `cat`, `touch` |
| **Search** | `grep`, `find` |
| **Text** | `head`, `tail`, `wc`, `env` |
| **Shell** | `echo`, `export`, `unset`, `alias`, `unalias`, `source`, `history` |
| **Jobs** | `jobs`, `fg`, `bg`, `kill` |
| **Utilities** | `which`, `sleep`, `clear`,`help`, `true`, `false`, `test` |