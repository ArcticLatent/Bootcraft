#!/usr/bin/env bash
# Check every package Bootcraft can install against a distribution's real
# repositories, inside a container. Bootcraft's own tasks enable the extra
# repositories first (RPM Fusion, contrib/non-free, multiverse, multilib), so
# this also runs the helper for real. With --install it then applies a few
# safe tasks (codecs, archive tools, fonts...) as an end-to-end test.
#
# Usage: scripts/check-packages.sh <fedora|debian|ubuntu-24.04|ubuntu-26.04|arch> [--install]
# Needs podman (or CONTAINER=docker) and the static helper:
#   cargo build --release -p bootcraft-helper --target x86_64-unknown-linux-musl
set -euo pipefail

distro="${1:?usage: $0 <fedora|debian|ubuntu-24.04|ubuntu-26.04|arch> [--install]}"
install="${2:-}"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
helper="$repo_root/target/x86_64-unknown-linux-musl/release/bootcraft-helper"
container="${CONTAINER:-podman}"
[[ -x "$helper" ]] || { echo "build the static helper first (see the header)" >&2; exit 2; }

# NVIDIA's Debian 13 keyring, pinned in the catalog; check names from its repository too.
cuda_url="$(grep -A1 'CUDA_KEYRING_URL: &str' "$repo_root/crates/core/src/catalog/debian.rs" | grep -o 'https://[^"]*')"
cuda_sha="$(sed -n 's/.*CUDA_KEYRING_SHA256: &str = "\([0-9a-f]*\)";/\1/p' "$repo_root/crates/core/src/catalog/debian.rs")"

case "$distro" in
  fedora)        image=registry.fedoraproject.org/fedora:44 ;;
  debian)        image=docker.io/library/debian:trixie ;;
  ubuntu-24.04)  image=docker.io/library/ubuntu:24.04 ;;
  ubuntu-26.04)  image=docker.io/library/ubuntu:26.04 ;;
  arch)          image=docker.io/library/archlinux:latest ;;
  *) echo "unknown distribution: $distro" >&2; exit 2 ;;
esac

# The script that runs inside the container. `bc` is the helper.
inner=$(cat <<'INNER'
set -euo pipefail
bc() { /bootcraft-helper "$@"; }
missing=()
check_apt() {
  local name
  for name in "$@"; do
    apt-cache show "$name" >/dev/null 2>&1 && continue
    # A virtual package another one provides.
    apt-cache showpkg "$name" 2>/dev/null | sed -n '/^Reverse Provides:/,$p' | grep -q ' ' && continue
    missing+=("$name")
  done
}
case "$DISTRO" in
  fedora)
    bc apply --yes --select fedora.rpmfusion,fedora.dnf-tweaks
    mapfile -t names < <(bc packages | awk '$1=="repo"{print $2}')
    mapfile -t found < <(dnf repoquery -q --qf '%{name}\n' "${names[@]}" 2>/dev/null | sort -u)
    for name in "${names[@]}"; do
      printf '%s\n' "${found[@]}" | grep -qx "$name" && continue
      dnf repoquery -q --whatprovides "$name" 2>/dev/null | grep -q . && continue
      missing+=("$name")
    done
    while read -r group; do
      dnf group info "$group" >/dev/null 2>&1 || missing+=("@$group")
    done < <(bc packages | awk '$1=="group"{print $2}')
    tasks=fedora.dnf-tweaks,media.codecs,tools.archive,tools.drives,system.fonts
    ;;
  debian|ubuntu-*)
    if [[ "$DISTRO" == debian ]]; then
      bc apply --yes --select debian.sources
      apt-get update -qq
      apt-get install -y -qq curl ca-certificates >/dev/null
      curl -fsSLo /tmp/cuda-keyring.deb "$CUDA_URL"
      echo "$CUDA_SHA  /tmp/cuda-keyring.deb" | sha256sum -c --quiet
      dpkg -i /tmp/cuda-keyring.deb >/dev/null
      tasks=debian.sources,apt.i386,media.codecs,tools.archive,tools.drives,system.fonts,system.ms-fonts
    else
      bc apply --yes --select ubuntu.multiverse
      tasks=ubuntu.multiverse,apt.i386,media.codecs,tools.archive,tools.drives,system.fonts
    fi
    apt-get update -qq
    mapfile -t names < <(bc packages | awk '$1=="repo"{print $2}')
    check_apt "${names[@]}"
    ;;
  arch)
    bc apply --yes --select arch.multilib
    while read -r name; do
      pacman -Sp --print-format %n "$name" >/dev/null 2>&1 || missing+=("$name")
    done < <(bc packages | awk '$1=="repo"{print $2}')
    tasks=arch.pacman-tweaks,media.codecs,tools.archive,tools.drives,system.fonts
    ;;
esac
echo "checked $(bc packages | awk '$1=="repo" || $1=="group"' | wc -l) repository packages"
if ((${#missing[@]})); then
  printf 'MISSING: %s\n' "${missing[@]}"
  exit 1
fi
echo "all present"
if [[ "$INSTALL" == --install ]]; then
  echo "== installing: $tasks"
  bc apply --yes --select "$tasks"
  echo "== second run (everything should be already done)"
  bc apply --yes --select "$tasks" | grep -E '^\s+\[' 
fi
INNER
)

aur_check() {
  # The AUR has no container; ask its API directly.
  local names query
  mapfile -t names < <("$helper" packages 2>/dev/null | awk '$1=="aur"{print $2}')
  ((${#names[@]})) || return 0
  query=$(printf '&arg[]=%s' "${names[@]}")
  local found
  found=$(curl -fsSL "https://aur.archlinux.org/rpc/v5/info?${query:1}" | grep -o '"Name":"[^"]*"' | cut -d'"' -f4)
  local name bad=0
  for name in "${names[@]}"; do
    grep -qx "$name" <<<"$found" || { echo "MISSING (AUR): $name"; bad=1; }
  done
  echo "checked ${#names[@]} AUR packages"
  return $bad
}

"$container" run --rm -i \
  -e DISTRO="$distro" -e INSTALL="$install" -e CUDA_URL="$cuda_url" -e CUDA_SHA="$cuda_sha" \
  -v "$helper:/bootcraft-helper:ro" \
  "$image" bash -c "$inner"
if [[ "$distro" == arch ]]; then aur_check; fi
