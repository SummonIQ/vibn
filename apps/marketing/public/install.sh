#!/bin/sh
# Vibn installer — downloads the macOS Apple Silicon build, drops it into
# /Applications, strips the quarantine flag, and launches it. Designed to
# be piped from curl: `curl -fsSL https://vibn.dev/install.sh | sh`.
#
# We download with curl (not the browser) so macOS never marks the bundle
# as quarantined in the first place, which is why this avoids the
# "Vibn is damaged" Gatekeeper dialog that ad-hoc-signed apps trip on
# Sequoia and later.

set -eu

VERSION="v0.1.0"
DMG_URL="https://50cbcsvzhpu0fjiw.public.blob.vercel-storage.com/${VERSION}/Vibn_0.1.0_aarch64.dmg"
APP_NAME="Vibn.app"

red()    { printf '\033[31m%s\033[0m\n' "$1" >&2; }
green()  { printf '\033[32m%s\033[0m\n' "$1"; }
dim()    { printf '\033[2m%s\033[0m\n' "$1"; }
step()   { printf '\033[36m→\033[0m %s\n' "$1"; }

if [ "$(uname -s)" != "Darwin" ]; then
  red "Vibn currently only ships a macOS build. Linux & Windows are coming."
  exit 1
fi
if [ "$(uname -m)" != "arm64" ]; then
  red "This build is Apple Silicon only. Intel Mac support isn't packaged yet."
  exit 1
fi

DEST="/Applications"
if [ ! -w "$DEST" ]; then
  DEST="$HOME/Applications"
  mkdir -p "$DEST"
  dim "  /Applications isn't writable; installing to $DEST instead."
fi

TMP=$(mktemp -d -t vibn-install)
MNT=""
cleanup() {
  if [ -n "$MNT" ] && [ -d "$MNT" ]; then
    hdiutil detach "$MNT" -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT INT TERM

step "Downloading Vibn ${VERSION}..."
if ! curl -fsSL --progress-bar "$DMG_URL" -o "$TMP/vibn.dmg"; then
  red "Download failed. Check your network and try again."
  exit 1
fi

step "Mounting installer..."
# hdiutil prints one line per partition; the mount point is the last
# whitespace-separated field on the line that has one.
MNT=$(hdiutil attach "$TMP/vibn.dmg" -nobrowse -readonly \
        | grep -E '\s/Volumes/' \
        | tail -1 \
        | awk '{ for (i=1; i<=NF; i++) if ($i ~ /^\/Volumes\//) { for (j=i; j<=NF; j++) printf "%s%s", $j, (j<NF?" ":""); print ""; exit } }')

if [ -z "$MNT" ] || [ ! -d "$MNT/$APP_NAME" ]; then
  red "Couldn't find $APP_NAME inside the DMG."
  exit 1
fi

step "Installing to $DEST/$APP_NAME..."
if [ -d "$DEST/$APP_NAME" ]; then
  rm -rf "$DEST/$APP_NAME"
fi
cp -R "$MNT/$APP_NAME" "$DEST/"

# Strip quarantine in case any nested file inherited it.
xattr -dr com.apple.quarantine "$DEST/$APP_NAME" 2>/dev/null || true

green "✓ Vibn installed at $DEST/$APP_NAME"
step "Launching..."
open -a "$DEST/$APP_NAME"
