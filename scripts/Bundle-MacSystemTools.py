#!/usr/bin/env python3
"""Relocate DjVu/7-Zip and their non-system dylibs, preserving exact source provenance."""
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
target = Path(sys.argv[1]).resolve()
architecture = sys.argv[2]
if root not in target.parents:
    raise SystemExit("Tool destination must be inside the workspace")
sources = root / "artifacts/mac-system-tool-sources" / architecture
sources.mkdir(parents=True, exist_ok=True)
licenses = target / "licenses"
licenses.mkdir()
pending = []
copied = {}
formulae = {}
def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()
def enqueue(source, name=None):
    source = Path(source).resolve(strict=True)
    name = name or source.name
    if name in copied:
        if copied[name] != source:
            raise RuntimeError(f"Conflicting library basename: {name}")
        return name
    copied[name] = source
    shutil.copy2(source, target / name)
    (target / name).chmod(0o755)
    pending.append((source, target / name))
    match = re.search(r"/Cellar/([^/]+)/([^/]+)/", str(source))
    if not match:
        raise RuntimeError(f"Expected a traceable Homebrew component: {source}")
    formulae[match[1]] = match[2]
    return name

for command, name in (("7zz", "7z"), ("ddjvu", "ddjvu"), ("djvutoxml", "djvutoxml")):
    found = shutil.which(command)
    if not found:
        raise RuntimeError(f"Missing build-time tool: {command}")
    enqueue(found, name)
while pending:
    source, destination = pending.pop()
    linked = subprocess.check_output(["otool", "-L", str(source)], text=True)
    for line in linked.splitlines()[1:]:
        dep = line.strip().split(" (")[0]
        if dep.startswith(("/usr/lib/", "/System/Library/")):
            continue
        if source.suffix == ".dylib" and Path(dep).name == source.name:
            continue
        if dep.startswith("@loader_path/"):
            resolved = source.parent / dep.removeprefix("@loader_path/")
        elif dep.startswith("/"):
            resolved = Path(dep)
        else:
            raise RuntimeError(f"Unresolved dependency: {source}: {dep}")
        name = enqueue(resolved)
        subprocess.run(["install_name_tool", "-change", dep, "@loader_path/" + name, str(destination)], check=True)
    if source.suffix == ".dylib":
        subprocess.run(["install_name_tool", "-id", "@loader_path/" + destination.name, str(destination)], check=True)

records = []
for formula, installed in sorted(formulae.items()):
    info = json.loads(subprocess.check_output(["brew", "info", "--json=v2", formula], text=True))["formulae"][0]
    expected = info["versions"]["stable"] + ("_" + str(info["revision"]) if info["revision"] else "")
    if installed != expected:
        raise RuntimeError(f"Source metadata differs from installed {formula}: {installed} vs {expected}")
    stable = info["urls"]["stable"]
    # Homebrew handles upstream mirrors and verifies the formula's source hash.
    subprocess.run(["brew", "fetch", "--build-from-source", formula], check=True)
    cached = Path(subprocess.check_output(["brew", "--cache", "--build-from-source", formula], text=True).strip())
    if sha(cached) != stable["checksum"]:
        raise RuntimeError(f"Source hash mismatch: {formula}")
    filename = f"{formula}-{installed}-{cached.name.split('--')[-1]}"
    archive = sources / filename
    shutil.copy2(cached, archive)
    (sources / f"{formula}.json").write_text(json.dumps(info, indent=2) + "\n")
    cellar = Path(subprocess.check_output(["brew", "--cellar", formula], text=True).strip()) / installed
    recipe = cellar / ".brew" / f"{formula}.rb"
    if recipe.is_file():
        shutil.copy2(recipe, sources / f"{formula}.rb")
    license_dir = licenses / formula
    license_dir.mkdir()
    total = 0
    if tarfile.is_tarfile(archive):
        with tarfile.open(archive) as package:
            for index, member in enumerate(package.getmembers()):
                leaf = Path(member.name).name
                if not member.isfile() or not re.search(r"LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE", leaf, re.I):
                    continue
                if member.size > 1024 * 1024:
                    raise RuntimeError("Oversized license text")
                total += member.size
                if total > 10 * 1024 * 1024:
                    raise RuntimeError("License text bound exceeded")
                with package.extractfile(member) as stream:
                    (license_dir / f"{index}-{leaf}").write_bytes(stream.read())
    # sevenzip's source is a .tar.xz on Homebrew. Retain its complete archive and
    # recipe even if an upstream source format changes in a future formula.
    records.append({"formula": formula, "version": installed, "license": info["license"],
                    "source_url": stable["url"], "source_file": filename,
                    "source_sha256": sha(archive), "recipe": f"{formula}.rb"})
report = {"components": records, "bundled_files": sorted(copied),
          "note": "Library paths relocated with install_name_tool; bundle hashes are recorded after ad-hoc signing."}
(target / "system-tools-provenance.json").write_text(json.dumps(report, indent=2) + "\n")
(sources / "provenance.json").write_text(json.dumps(report, indent=2) + "\n")
version = re.search(r'^version = "([^"]+)"', (root / "Cargo.toml").read_text(), re.M)[1]
output = root / "artifacts/packages" / f"crabpdf-{version}-mac-{architecture}-system-tool-sources.zip"
output.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as archive:
    for file in sorted(sources.iterdir()):
        archive.write(file, f"{output.stem}/{file.name}")
print(f"Bundled {len(copied)} system tools/libraries with {len(records)} corresponding source archives")
