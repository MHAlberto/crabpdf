"""Save Python distribution license files alongside the frozen application."""
from importlib.metadata import distributions
from pathlib import Path
import re
import shutil
import sys
import sysconfig

destination = Path(sys.argv[1])
for distribution in distributions():
    name = re.sub(r"[^a-zA-Z0-9_.-]", "_", distribution.metadata["Name"])
    for source in distribution.files or ():
        if not any(word in source.name.lower() for word in ("license", "copying", "notice")):
            continue
        if not any(".dist-info" in part for part in source.parts):
            continue
        folder = destination / name
        folder.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(distribution.locate_file(source), folder / source.name)
for folder in (Path(sys.base_prefix), Path(sysconfig.get_path("stdlib"))):
    python_license = folder / "LICENSE.txt"
    if python_license.is_file():
        destination.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(python_license, destination / "Python-LICENSE.txt")
        break