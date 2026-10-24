#!/usr/bin/env bash
# Writes and signs a release's manifest, next to its files in a folder (the
# release workflow's publish job):
#
#   packaging/release-manifests.sh <dir> <version> <owner/repo>
#
#   linux-release.json   the .deb, .rpm, Arch package and AppImage, with their
#                        SHA-256
#   SHA256SUMS           every file's checksum
#
# The manifest is signed with ARCTIC_UPDATE_SIGNING_KEY (tools/
# manifest-signer) and checked against the public key built into
# Bootcraft, so a wrong key stops the release. It has the format ComfyPilot
# publishes (and Bootcraft checks when it installs ComfyPilot).
set -euo pipefail

DIR="${1:?usage: $0 <dir> <version> <owner/repo>}"
VERSION="${2:?version missing}"
REPOSITORY="${3:?repository missing}"
TAG="v$VERSION"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_URL="https://github.com/$REPOSITORY/releases/download/$TAG"

[[ -n "${ARCTIC_UPDATE_SIGNING_KEY:-}" ]] || {
  echo "ARCTIC_UPDATE_SIGNING_KEY is not set" >&2
  exit 1
}

sha() { sha256sum "$1" | awk '{print $1}'; }
signer() {
  cargo run --quiet --release --locked --manifest-path "$ROOT/Cargo.toml" -p bootcraft-manifest-signer -- "$@"
}

mapfile -t packages < <(find "$DIR" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.zst' -o -name '*.AppImage' \) -printf '%f\n' | sort)
((${#packages[@]} == 4)) || {
  echo "expected a .deb, an .rpm, an Arch package and an AppImage in $DIR, found: ${packages[*]:-none}" >&2
  exit 1
}

for name in "${packages[@]}"; do
  jq -n --arg name "$name" --arg sha "$(sha "$DIR/$name")" --arg url "$BASE_URL/$name" \
    '{name: $name, sha256: $sha, download_url: $url}'
done | jq -s --arg version "$VERSION" --arg tag "$TAG" --arg repo "$REPOSITORY" \
  '{version: $version, tag: $tag, repository: $repo, assets: .}' >"$DIR/linux-release.json"

signer sign --manifest "$DIR/linux-release.json"
signer verify --manifest "$DIR/linux-release.json"

(cd "$DIR" && sha256sum "${packages[@]}" linux-release.json >SHA256SUMS)
echo "Manifest for $TAG:"
cat "$DIR/SHA256SUMS"
