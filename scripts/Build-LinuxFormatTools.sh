#!/usr/bin/env bash
# Build the same MuPDF/libmobi versions used by the Windows packages on Linux x86_64.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sources="$root/native/format-downloads"
tools="$root/native/linux-x64/format-tools"
build_root="${CRABPDF_LINUX_FORMAT_BUILD:-$root/artifacts/linux-format-build}"
for name in make cmake gcc g++ sha256sum readelf; do
    command -v "$name" >/dev/null || { echo "Falta $name" >&2; exit 1; }
done
(
    cd "$sources"
    echo '98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934  mupdf-1.28.5-source.tar.gz' | sha256sum -c -
    echo '7abb0ee4c401ef225bbe03b4ebf94d76c81d810fa9b6274f74b4751e10ecd4f0  libmobi-source.tar.gz' | sha256sum -c -
)
mkdir -p "$build_root/mupdf" "$build_root/build/mupdf" "$build_root/libmobi" "$tools"
if [ ! -f "$build_root/mupdf/Makefile" ]; then
    tar -xzf "$sources/mupdf-1.28.5-source.tar.gz" -C "$build_root/mupdf" --strip-components=1
fi
if [ ! -f "$build_root/libmobi/CMakeLists.txt" ]; then
    tar -xzf "$sources/libmobi-source.tar.gz" -C "$build_root/libmobi" --strip-components=1
fi
make -C "$build_root/mupdf" build=release HAVE_X11=no HAVE_GLUT=no USE_SYSTEM_LIBS=no \
    OUT="$build_root/build/mupdf" -j 3 "$build_root/build/mupdf/mutool"
cmake -S "$build_root/libmobi" -B "$build_root/build/libmobi" -DCMAKE_BUILD_TYPE=Release \
    -DTOOLS_STATIC=ON -DUSE_ZLIB=OFF -DUSE_LIBXML2=OFF -DUSE_ENCRYPTION=OFF
cmake --build "$build_root/build/libmobi" --target mobitool --parallel 2
cp "$build_root/build/mupdf/mutool" "$tools/mutool"
cp "$build_root/build/libmobi/tools/mobitool" "$tools/mobitool"
chmod 755 "$tools/mutool" "$tools/mobitool"
cp "$build_root/mupdf/COPYING" "$tools/MuPDF-COPYING.txt"
cp "$build_root/libmobi/COPYING" "$tools/Libmobi-COPYING.LESSER.txt"
for binary in "$tools/mutool" "$tools/mobitool"; do
    readelf -h "$binary" | grep -q 'Machine:.*Advanced Micro Devices X86-64' || {
        echo "No es ELF x86_64: $binary" >&2; exit 1;
    }
done
mutool_hash="$(sha256sum "$tools/mutool" | cut -d ' ' -f 1)"
mobitool_hash="$(sha256sum "$tools/mobitool" | cut -d ' ' -f 1)"
cat > "$tools/provenance.json" <<EOF
{
  "target": "x86_64-unknown-linux-gnu",
  "mupdf": "1.28.5",
  "libmobi": "0.12",
  "source_mupdf_sha256": "98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934",
  "source_libmobi_sha256": "7abb0ee4c401ef225bbe03b4ebf94d76c81d810fa9b6274f74b4751e10ecd4f0",
  "mutool_sha256": "$mutool_hash",
  "mobitool_sha256": "$mobitool_hash"
}
EOF
echo "MuPDF 1.28.5 y libmobi 0.12 listos en $tools"
