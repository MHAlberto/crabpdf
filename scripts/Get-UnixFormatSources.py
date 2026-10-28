#!/usr/bin/env python3
"""Recover the exact converter sources already shipped with release 0.4.2."""
import hashlib
import shutil
import urllib.request
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
output = root / "native/format-downloads"
output.mkdir(parents=True, exist_ok=True)
sources = {
    "mupdf-1.28.5-source.tar.gz": "98a5c10cda20c3992cdf76ff6b2a1149c32bd79cc796d3f703230b1185b7e934",
    "libmobi-source.tar.gz": "7abb0ee4c401ef225bbe03b4ebf94d76c81d810fa9b6274f74b4751e10ecd4f0",
}
def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

if all((output / name).is_file() and digest(output / name) == sha for name, sha in sources.items()):
    print("Pinned converter sources already verified")
    raise SystemExit(0)
archive = output / "crabpdf-0.4.2-format-tools-sources.zip"
if not archive.exists():
    request = urllib.request.Request(
        "https://github.com/MHAlberto/crabpdf/releases/download/v0.4.2/crabpdf-0.4.2-format-tools-sources.zip",
        headers={"User-Agent": "CrabPDF-source-packaging"},
    )
    with urllib.request.urlopen(request, timeout=180) as response, archive.open("xb") as target:
        total = 0
        while chunk := response.read(1024 * 1024):
            total += len(chunk)
            if total > 100 * 1024 * 1024:
                raise RuntimeError("Source archive exceeds its bound")
            target.write(chunk)
if digest(archive) != "b42eb877fbcf8bcaf0f905ec7dc5b02bcc3feb1309280ab27739725f6cd472de":
    raise RuntimeError("Release source archive hash mismatch")
with zipfile.ZipFile(archive) as package:
    for name, sha in sources.items():
        entries = [entry for entry in package.infolist() if Path(entry.filename).name == name]
        if len(entries) != 1 or entries[0].file_size > 120 * 1024 * 1024:
            raise RuntimeError(f"Invalid source entry: {name}")
        with package.open(entries[0]) as source, (output / name).open("wb") as target:
            shutil.copyfileobj(source, target)
        if digest(output / name) != sha:
            raise RuntimeError(f"Source hash mismatch: {name}")
        print(f"Verified {name}")
