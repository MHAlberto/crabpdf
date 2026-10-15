#!/usr/bin/env bash
# Run inside Ubuntu 24.04 (or a compatible Linux container) after building with Rust 1.96.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(awk -F '"' '/^version = "/ { print $2; exit }' "$root/Cargo.toml")"
binary="$root/artifacts/crabpdf-linux-x64"
native="$root/native/linux-x64"
bundle="$native/pdfium-7881-linux-x64"
format_tools="$native/format-tools"
licenses="$root/artifacts/linux-rust-licenses"
output="$root/artifacts/packages"

for command in readelf dpkg-deb sha256sum tar; do
    command -v "$command" >/dev/null || { echo "Falta $command" >&2; exit 1; }
done
test -f "$binary" && test -f "$native/libpdfium.so" && test -d "$licenses" || {
    echo 'Faltan el binario Linux, PDFium o los textos de licencia Rust (Export-RustLicenses.py).' >&2
    exit 1
}
test -f "$format_tools/mutool" && test -f "$format_tools/mobitool" &&
    test -f "$format_tools/provenance.json" || {
    echo 'Faltan los decodificadores Linux (Build-LinuxFormatTools.sh).' >&2; exit 1;
}
test "$(readelf -h "$binary" | awk '/Machine:/ { print $2 }')" = Advanced || {
    echo 'El ejecutable no es ELF x86_64.' >&2; exit 1;
}
test "$(readelf -h "$native/libpdfium.so" | awk '/Machine:/ { print $2 }')" = Advanced || {
    echo 'PDFium no es ELF x86_64.' >&2; exit 1;
}
grep -Eq '"build": *"7881"' "$native/pdfium-provenance.json"
expected="$(sed -n 's/.*"archive_sha256": *"\([^"]*\)".*/\1/p' "$native/pdfium-provenance.json")"
actual="$(sha256sum "$native/pdfium-linux-x64.tgz" | cut -d ' ' -f 1)"
test -n "$expected" && test "$expected" = "$actual" &&
    cmp -s "$native/libpdfium.so" "$bundle/lib/libpdfium.so" || {
    echo 'PDFium no coincide con el archivo verificado.' >&2; exit 1;
}
for tool in mutool mobitool; do
    test "$(readelf -h "$format_tools/$tool" | awk '/Machine:/ { print $2 }')" = Advanced || {
        echo "Arquitectura incorrecta: $tool" >&2; exit 1;
    }
    expected="$(sed -n "s/.*\"${tool}_sha256\": *\"\([^\"]*\)\".*/\1/p" "$format_tools/provenance.json")"
    actual="$(sha256sum "$format_tools/$tool" | cut -d ' ' -f 1)"
    test -n "$expected" && test "$expected" = "$actual" || {
        echo "Hash incorrecto: $tool" >&2; exit 1;
    }
done

mkdir -p "$output"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
name="crabpdf-$version-linux-x64"
app="$work/$name"
mkdir -p "$app/pdfium-licenses" "$app/rust-licenses"
mkdir -p "$app/format-tools"
cp "$binary" "$app/crabpdf"
chmod 755 "$app/crabpdf"
cp "$native/libpdfium.so" "$app/libpdfium.so"
cp "$root/LICENSE" "$app/LICENSE"
cp "$root/crabpdf-logo.svg" "$app/crabpdf-logo.svg"
cp "$root/docs/THIRD_PARTY_NOTICES.md" "$app/THIRD_PARTY_NOTICES.md"
cp "$root/docs/dependencies.csv" "$app/dependencies.csv"
cp "$root/docs/FORMATS_AND_ANNOTATIONS.md" "$app/FORMATS_AND_ANNOTATIONS.md"
cp -a "$licenses/." "$app/rust-licenses/"
cp -a "$root/docs/licenses-upstream" "$app/licenses-upstream"
cp "$bundle/LICENSE" "$app/pdfium-licenses/LICENSE"
cp -a "$bundle/licenses" "$app/pdfium-licenses/licenses"
cp "$native/pdfium-provenance.json" "$app/pdfium-licenses/provenance.json"
cp -a "$format_tools/." "$app/format-tools/"
cat > "$app/INSTALL-LINUX.md" <<'EOF'
# CrabPDF 0.3.3 para Linux x86_64

Ejecutar `./crabpdf` en un escritorio X11 o Wayland. Conservar `libpdfium.so` y
`format-tools/` junto al ejecutable. El tar incluye MuPDF 1.28.5 y libmobi 0.12,
pero requiere bibliotecas gráficas y OpenSSL 3 del sistema. Para DjVu, CHM,
cómics y otros archivos comprimidos, instalar las herramientas del sistema.
En Ubuntu 24.04/Mint 22:

```sh
sudo apt update
sudo apt install djvulibre-bin 7zip
```

El paquete `.deb` declara estas dependencias y las resuelve al instalarse con `apt`.
Otros sistemas pueden usar nombres distintos para `ddjvu`, `djvutoxml` y `7z`.
Los documentos se procesan localmente.
Esta es una versión de desarrollo 0.3.3, no la paridad 1.0 de SumatraPDF.
EOF
(
    cd "$app"
    find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
)
tar -czf "$output/$name.tar.gz" -C "$work" "$name"

deb="$work/deb"
mkdir -p "$deb/DEBIAN" "$deb/opt/crabpdf" "$deb/usr/bin" \
    "$deb/usr/share/applications" "$deb/usr/share/icons/hicolor/scalable/apps"
cp -a "$app/." "$deb/opt/crabpdf/"
cat > "$deb/usr/bin/crabpdf" <<'EOF'
#!/bin/sh
exec /opt/crabpdf/crabpdf "$@"
EOF
chmod 755 "$deb/usr/bin/crabpdf"
cp "$root/crabpdf-logo.svg" "$deb/usr/share/icons/hicolor/scalable/apps/crabpdf.svg"
cat > "$deb/usr/share/applications/crabpdf.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=CrabPDF
Comment=Lector local de documentos
Exec=crabpdf %F
Icon=crabpdf
Categories=Office;Viewer;
MimeType=application/pdf;application/epub+zip;image/vnd.djvu;application/x-cbr;application/x-cbz;
Terminal=false
EOF
cat > "$deb/DEBIAN/control" <<EOF
Package: crabpdf
Version: $version
Architecture: amd64
Maintainer: CrabPDF contributors <noreply@github.com>
Depends: libc6 (>= 2.36), libssl3t64, libegl1, libgl1, libgl1-mesa-dri, libx11-6, libx11-xcb1, libxcb1, libxcursor1, libxi6, libxrandr2, libxinerama1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0, libdbus-1-3, djvulibre-bin, 7zip
Section: graphics
Priority: optional
Homepage: https://github.com/MHAlberto/crabpdf
Description: Local document reader built with Rust and PDFium
 CrabPDF reads PDFs and imports several other document formats locally.
 This is a development release toward SumatraPDF feature parity.
EOF
dpkg-deb --build --root-owner-group "$deb" "$output/crabpdf-$version-ubuntu24.04-amd64.deb" >/dev/null
sha256sum "$output/$name.tar.gz" "$output/crabpdf-$version-ubuntu24.04-amd64.deb"
