# Building rSHELL from Source

This document covers everything you need to build, run, and deploy rSHELL using the provided Makefile.

---

## Prerequisites

You'll need the following installed before any of this works:

- **Rust (stable)** — install via [rustup](https://rustup.rs)
- **cargo-xwin** — only needed for building the Windows `.exe` from Linux/macOS
- **clang + llvm + nasm** — only needed for Windows cross-compilation

### Installing cargo-xwin

```sh
cargo install cargo-xwin
```

### Installing the cross-compilation tools (Linux)

```sh
sudo apt update
sudo apt install clang llvm nasm
```

### Adding the Windows target

```sh
rustup target add x86_64-pc-windows-msvc
```

---

## Quick Reference

| Command | What it does |
|---------|-------------|
| `make check` | Check for errors without building |
| `make build` | Debug build |
| `make run` | Build and run locally |
| `make linux` | Build optimised Linux binary |
| `make exe` | Build Windows `.exe` |
| `make ship` | Build `.exe` and deploy to Windows in one command |
| `make deploy` | Copy existing `.exe` to Windows folder |
| `make fmt` | Format code |
| `make lint` | Run Clippy linter |
| `make test` | Run tests |
| `make clean` | Wipe build artifacts |
| `make clean-all` | Wipe build artifacts and remove deployed `.exe` |

---

## Development

### Check for errors

The fastest feedback loop — checks your code compiles without actually building a binary:

```sh
make check
```

### Debug build

Builds quickly, no optimisations. Good for testing changes:

```sh
make build
```

Output: `target/debug/rSHELL`

### Run locally

Builds a debug binary and launches the shell immediately:

```sh
make run
```

### Format code

```sh
make fmt
```

### Lint

Runs Clippy and reports warnings:

```sh
make lint
```

### Tests

```sh
make test
```

---

## Building Releases

### Linux binary

Builds an optimised binary for Linux x64:

```sh
make linux
```

Output: `target/x86_64-unknown-linux-gnu/release/rSHELL`

### Windows `.exe`

Cross-compiles a Windows binary from Linux using `cargo-xwin`. Requires the prerequisites above.

```sh
make exe
```

Output: `target/x86_64-pc-windows-msvc/release/rSHELL.exe`

### Both platforms

```sh
make all-platforms
```

---

## Deploying to Windows

These commands copy the built `.exe` to a folder on the Windows side of WSL. The Makefile automatically detects your Windows username.

### Deploy to `C:\Users\<you>\RSHELL\`

```sh
make deploy
```

This is what `make ship` calls after building. The folder is created if it doesn't exist.

### Build and deploy in one command

The one you'll use most often:

```sh
make ship
```

Equivalent to running `make exe` then `make deploy`.

### Copy to Desktop

```sh
make install-win
```

### Copy to Program Files

```sh
make install-win-system
```

Installs to `C:\Program Files\myshell\`. May require elevated permissions.

---

## Cleaning Up

### Remove build artifacts

Wipes the `target/` directory:

```sh
make clean
```

### Remove deployed `.exe`

Removes the `.exe` from `C:\Users\<you>\RSHELL\`:

```sh
make undeploy
```

### Remove everything

```sh
make clean-all
```

---

## Troubleshooting

**`make: cargo: No such file or directory`**

Cargo isn't in the PATH that Make sees. Add this to your `~/.bashrc`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

Then `source ~/.bashrc` and try again.

**`failed to find tool "clang-cl"`**

Install clang:

```sh
sudo apt update && sudo apt install clang
```

**`failed to find tool "llvm-lib"`**

Install llvm:

```sh
sudo apt install llvm
```

If it installs with a version suffix (e.g. `llvm-lib-17`), symlink it:

```sh
sudo ln -s /usr/bin/llvm-lib-17 /usr/local/bin/llvm-lib
```

**`NASM command not found`**

```sh
sudo apt install nasm
```

**`the x86_64-pc-windows-msvc target may not be installed`**

```sh
rustup target add x86_64-pc-windows-msvc
```