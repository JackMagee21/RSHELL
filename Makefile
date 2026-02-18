# ── myshell Makefile ──────────────────────────────────────

APP_NAME = new_R_Shell
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

# Run the shell locally (Linux)
run:
	cargo run --bin $(APP_NAME)

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

.PHONY: all check build linux exe all-platforms install-win install-win-system run test clean fmt lint