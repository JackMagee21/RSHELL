# ── myshell Makefile ──────────────────────────────────────

# GNU Make on native Windows defaults to cmd.exe as its recipe shell,
# but every recipe below is POSIX shell. Force Git Bash (already
# required to `git clone` this repo in the first place) when present,
# so recipes behave the same on Windows as on Linux/macOS/WSL.
ifeq ($(OS),Windows_NT)
  GITBASH := C:/Program Files/Git/bin/bash.exe
  ifneq ($(shell if exist "$(GITBASH)" echo yes),yes)
    GITBASH := $(subst \,/,$(LOCALAPPDATA))/Programs/Git/bin/bash.exe
    ifneq ($(shell if exist "$(subst /,\,$(GITBASH))" echo yes),yes)
      GITBASH :=
    endif
  endif
  ifneq ($(GITBASH),)
    SHELL := $(GITBASH)
    .SHELLFLAGS := -c
  endif
endif

APP_NAME = rSHELL
WIN_TARGET = x86_64-pc-windows-msvc
LINUX_TARGET = x86_64-unknown-linux-gnu
INSTALL_DIR = /mnt/c/Program Files/myshell
WINDOWS_USER = $(shell cmd.exe /C "echo %USERNAME%" 2>/dev/null | tr -d '\r')
DEPLOY_DIR = /mnt/c/Users/$(WINDOWS_USER)/RSHELL

# Default - just check the code
all: check

# Check for errors without building
check:
	cargo check

# Build debug (fast, for development)
build:
	cargo build

# Build optimized Linux binary
linux:
	cargo build --release --target $(LINUX_TARGET)
	@echo "✅ Linux binary: target/$(LINUX_TARGET)/release/$(APP_NAME)"

# Build Windows .exe
exe:
	cargo xwin build --release --target $(WIN_TARGET)
	@echo "✅ Windows binary: target/$(WIN_TARGET)/release/$(APP_NAME).exe"

# Build both
all-platforms: linux exe

# Copy .exe to Windows desktop
install-win:
	cp target/$(WIN_TARGET)/release/$(APP_NAME).exe \
	   "/mnt/c/Users/$$WINDOWS_USER/Desktop/$(APP_NAME).exe"
	@echo "✅ Copied to Desktop"

# Copy .exe to Program Files
install-win-system:
	mkdir -p "$(INSTALL_DIR)"
	cp target/$(WIN_TARGET)/release/$(APP_NAME).exe "$(INSTALL_DIR)/"
	@echo "✅ Installed to Program Files"

# Installs Rust via rustup if `cargo` isn't already on PATH, so a
# fresh clone needs nothing but `make` (+ curl) pre-installed.
ensure-rust:
	@command -v cargo >/dev/null 2>&1 && exit 0; \
	echo "⚠️  Rust not found — installing via rustup..."; \
	case "$$(uname -s 2>/dev/null)" in \
		MINGW*|MSYS*) \
			curl -sSfo /tmp/rustup-init.exe https://win.rustup.rs/x86_64 && \
			/tmp/rustup-init.exe -y --default-toolchain stable ;; \
		*) \
			curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable ;; \
	esac

# Windows-only: `cargo build` needs a C/C++ linker, and Rust's default
# Windows target (x86_64-pc-windows-msvc) needs the one that ships with
# Visual Studio Build Tools — rustup doesn't install it for you. Checked
# via vswhere (the official way to query installed VS components), since
# link.exe isn't normally on PATH even after a correct install.
ensure-msvc:
	@case "$$(uname -s 2>/dev/null)" in \
		MINGW*|MSYS*) \
			VSWHERE="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"; \
			if [ -x "$$VSWHERE" ] && "$$VSWHERE" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>/dev/null | grep -q .; then \
				exit 0; \
			fi; \
			echo "⚠️  MSVC C++ build tools not found — installing via winget."; \
			echo "    This is a multi-GB download and can take several minutes,"; \
			echo "    and may prompt you for admin approval (UAC)."; \
			winget install --id Microsoft.VisualStudio.2022.BuildTools --silent \
				--accept-package-agreements --accept-source-agreements \
				--override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" ;; \
		*) exit 0 ;; \
	esac

# Run the shell locally (any OS with `make` + `curl`)
run: ensure-rust ensure-msvc
	@export PATH="$$HOME/.cargo/bin:$$PATH"; cargo run --bin $(APP_NAME)

# Run tests
test:
	cargo test

# Remove JUST the build artifacts
clean:
	cargo clean
	@echo "✅ Build artifacts cleaned"

# Remove the deployed .exe from Windows
undeploy:
	@rm -f "$(DEPLOY_DIR)/$(APP_NAME).exe"
	@echo "✅ Removed from C:\Users\$(WINDOWS_USER)\RSHELL"

# Nuclear option - clean everything including deployed files
clean-all: clean undeploy
	@echo "✅ Everything cleaned"


# Format code
fmt:
	cargo fmt

# Lint
lint:
	cargo clippy

# Copy .exe to C:\Users\mtj07\RSHELL (creates folder if it doesn't exist)
deploy:
	@mkdir -p "$(DEPLOY_DIR)"
	@cp target/$(WIN_TARGET)/release/$(APP_NAME).exe "$(DEPLOY_DIR)/$(APP_NAME).exe"
	@echo "✅ Deployed to C:\Users\$(WINDOWS_USER)\RSHELL\$(APP_NAME).exe"

# Build fresh .exe then deploy in one command
ship: exe deploy

help:
	@echo ""
	@echo "  🦀 myshell — available commands"
	@echo ""
	@echo "  Development"
	@echo "    make check        Check for errors without building"
	@echo "    make build        Debug build (fast)"
	@echo "    make run          Run the shell locally"
	@echo "    make fmt          Format code"
	@echo "    make lint         Run clippy linter"
	@echo "    make test         Run tests"
	@echo ""
	@echo "  Building"
	@echo "    make linux        Build optimised Linux binary"
	@echo "    make exe          Build Windows .exe"
	@echo "    make all-platforms  Build both"
	@echo ""
	@echo "  Deploying"
	@echo "    make deploy       Copy .exe to C:\Users\$(WINDOWS_USER)\RSHELL"
	@echo "    make ship         Build .exe + deploy in one command"
	@echo "    make undeploy     Remove .exe from Windows folder"
	@echo ""
	@echo "  Cleaning"
	@echo "    make clean        Wipe build artifacts (target/ folder)"
	@echo "    make clean-all    Clean build artifacts + deployed .exe"
	@echo ""

.PHONY: all check build linux exe all-platforms install-win install-win-system ensure-rust ensure-msvc run test clean fmt lint