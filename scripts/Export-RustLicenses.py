#!/usr/bin/env python3
"""Copy original Cargo dependency license texts into a portable bundle."""

import json
import re
import shutil
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
destination = root / "artifacts" / "linux-rust-licenses"
metadata = json.loads(
    subprocess.check_output(
        [
            "cargo", "metadata", "--locked", "--offline", "--format-version", "1",
            "--filter-platform", "x86_64-unknown-linux-gnu",
        ],
        cwd=root,
    )
)
destination.mkdir(parents=True, exist_ok=True)
records = []
resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
pattern = re.compile(r"LICENSE|LICENCE|COPYING|NOTICE|(^|[-_.])(OFL|UFL)([-_.]|$)", re.I)
for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
    if package["id"] not in resolved or package["id"] in metadata["workspace_members"]:
        continue
    source = Path(package["manifest_path"]).parent
    folder = destination / f"{package['name']}-{package['version']}"
    count = 0
    for path in source.rglob("*"):
        if not path.is_file():
            continue
        if not (
            pattern.search(path.name)
            or package["name"] == "epaint_default_fonts" and path.suffix == ".txt"
            or package.get("license_file") and path == source / package["license_file"]
        ):
            continue
        target = folder / path.relative_to(source)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        count += 1
    records.append(
        {
            "name": package["name"],
            "version": package["version"],
            "license": package.get("license"),
            "repository": package.get("repository"),
            "text_count": count,
        }
    )
(destination / "index.json").write_text(
    json.dumps(
        {
            "scope": "Cargo.lock: Linux x86_64 normal, build and development dependencies",
            "packages": records,
        },
        ensure_ascii=False,
        indent=2,
    ),
    encoding="utf-8",
)
print(f"Exported {len(records)} dependency records to {destination}")
