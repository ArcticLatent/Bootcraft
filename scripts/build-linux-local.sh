#!/usr/bin/env bash
# Builds Bootcraft's Linux packages on this machine, each in a clean podman
# container of its distro (the same scripts the release workflow runs):
#
#   scripts/build-linux-local.sh [deb|rpm|arch|appimage|all]   (default: all)
#
# Packages land in dist/. Rust toolchains, crates and build output are kept
# in a podman volume per distro, so later builds are quick.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGETS=("${@:-all}")
[[ "${TARGETS[0]}" == all ]] && TARGETS=(deb rpm arch appimage)

image_for() {
  case "$1" in
    deb) echo "docker.io/library/debian:trixie" ;;
    rpm) echo "registry.fedoraproject.org/fedora:44" ;;
    arch) echo "docker.io/library/archlinux:latest" ;;
    appimage) echo "docker.io/library/ubuntu:22.04" ;;
    *) echo "unknown target: $1" >&2; exit 1 ;;
  esac
}

for target in "${TARGETS[@]}"; do
  image="$(image_for "$target")"
  echo "== $target in $image"
  # The AppImage has its own script, and keeps the Qt it downloads.
  script=(bash /src/packaging/build-linux.sh "$target" --install-deps)
  extra=()
  if [[ "$target" == appimage ]]; then
    script=(bash /src/packaging/appimage/build.sh --install-deps)
    extra=(-v bootcraft-qt683:/opt/qt)
  fi
  podman run --rm \
    -v "$ROOT:/src:Z" \
    -v "bootcraft-build-$target:/cache" \
    "${extra[@]}" \
    -e CARGO_HOME=/cache/cargo \
    -e RUSTUP_HOME=/cache/rustup \
    -e CARGO_TARGET_DIR=/cache/target \
    "$image" "${script[@]}"
done
ls -l "$ROOT/dist"
