#!/usr/bin/env bash
# Run in a fresh Ubuntu 24.04 container, not on a user's installed system.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
version="$(awk -F '"' '/^version = "/ { print $2; exit }' Cargo.toml)"
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
    "./artifacts/packages/crabpdf-$version-ubuntu24.04-amd64.deb" \
    python3 xvfb xauth dbus-x11 mesa-utils fonts-dejavu-core binutils
test "$(dpkg-query -W -f '${Version}' crabpdf)" = "$version"
test -f /usr/share/applications/crabpdf.desktop
cd /opt/crabpdf
sha256sum -c SHA256SUMS > "$root/artifacts/linux-installed-hashes.log"
./crabpdf --version
for binary in crabpdf libpdfium.so format-tools/mutool format-tools/mobitool; do
    linked="$(ldd "$binary")"
    printf '%s\n' "$linked"
    if grep -q 'not found' <<< "$linked"; then exit 1; fi
done
cp "$root/target/linux-bookworm/release/examples/package_smoke" /opt/crabpdf/package_smoke
cd "$root"
python3 scripts/New-UnixFormatSamples.py
unset RUST_READER_PDFIUM CRABPDF_FORMAT_TOOLS
export LIBGL_ALWAYS_SOFTWARE=1
export XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
for sample in book.pdf book.epub book.djvu comic.cbz; do
    output="$root/artifacts/validation/linux-x64/${sample//./-}"
    CRABPDF_DATA_DIR="$output/data" xvfb-run -a dbus-run-session -- \
        /opt/crabpdf/package_smoke "$output" "$root/artifacts/unix-format-samples/$sample"
done
python3 - "$version" <<'PY'
import hashlib, json, os, pathlib, subprocess, sys
root = pathlib.Path.cwd()
version = sys.argv[1]
assets = []
for path in sorted((root / "artifacts/packages").iterdir()):
    if path.name.endswith((".deb", ".tar.gz")):
        assets.append({"name": path.name, "bytes": path.stat().st_size,
                       "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
report = {"version": version, "target": "x86_64-unknown-linux-gnu",
          "build_commit": os.environ["CRABPDF_BUILD_COMMIT"], "application_source_tag": "v" + version,
          "builder": "Rust 1.96.0 on Debian 12 / glibc 2.36", "installed_on": "Ubuntu 24.04 clean container",
          "package_hashes_verified": True, "dependency_resolution_verified": True,
          "native_gui_formats_passed": ["PDF", "EPUB", "DjVu", "CBZ"],
          "gui_environment": "Xvfb and Mesa software rendering", "assets": assets}
(root / "artifacts/packages" / f"crabpdf-{version}-linux-validation.json").write_text(json.dumps(report, indent=2) + "\n")
PY
