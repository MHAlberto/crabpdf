#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
arch="${1:?Expected arm64 or x64}"
case "$arch" in arm64) machine=arm64 ;; x64) machine=x86_64 ;; *) exit 2 ;; esac
test "$(uname -s)" = Darwin && test "$(uname -m)" = "$machine"
tools="$root/native/mac-$arch/format-tools"
if [ -f "$tools/provenance.json" ] && [ -f "$tools/mutool" ] && [ -f "$tools/mobitool" ]; then
    python3 - "$tools" <<'PY'
import hashlib, json, pathlib, sys
tools = pathlib.Path(sys.argv[1])
report = json.loads((tools / "provenance.json").read_text())
assert report["source_mupdf_sha256"] == "98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934"
assert report["source_libmobi_sha256"] == "7abb0ee4c401ef225bbe03b4ebf94d76c81d810fa9b6274f74b4751e10ecd4f0"
for name in ("mutool", "mobitool"):
    assert hashlib.sha256((tools / name).read_bytes()).hexdigest() == report[name + "_sha256"]
PY
    for file in "$tools/mutool" "$tools/mobitool"; do lipo "$file" -verify_arch "$machine"; done
    exit 0
fi
python3 "$root/scripts/Get-UnixFormatSources.py"
sources="$root/native/format-downloads"
build="$root/artifacts/mac-format-build-$arch"
mkdir -p "$build/mupdf" "$build/libmobi" "$build/output" "$tools"
for item in mupdf-1.28.5-source.tar.gz libmobi-source.tar.gz; do
    if tar -tzf "$sources/$item" | grep -Eq '(^/|(^|/)\.\.(/|$))'; then
        echo "Unsafe source archive: $item" >&2; exit 1
    fi
done
tar -xzf "$sources/mupdf-1.28.5-source.tar.gz" -C "$build/mupdf" --strip-components=1
tar -xzf "$sources/libmobi-source.tar.gz" -C "$build/libmobi" --strip-components=1
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}"
make -C "$build/mupdf" build=release HAVE_X11=no HAVE_GLUT=no HAVE_LIBCRYPTO=no \
    USE_SYSTEM_LIBS=no OUT="$build/output/mupdf" -j 3 "$build/output/mupdf/mutool"
cmake -S "$build/libmobi" -B "$build/output/libmobi" -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET" \
    -DTOOLS_STATIC=ON -DUSE_ZLIB=OFF -DUSE_LIBXML2=OFF -DUSE_ENCRYPTION=OFF
cmake --build "$build/output/libmobi" --target mobitool --parallel 2
cp "$build/output/mupdf/mutool" "$tools/mutool"
cp "$build/output/libmobi/tools/mobitool" "$tools/mobitool"
chmod 755 "$tools/mutool" "$tools/mobitool"
cp "$build/mupdf/COPYING" "$tools/MuPDF-COPYING.txt"
cp "$build/libmobi/COPYING" "$tools/Libmobi-COPYING.LESSER.txt"
for file in "$tools/mutool" "$tools/mobitool"; do lipo "$file" -verify_arch "$machine"; done
python3 - "$tools" "$arch" <<'PY'
import hashlib, json, pathlib, sys
tools, arch = pathlib.Path(sys.argv[1]), sys.argv[2]
report = {"architecture": arch, "mupdf": "1.28.5", "libmobi": "0.12",
          "source_mupdf_sha256": "98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934",
          "source_libmobi_sha256": "7abb0ee4c401ef225bbe03b4ebf94d76c81d810fa9b6274f74b4751e10ecd4f0",
          "hash_scope": "Before bundle ad-hoc signing"}
for name in ("mutool", "mobitool"):
    report[name + "_sha256"] = hashlib.sha256((tools / name).read_bytes()).hexdigest()
(tools / "provenance.json").write_text(json.dumps(report, indent=2) + "\n")
PY
