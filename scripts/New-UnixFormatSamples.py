#!/usr/bin/env python3
"""Generate original, small cross-platform package fixtures without extra modules."""
import shutil
import struct
import subprocess
import zlib
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
out = root / "artifacts/unix-format-samples"
out.mkdir(parents=True, exist_ok=True)
shutil.copy2(root / "tests/fixtures/features-sample.pdf", out / "book.pdf")
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
pixels = b"".join(b"\0" + bytes((70, 135 + row, 195)) * 64 for row in range(64))
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 64, 64, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")
(out / "picture.png").write_bytes(png)
with zipfile.ZipFile(out / "comic.cbz", "w") as archive:
    archive.writestr("1.png", png)
    archive.writestr("2.png", png)
with zipfile.ZipFile(out / "book.epub", "w") as archive:
    archive.writestr("mimetype", "application/epub+zip", compress_type=zipfile.ZIP_STORED)
    archive.writestr("META-INF/container.xml", '<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>')
    archive.writestr("content.opf", '<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>CrabPDF package fixture</dc:title><dc:identifier id="id">crabpdf-package-test</dc:identifier><dc:language>es</dc:language></metadata><manifest><item id="one" href="one.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="one"/></spine></package>')
    archive.writestr("one.xhtml", '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>CrabPDF</title></head><body><h1>CrabPDF package fixture</h1><p>Texto original: cap\u00edtulo y lectura.</p></body></html>')
(out / "picture.ppm").write_bytes(b"P6\n64 64\n255\n" + bytes((70, 140, 195)) * 64 * 64)
subprocess.run(["c44", str(out / "picture.ppm"), str(out / "book.djvu")], check=True)
print(f"Original PDF, EPUB, PNG, CBZ and DjVu fixtures: {out}")
