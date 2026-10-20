#!/bin/sh
set -e

REPO="JackMagee21/RSHELL"
INSTALL_DIR="$HOME/.local/bin"

echo ""
echo "  Installing rSHELL..."

# Check curl is available
if ! command -v curl >/dev/null 2>&1; then
    echo "  Error: curl is required but not installed."
    exit 1
fi

# Fetch latest release info
RELEASE=$(curl -sf "https://api.github.com/repos/$REPO/releases/latest")
if [ -z "$RELEASE" ]; then
    echo "  Error: Could not reach GitHub API. Check your internet connection."
    exit 1
fi

# Extract the Linux binary URL (exclude .exe)
URL=$(echo "$RELEASE" | grep "browser_download_url" | grep -v ".exe" | cut -d '"' -f 4 | head -n 1)
if [ -z "$URL" ]; then
    echo "  Error: No Linux binary found in the latest release."
    exit 1
fi

VERSION=$(echo "$RELEASE" | grep '"tag_name"' | cut -d '"' -f 4)

# Create install directory
mkdir -p "$INSTALL_DIR"

# Download the binary
echo "  Downloading rSHELL $VERSION..."
curl -L --progress-bar "$URL" -o "$INSTALL_DIR/rSHELL"
chmod +x "$INSTALL_DIR/rSHELL"

# Check if install dir is in PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        ;;
    *)
        echo ""
        echo "  Note: $INSTALL_DIR is not in your PATH."
        echo "  Add this to your ~/.bashrc or ~/.zshrc:"
        echo ""
        echo '    export PATH="$HOME/.local/bin:$PATH"'
        echo ""
        ;;
esac

echo ""
echo "  Done! rSHELL $VERSION installed to $INSTALL_DIR/rSHELL"
echo "  Restart your terminal, then type: rSHELL"
echo ""