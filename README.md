<div align="center">

```
    ██████╗ ███████╗██╗  ██╗███████╗██╗     ██╗     
    ██╔══██╗██╔════╝██║  ██║██╔════╝██║     ██║     
    ██████╔╝███████╗███████║█████╗  ██║     ██║     
    ██╔══██╗╚════██║██╔══██║██╔══╝  ██║     ██║     
    ██║  ██║███████║██║  ██║███████╗███████╗███████╗
    ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝
```

**A shell that actually feels like a shell — written from scratch in Rust.**

MIT License · Built with Rust · Windows & Linux

</div>

---

I got fed up with PowerShell. So I built a replacement.

RShell is a fully custom shell written in Rust with its own parser, executor, line editor, glob engine, and built-in package manager. It's designed to feel natural coming from bash, work well on Windows without WSL, and stay out of your way.

## Platform Support

| Platform | Status |
|----------|--------|
| Windows (x64) | ✅ Works |
| Linux (x64, Ubuntu tested) | ✅ Works |
| macOS | ✅ Works, however untested |

---

## Features

- **Bash-like syntax** — pipes, redirects, `&&`, `||`, `;`, backgrounding with `&`
- **Built-in commands** — no dependency on coreutils; `ls`, `grep`, `find`, `cat`, `cp`, `mv`, `rm`, `head`, `tail`, `wc`, `sort`, `uniq`, `xargs`, and more work out of the box
- **Scripting** — `if/then/fi`, `for/in/do/done`, `while`, functions, `$((arithmetic))`, `$(command substitution)`, glob expansion
- **Smart line editor** — powered by [reedline](https://github.com/nushell/reedline) with history, tab completion, and fish-style inline hints
- **Package manager** — install tools like `ripgrep`, `fd`, `bat`, `fzf`, `node`, `python`, `go`, and more with a single command
- **Built-in text editor** — `mini` is a lightweight editor with syntax highlighting, undo, and keyboard shortcuts, directly in the shell
- **Git-aware prompt** — shows the current branch automatically
- **Persistent aliases and functions** — saved to `~/.myshellrc` and loaded on every session
- **"Did you mean?" suggestions** — Levenshtein distance matching when a command isn't found

---

## Installation

### Windows

Run this in PowerShell:

```powershell
irm https://raw.githubusercontent.com/JackMagee21/RSHELL/main/install.ps1 | iex
```

If you get an execution policy error, run this first, then try again:

```powershell
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### Linux

```sh
curl -sSf https://raw.githubusercontent.com/JackMagee21/RSHELL/main/install.sh | sh
```

Restart your terminal after installing, then type `rSHELL` to start.

### Download manually

Grab the latest binary from the [Releases](https://github.com/JackMagee21/RSHELL/releases/latest) page.

### Build from source

Requires Rust (stable). Install it via [rustup](https://rustup.rs) if you haven't already.

```sh
git clone https://github.com/JackMagee21/RSHELL
cd RSHELL
cargo build --release
./target/release/rSHELL
```

---

## Built-in Commands

| Category | Commands |
|----------|----------|
| **Navigation** | `cd`, `pwd`, `pushd`, `popd`, `dirs` |
| **Files** | `ls`, `mkdir`, `rm`, `cp`, `mv`, `cat`, `touch`, `chmod`, `ln` |
| **Search** | `grep`, `find` |
| **Text** | `head`, `tail`, `wc`, `sort`, `uniq`, `xargs`, `env` |
| **Shell** | `echo`, `export`, `unset`, `alias`, `unalias`, `source`, `history` |
| **Jobs** | `jobs`, `fg`, `bg`, `kill` |
| **Utilities** | `which`, `sleep`, `clear`, `help`, `true`, `false`, `test` |
| **Editor** | `mini <file>` |
| **Packages** | `pkg install`, `pkg uninstall`, `pkg list`, `pkg search`, `pkg upgrade` |

Run `help` for a full overview, or `help <topic>` for details on a specific area (e.g. `help scripting`, `help nav`, `help pkg`).

---

## Package Manager

RShell has a built-in package manager that downloads pre-built binaries and creates shims in `~/.rshell/bin`. The bin directory is automatically added to your `$PATH`.

```sh
# Search available packages
pkg search

# Install a package
install ripgrep
install node
install python

# List what's installed
pkg list

# Upgrade everything
pkg upgrade

# Remove a package
uninstall bat
```

**Available packages:**

| Package | Description |
|---------|-------------|
| `ripgrep` | Fast `grep` replacement (`rg`) |
| `fd` | Fast `find` replacement |
| `bat` | `cat` with syntax highlighting |
| `fzf` | Fuzzy finder |
| `delta` | Better git diff viewer |
| `zig` | Zig compiler (also works as a C/C++ compiler via `zig cc`) |
| `node` | Node.js runtime (includes `npm` and `npx`) |
| `python` | Python 3.12 with `pip` |
| `go` | Go programming language |
| `deno` | Deno JS/TS runtime |
| `jq` | Lightweight JSON processor |
| `dust` | Intuitive `du` replacement |
| `hyperfine` | Command-line benchmarking tool |

The registry is fetched from this repo at `registry/registry.json` and cached locally for an hour.

---

## Scripting

RShell supports a bash-compatible scripting syntax.

```sh
# Variables and arithmetic
x=10
echo $((x * 2 + 5))

# if / else
if test -f README.md; then
    echo "found it"
else
    echo "not here"
fi

# for loop
for f in *.rs; do
    echo "Processing $f"
done

# while loop
i=0
while test $i -lt 5; do
    echo $i
    i=$((i + 1))
done

# Functions
greet() {
    echo "Hello, $1!"
}
greet world

# Pipes and redirects
cat file.txt | grep "pattern" | sort | uniq -c > output.txt

# Command substitution
today=$(date +%Y-%m-%d)
echo "Today is $today"

# Background jobs
sleep 10 &
jobs
```

---

## The `mini` Editor

`mini` is a small but functional text editor built directly into the shell — no external dependencies needed.

```sh
mini myfile.rs
```

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save |
| `Ctrl+Z` | Undo |
| `Ctrl+Q` | Quit |
| Arrow keys | Move cursor |
| `Home` / `End` | Start / end of line |

Syntax highlighting is applied automatically for `.rs`, `.py`, `.js`, `.ts`, and `.sh` files.

---

## Shell Keybindings

| Key | Action |
|-----|--------|
| `Tab` | Complete command or file path |
| `↑` / `↓` | Navigate history |
| `Ctrl+C` | Cancel current input |
| `Ctrl+D` | Exit the shell |
| `Ctrl+L` | Clear the screen |
| `Ctrl+Z` | Suspend foreground job (Unix) |
| `!!` | Repeat last command |
| `!n` | Repeat command number `n` from history |

---

## Project Structure

```
src/
├── main.rs                     # Entry point, REPL loop
├── shell/
│   ├── mod.rs                  # Shell state, eval(), load_rc()
│   ├── prompt.rs               # Prompt building, git branch
│   ├── history.rs              # History load/save/expansion
│   └── persist.rs              # Alias and function persistence
├── parser/
│   ├── mod.rs                  # Parse entry point
│   ├── ast.rs                  # Command AST types
│   ├── tokenizer.rs            # Tokenizer
│   └── block.rs                # if / for / while parsers
├── executor/
│   ├── mod.rs                  # Command dispatch
│   ├── expand.rs               # Variable and arithmetic expansion
│   ├── pipeline.rs             # Pipe execution
│   └── builtin/
│       ├── mod.rs              # Builtin dispatch table
│       ├── core.rs             # cd, echo, alias, export, history ...
│       ├── fs.rs               # ls, mkdir, rm, cp, mv, cat, touch ...
│       ├── grep.rs             # grep
│       ├── find.rs             # find
│       ├── text.rs             # head, tail, wc, sort, uniq, xargs
│       ├── jobs.rs             # jobs, fg, bg, kill
│       ├── mini.rs             # Built-in text editor
│       ├── test.rs             # test / [
│       ├── util.rs             # Shared helpers
│       └── pkg/                # Package manager
│           ├── mod.rs
│           ├── install.rs      # Download, extract, shim creation
│           ├── registry.rs     # Registry fetch and caching
│           ├── meta.rs         # Package metadata
│           ├── paths.rs        # ~/.rshell path resolution
│           └── progress.rs     # Download/install progress bars
├── completion/
│   └── mod.rs                  # Tab completion engine
├── readline/
│   └── mod.rs                  # Line editor (reedline wrapper)
└── glob.rs                     # Glob expansion engine
registry/
└── registry.json               # Package registry
```

---

## Configuration

On startup, RShell loads `~/.myshellrc`. Aliases and functions defined during a session are automatically written back here.

Example `~/.myshellrc`:

```sh
# Aliases
alias gs='git status'
alias gc='git commit -m'
alias gp='git push'
alias ll='ls -la'

# Functions
mkcd() {
    mkdir -p $1
    cd $1
}
```

---

## Building for Release

```sh
# Linux
cargo build --release --target x86_64-unknown-linux-gnu

# Windows (requires cargo-xwin or a Windows machine)
cargo xwin build --release --target x86_64-pc-windows-msvc
```

See the `Makefile` for all available build targets (`make help`).

---

## Why?

Mainly to learn Rust properly — parsers, I/O, cross-platform system calls, the works. If you find it useful that's a bonus.

---

## License

MIT — see [LICENSE](LICENSE).
> Building from source? See [BUILDING.md](BUILDING.md) for the full guide.