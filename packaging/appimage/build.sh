#!/usr/bin/env bash
# Builds Bootcraft's AppImage (the release workflow's AppImage job):
#
#   packaging/appimage/build.sh [--install-deps]
#
# Meant for Ubuntu 22.04 (its glibc is the oldest the AppImage then needs) with
# Qt 6.8 from the Qt installer or aqtinstall: QT_ROOT_DIR, or qmake on PATH.
# --install-deps installs the Ubuntu build packages, and Qt 6.8.3 into
# /opt/qt with aqtinstall when there's no Qt yet. Bundles Qt (QML modules, SVG,
# Wayland, the desktop portal theme) with linuxdeploy and its Qt plugin. The
# helper is built static (musl): the app copies it out of the AppImage before
# pkexec runs it, since root can't read the AppImage's FUSE mount. The
# AppImage lands in dist/Bootcraft-<version>-x86_64.AppImage.
#
# Honoured environment: CARGO_HOME, RUSTUP_HOME, CARGO_TARGET_DIR.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/dist"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)"
MIN_RUST="$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)"
APP_ID="io.github.ArcticLatent.Bootcraft"
QT_VERSION="6.8.3"
MUSL="x86_64-unknown-linux-musl"
# linuxdeploy and its Qt plugin, pinned to known-good continuous builds (as ComfyPilot).
LINUXDEPLOY="https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage"
LINUXDEPLOY_QT="https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/1-alpha-20250213-1/linuxdeploy-plugin-qt-x86_64.AppImage"

fail() {
  echo "appimage: $*" >&2
  exit 1
}

[[ -n "$VERSION" ]] || fail "no version in Cargo.toml"

sudo_if_needed() {
  if [[ $EUID -eq 0 ]]; then "$@"; else sudo "$@"; fi
}

install_deps() {
  sudo_if_needed apt-get update
  # Build tools, and the libraries Qt's xcb/Wayland/EGL plugins link against,
  # which linuxdeploy must find to bundle them.
  sudo_if_needed env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl git file pkg-config python3-pip libcom-err2 libgpg-error0 \
    libdbus-1-3 libgl-dev libegl-dev libfontconfig1 libfreetype6 libglib2.0-0 \
    libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libwayland-cursor0 libwayland-egl1 \
    libxcb-cursor0 libxcb-icccm4 libxcb-image0 libxcb-keysyms1 libxcb-randr0 \
    libxcb-render-util0 libxcb-shape0 libxcb-xinerama0 libxcb-xkb1 libxcb-xfixes0 \
    libx11-xcb1 libxrender1 libxi6 libsm6 libice6
  if ! qt_root >/dev/null; then
    python3 -m pip install --quiet "aqtinstall==3.3.0"
    python3 -m aqt install-qt linux desktop "$QT_VERSION" linux_gcc_64 -O /opt/qt
  fi
}

# Qt's install folder (with bin/qmake).
qt_root() {
  local dir
  for dir in "${QT_ROOT_DIR:-}" "/opt/qt/$QT_VERSION/gcc_64"; do
    [[ -n "$dir" && -x "$dir/bin/qmake" ]] && { echo "$dir"; return 0; }
  done
  command -v qmake >/dev/null 2>&1 && { qmake -query QT_INSTALL_PREFIX; return 0; }
  return 1
}

ensure_rust() {
  local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
  export PATH="$cargo_bin:$PATH"
  local have=""
  command -v cargo >/dev/null 2>&1 && have="$(cargo --version | awk '{print $2}')"
  if [[ -z "$have" || "$(printf '%s\n%s\n' "$MIN_RUST" "$have" | sort -V | head -1)" != "$MIN_RUST" ]]; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
  fi
  rustup target add "$MUSL"
}

[[ "${1:-}" == "--install-deps" ]] && install_deps
ensure_rust
QT="$(qt_root)" || fail "no Qt found (set QT_ROOT_DIR, or put Qt's qmake on PATH)"
export PATH="$QT/bin:$PATH" QMAKE="$QT/bin/qmake" LD_LIBRARY_PATH="$QT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
echo "Building Bootcraft $VERSION (AppImage) with Qt from $QT..."

cd "$ROOT"
cargo build --release --locked -p bootcraft
cargo build --release --locked -p bootcraft-helper --target "$MUSL"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

WORK="${WORK_DIR:-/tmp/bootcraft-appimage}"
APPDIR="$WORK/AppDir"
rm -rf "$WORK"
mkdir -p "$APPDIR/usr/bin" "$OUT"
install -m755 "$TARGET_DIR/release/bootcraft" "$APPDIR/usr/bin/bootcraft"
install -Dm755 "$TARGET_DIR/$MUSL/release/bootcraft-helper" "$APPDIR/usr/lib/bootcraft/bootcraft-helper"
install -Dm644 "packaging/linux/$APP_ID.desktop" "$APPDIR/usr/share/applications/$APP_ID.desktop"
install -Dm644 "packaging/linux/$APP_ID.metainfo.xml" "$APPDIR/usr/share/metainfo/$APP_ID.appdata.xml"
for size in 32 64 128 256 512; do
  install -Dm644 "crates/app/icons/icon-$size.png" "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps/$APP_ID.png"
done
install -Dm644 crates/app/icons/icon.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"

cd "$WORK"
curl -fsSL -o linuxdeploy "$LINUXDEPLOY"
curl -fsSL -o linuxdeploy-plugin-qt "$LINUXDEPLOY_QT"
chmod +x linuxdeploy linuxdeploy-plugin-qt

# No FUSE in containers and CI: the tools unpack themselves instead.
export APPIMAGE_EXTRACT_AND_RUN=1
# The QML the app imports (it's compiled in, so point the scanner at the
# sources), the Wayland platform plugins next to xcb (and offscreen, for
# headless checks), and the output name.
export QML_SOURCES_PATHS="$ROOT/crates/app/qml"
export EXTRA_PLATFORM_PLUGINS="libqwayland-egl.so;libqwayland-generic.so;libqoffscreen.so"
export LDAI_OUTPUT="Bootcraft-$VERSION-x86_64.AppImage"
export OUTPUT="$LDAI_OUTPUT"
./linuxdeploy --appdir "$APPDIR" --plugin qt \
  --desktop-file "$APPDIR/usr/share/applications/$APP_ID.desktop" \
  --icon-file "$APPDIR/usr/share/icons/hicolor/256x256/apps/$APP_ID.png"

# What the app needs at runtime but the QML scanner can't see: the Basic
# Controls style (set in main.rs), SVG for the icons, the desktop portal theme
# (native file dialogs; set in main.rs) and Wayland's shell, decoration and
# graphics plugins (windows can't open on Wayland without xdg-shell).
QML_DIR="$APPDIR/usr/qml"
PLUGIN_DIR="$APPDIR/usr/plugins"
[[ -d "$QML_DIR/QtQuick/Controls/Basic" ]] || cp -a "$QT/qml/QtQuick/Controls/Basic" "$QML_DIR/QtQuick/Controls/"
for plugin in imageformats/libqsvg.so iconengines/libqsvgicon.so platformthemes/libqxdgdesktopportal.so; do
  mkdir -p "$PLUGIN_DIR/$(dirname "$plugin")"
  cp -an "$QT/plugins/$plugin" "$PLUGIN_DIR/$plugin"
done
for dir in wayland-shell-integration wayland-decoration-client wayland-graphics-integration-client; do
  cp -an "$QT/plugins/$dir" "$PLUGIN_DIR/"
done
# Libraries linuxdeploy leaves out as "always there" that minimal Debian and
# openSUSE systems lack: libcom_err (for Qt Network's Kerberos libraries) and
# libgpg-error (for libgcrypt, via libsystemd, via D-Bus).
for soname in libcom_err.so.2 libgpg-error.so.0; do
  lib="$(ldconfig -p | awk -v n="$soname" '$1 == n && /x86-64/ {print $NF; exit}')"
  [[ -n "$lib" ]] || fail "$soname not found on the build system"
  cp -L "$lib" "$APPDIR/usr/lib/$soname"
done

# A second pass bundles what the plugins copied above link against.
./linuxdeploy --appdir "$APPDIR" --output appimage
mv -f "$LDAI_OUTPUT" "$OUT/"
echo "Built $OUT/$LDAI_OUTPUT ($(du -h "$OUT/$LDAI_OUTPUT" | cut -f1))"
