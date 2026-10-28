#!/usr/bin/env python3
"""Build, sign ad hoc, exercise and verify a macOS app, ZIP and DMG."""
import argparse
import hashlib
import json
import os
import plistlib
import re
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("architecture", choices=("arm64", "x64"))
args = parser.parse_args()
machine = "arm64" if args.architecture == "arm64" else "x86_64"
target = f"{machine}-apple-darwin"
version = re.search(r'^version = "([^"]+)"', (root / "Cargo.toml").read_text(), re.M)[1]
out = root / "artifacts/packages"
name = f"crabpdf-{version}-mac-{args.architecture}"
folder = root / "artifacts/mac-bundles" / name
if folder.exists():
    raise SystemExit("Use a fresh bundle directory")
app = folder / "CrabPDF.app"
executables = app / "Contents/MacOS"
resources = app / "Contents/Resources"
executables.mkdir(parents=True)
resources.mkdir()
out.mkdir(parents=True, exist_ok=True)
native = root / "native" / f"mac-{args.architecture}"
sdk = native / f"pdfium-7881-mac-{args.architecture}"
release = root / "target" / target / "release"
def run(*command, **options):
    return subprocess.run([str(x) for x in command], check=True, **options)
def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

pdfium = json.loads((native / "pdfium-provenance.json").read_text(encoding="utf-8-sig"))
archive = native / f"pdfium-mac-{args.architecture}.tgz"
if pdfium["build"] != "7881" or pdfium["archive_sha256"] != digest(archive):
    raise SystemExit("PDFium provenance mismatch")
sdk_libraries = list(sdk.rglob("libpdfium.dylib"))
if len(sdk_libraries) != 1 or digest(sdk_libraries[0]) != digest(native / "libpdfium.dylib"):
    raise SystemExit("PDFium library differs from verified SDK")
shutil.copy2(release / "crabpdf", executables / "crabpdf")
shutil.copy2(native / "libpdfium.dylib", executables / "libpdfium.dylib")
shutil.copytree(native / "format-tools", executables / "format-tools")
for tool, alternatives in {"7z": ("7zz", "7z"), "ddjvu": ("ddjvu",), "djvutoxml": ("djvutoxml",)}.items():
    # Finder does not necessarily inherit Homebrew's PATH. These optional system
    # tools remain external; their binaries and libraries are not redistributed.
    wrapper = "#!/bin/sh\nset -eu\n"
    for prefix in ("/opt/homebrew/bin", "/usr/local/bin"):
        for alternative in alternatives:
            path = f"{prefix}/{alternative}"
            wrapper += f'if [ -x "{path}" ]; then exec "{path}" "$@"; fi\n'
    wrapper += 'echo "Install optional DjVu/archive tools: brew install djvulibre sevenzip" >&2\nexit 127\n'
    (executables / "format-tools" / tool).write_text(wrapper)
    (executables / "format-tools" / tool).chmod(0o755)
for source in ("LICENSE", "docs/THIRD_PARTY_NOTICES.md", "docs/FORMATS_AND_ANNOTATIONS.md", "assets/crabpdf-logo.svg"):
    shutil.copy2(root / source, resources / Path(source).name)
shutil.copytree(root / "docs/licenses-upstream", resources / "licenses-upstream")
shutil.copytree(root / "artifacts/mac-rust-licenses", resources / "rust-licenses")
shutil.copytree(sdk / "licenses", resources / "pdfium-licenses")
shutil.copy2(sdk / "LICENSE", resources / "pdfium-licenses/LICENSE")
shutil.copy2(native / "pdfium-provenance.json", resources / "pdfium-licenses/provenance.json")
icons = folder / "CrabPDF.iconset"
icons.mkdir()
png = next((release / "build").glob("crabpdf-*/out/crabpdf.png"))
for size in (16, 32, 128, 256):
    run("sips", "-z", size, size, png, "--out", icons / f"icon_{size}x{size}.png", stdout=subprocess.DEVNULL)
    if size <= 128:
        run("sips", "-z", size * 2, size * 2, png, "--out", icons / f"icon_{size}x{size}@2x.png", stdout=subprocess.DEVNULL)
run("iconutil", "-c", "icns", icons, "-o", resources / "CrabPDF.icns")
shutil.rmtree(icons)
mach_files = [executables / "crabpdf", executables / "libpdfium.dylib",
              executables / "format-tools/mutool", executables / "format-tools/mobitool"]
minimum = (13, 0)
dependencies = {}
for file in mach_files:
    run("lipo", "-verify_arch", machine, file)
    load = subprocess.check_output(["otool", "-l", str(file)], text=True)
    for number in re.findall(r"\bminos\s+(\d+(?:\.\d+)+)", load):
        minimum = max(minimum, tuple(map(int, number.split("."))))
    old = re.search(r"cmd LC_VERSION_MIN_MACOSX.*?\bversion\s+(\d+(?:\.\d+)+)", load, re.S)
    if old:
        minimum = max(minimum, tuple(map(int, old[1].split("."))))
    linked = subprocess.check_output(["otool", "-L", str(file)], text=True)
    deps = [line.strip().split(" (")[0] for line in linked.splitlines()[1:]]
    for dep in deps:
        if dep.startswith(("/usr/lib/", "/System/Library/")):
            continue
        if file.name == "libpdfium.dylib" and dep.endswith("/libpdfium.dylib"):
            continue
        raise RuntimeError(f"Unbundled dependency: {file.name}: {dep}")
    dependencies[str(file.relative_to(app))] = deps
    run("codesign", "--force", "--sign", "-", "--timestamp=none", file)
plist = {"CFBundleExecutable": "crabpdf", "CFBundleIdentifier": "io.github.mhalberto.crabpdf",
         "CFBundleName": "CrabPDF", "CFBundleDisplayName": "CrabPDF", "CFBundlePackageType": "APPL",
         "CFBundleShortVersionString": version, "CFBundleVersion": version, "CFBundleIconFile": "CrabPDF.icns",
         "LSMinimumSystemVersion": ".".join(map(str, minimum)), "NSHighResolutionCapable": True}
(app / "Contents/Info.plist").write_bytes(plistlib.dumps(plist))
# Test automatic library/tool discovery from the actual application directory.
helper = executables / "package_smoke"
shutil.copy2(release / "examples/package_smoke", helper)
run("codesign", "--force", "--sign", "-", "--timestamp=none", helper)
validation_root = root / "artifacts/validation" / name
for sample in ("book.pdf", "book.epub", "book.djvu", "comic.cbz"):
    output = validation_root / sample.replace(".", "-")
    env = os.environ.copy()
    env.pop("RUST_READER_PDFIUM", None)
    env.pop("CRABPDF_FORMAT_TOOLS", None)
    env["CRABPDF_DATA_DIR"] = str(output / "data")
    run(helper, output, root / "artifacts/unix-format-samples" / sample, env=env, timeout=90)
helper.unlink()
run("codesign", "--force", "--sign", "-", "--timestamp=none", "--deep", app)
run("codesign", "--verify", "--deep", "--strict", app)
(folder / "LEEME.md").write_text(f"""# CrabPDF {version} para macOS {args.architecture}

Arrastra CrabPDF.app a Aplicaciones. El paquete incluye PDFium, MuPDF y libmobi.
PDF y EPUB funcionan sin instalar conversores externos. Para DjVu, comics y
archivos comprimidos: `brew install djvulibre sevenzip`. Estos componentes
opcionales se instalan desde Homebrew y no se incluyen en este paquete.

Version minima declarada por los binarios: {plist['LSMinimumSystemVersion']}.
Compilado y probado en macOS 15. Firma ad hoc, sin Developer ID ni notarizacion.
macOS puede bloquear una descarga no notarizada; consulta la documentacion de
Apple: https://support.apple.com/102445 . No se cambia Gatekeeper.

Las licencias y la procedencia se conservan en Contents/Resources y format-tools.
""")
checksums = []
for file in sorted(folder.rglob("*")):
    if file.is_file():
        checksums.append(f"{digest(file)}  {file.relative_to(folder).as_posix()}")
(folder / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
(folder / "Applications").symlink_to("/Applications", target_is_directory=True)
zip_path = out / f"{name}.zip"
dmg_path = out / f"{name}.dmg"
run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", folder, zip_path)
run("hdiutil", "create", "-volname", f"CrabPDF {version} {args.architecture}", "-srcfolder", folder, "-ov", "-format", "UDZO", dmg_path)
run("hdiutil", "verify", dmg_path)
mount = root / "artifacts/mac-mounted" / name
mount.mkdir(parents=True)
run("hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, dmg_path)
try:
    run("codesign", "--verify", "--deep", "--strict", mount / "CrabPDF.app")
    run("shasum", "-a", "256", "-c", "SHA256SUMS", cwd=mount, stdout=subprocess.DEVNULL)
finally:
    run("hdiutil", "detach", mount)
report = {"version": version, "architecture": args.architecture, "target": target,
          "build_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
          "application_source_tag": f"v{version}", "minimum_macos": plist["LSMinimumSystemVersion"],
          "signing": "ad-hoc", "notarized": False, "dependencies": dependencies,
          "native_gui_formats_passed": ["PDF", "EPUB", "DjVu", "CBZ"],
          "dmg_mounted_and_hashes_verified": True,
          "optional_system_tools": ["Homebrew djvulibre", "Homebrew sevenzip"],
          "assets": [{"name": file.name, "bytes": file.stat().st_size, "sha256": digest(file)} for file in (zip_path, dmg_path)]}
(out / f"{name}-validation.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
