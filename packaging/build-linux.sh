#!/usr/bin/env bash
# Builds Bootcraft's package for one Linux family, meant to run as root in a
# clean container of that distro (the release workflow, or locally through
# scripts/build-linux-local.sh):
#
#   packaging/build-linux.sh deb|rpm|arch [--install-deps]
#
#   deb   built on Debian 13; installs on Debian 13 and Ubuntu 26.04. Ubuntu
#         24.04 and Mint 22 (Qt 6.4, whose QML can't load the app's
#         singletons) use the AppImage, which brings Qt 6.8
#   rpm   built on Fedora 44
#   arch  Arch Linux (.pkg.tar.zst, built with makepkg; not from the AUR)
#
# The version comes from the workspace Cargo.toml. The checkout is copied to
# a scratch folder first (it may be read-only or owned by another user), and
# the package lands in dist/. --install-deps installs the distro's build
# dependencies; Rust comes from rustup (deb, rpm) or the distro (arch).
#
# Honoured environment: CARGO_HOME, RUSTUP_HOME and CARGO_TARGET_DIR (caches
# between builds).
set -euo pipefail

TARGET="${1:-}"
INSTALL_DEPS=0
[[ "${2:-}" == "--install-deps" ]] && INSTALL_DEPS=1

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/dist"
WORK="${WORK_DIR:-/tmp/bootcraft-build}"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)"
# The workspace's rust-version.
MIN_RUST="$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -1)"

fail() {
  echo "build-linux: $*" >&2
  exit 1
}

[[ -n "$VERSION" ]] || fail "no version in Cargo.toml"
case "$TARGET" in
  deb | rpm | arch) ;;
  *) fail "usage: $0 deb|rpm|arch [--install-deps]" ;;
esac

install_deps() {
  case "$TARGET" in
    deb)
      apt-get update
      DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential debhelper fakeroot pkg-config ca-certificates curl git binutils \
        qt6-base-dev qt6-base-dev-tools qt6-declarative-dev qt6-declarative-dev-tools
      ;;
    rpm)
      dnf install -y rpm-build gcc-c++ make git curl tar binutils \
        qt6-qtbase-devel qt6-qtdeclarative-devel
      ;;
    arch)
      pacman -Syu --noconfirm --needed base-devel git binutils pkgconf rust \
        qt6-base qt6-declarative qt6-svg qt6-wayland xdg-desktop-portal polkit curl
      ;;
  esac
}

# rustup's stable toolchain when there's no cargo or it's older than MIN_RUST.
ensure_rust() {
  local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
  export PATH="$cargo_bin:$PATH"
  if command -v cargo >/dev/null 2>&1; then
    local have
    have="$(cargo --version | awk '{print $2}')"
    if [[ "$(printf '%s\n%s\n' "$MIN_RUST" "$have" | sort -V | head -1)" == "$MIN_RUST" ]]; then
      return
    fi
  fi
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
  command -v cargo >/dev/null 2>&1 || fail "cargo is missing after installing rustup"
}

# The checkout, without build output, in a scratch folder.
copy_source() {
  rm -rf "$WORK"
  mkdir -p "$WORK/src"
  tar -C "$ROOT" \
    --exclude=./.git --exclude=./target --exclude=./dist \
    --exclude=./packaging/arch/src --exclude=./packaging/arch/pkg \
    -cf - . | tar -C "$WORK/src" -xf -
}

build_deb() {
  cd "$WORK/src"
  cp -a packaging/debian/debian debian
  # One changelog entry for this version (the package's own history).
  cat >debian/changelog <<CHANGELOG
bootcraft (${VERSION}-1) unstable; urgency=medium

  * Release ${VERSION}.

 -- Arctic Latent <contact@arcticlatent.com>  $(date -R)
CHANGELOG
  # -d: Rust comes from rustup, not the cargo package in Build-Depends.
  dpkg-buildpackage -us -uc -b -d
  cp -f "../bootcraft_${VERSION}-1_amd64.deb" "$OUT/"
}

build_rpm() {
  local top="$WORK/rpmbuild"
  mkdir -p "$top"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
  tar -C "$WORK/src" --transform "s,^\.,bootcraft-${VERSION}," \
    -czf "$top/SOURCES/bootcraft-${VERSION}.tar.gz" .
  sed "s/^Version:.*/Version:        ${VERSION}/" "$WORK/src/packaging/fedora/bootcraft.spec" \
    >"$top/SPECS/bootcraft.spec"
  # --nodeps: Rust comes from rustup, not the cargo/rust packages.
  rpmbuild --define "_topdir $top" --nodeps -bb "$top/SPECS/bootcraft.spec"
  find "$top/RPMS" -name "bootcraft-${VERSION}-*.x86_64.rpm" -exec cp -f {} "$OUT/" \;
}

build_arch() {
  # makepkg won't run as root.
  id builder >/dev/null 2>&1 || useradd -m builder
  sed -i "s/^pkgver=.*/pkgver=${VERSION}/" "$WORK/src/packaging/arch/PKGBUILD"
  chown -R builder: "$WORK"
  local dir
  for dir in "${CARGO_HOME:-}" "${CARGO_TARGET_DIR:-}"; do
    [[ -n "$dir" ]] || continue
    mkdir -p "$dir"
    chown -R builder: "$dir"
  done
  runuser -u builder -- env \
    CARGO_HOME="${CARGO_HOME:-/home/builder/.cargo}" \
    BOOTCRAFT_CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-}" \
    bash -c "cd '$WORK/src/packaging/arch' && makepkg -f --noconfirm"
  cp -f "$WORK/src/packaging/arch/bootcraft-${VERSION}-1-x86_64.pkg.tar.zst" "$OUT/"
}

((INSTALL_DEPS)) && install_deps
[[ "$TARGET" == arch ]] || ensure_rust
mkdir -p "$OUT"
copy_source
echo "Building Bootcraft ${VERSION} (${TARGET})..."
"build_${TARGET}"
echo "Built:"
find "$OUT" -maxdepth 1 -type f -name "*${VERSION}*" -printf '  %f (%s bytes)\n'
