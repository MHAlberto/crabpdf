#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(awk -F '"' '/^version = "/ { print $2; exit }' "$root/Cargo.toml")"
output="${1:-$root/artifacts/packages}"
tarball="$output/crabpdf-$version-linux-x64.tar.gz"
deb="$output/crabpdf-$version-ubuntu24.04-amd64.deb"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/tar" "$work/deb"
tar -xzf "$tarball" -C "$work/tar"
(
    cd "$work/tar/crabpdf-$version-linux-x64"
    sha256sum -c SHA256SUMS > "$work/tar-hashes.log"
    cmp -s crabpdf "$root/artifacts/crabpdf-linux-x64"
)
dpkg-deb -x "$deb" "$work/deb"
(
    cd "$work/deb/opt/crabpdf"
    sha256sum -c SHA256SUMS > "$work/deb-hashes.log"
    cmp -s crabpdf "$root/artifacts/crabpdf-linux-x64"
)
echo "TAR: $(wc -l < "$work/tar-hashes.log") archivos verificados"
echo "DEB: $(wc -l < "$work/deb-hashes.log") archivos verificados"
dpkg-deb -f "$deb" Version Architecture Depends
sha256sum "$tarball" "$deb"
