"""Build a native OCR package from the pinned Python environment."""
import subprocess
from pathlib import Path
import shutil
import sys
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parents[2]


def build(target):
    expected = {"win32": "x86_64-pc-windows-msvc", "linux": "x86_64-unknown-linux-gnu"}
    if expected.get(sys.platform) != target:
        raise ValueError("El entorno de compilación del OCR no coincide con el destino.")
    package_root = ROOT / "artifacts" / "extension-build" / f"math-ocr-{target}"
    package_root.mkdir(parents=True, exist_ok=True)
    source = ROOT / "extensions" / "math-ocr"
    subprocess.run([
        sys.executable, "-m", "PyInstaller", "--noconfirm", "--onedir", "--name", "ocr",
        "--distpath", package_root / "dist", "--workpath", package_root / "work",
        "--specpath", package_root, "--collect-data", "rapid_latex_ocr",
        "--collect-binaries", "onnxruntime", "--exclude-module", "tkinter", source / "ocr.py",
    ], check=True)
    folder = package_root / "dist" / "ocr"
    executable = "ocr.exe" if sys.platform == "win32" else "ocr"
    manifest = (source / "extension.toml").read_text(encoding="utf-8")
    (folder / "extension.toml").write_text(
        manifest.replace("@TARGET@", target).replace("@EXE@", executable), encoding="utf-8",
    )
    with (folder / "DEPENDENCIAS.txt").open("wb") as dependencies:
        subprocess.run([sys.executable, "-m", "pip", "list", "--format=freeze"],
                       stdout=dependencies, check=True)
    shutil.copyfile(source / "SOURCES.txt", folder / "SOURCES.txt")
    shutil.copyfile(ROOT / "LICENSE", folder / "LICENSE")
    subprocess.run([sys.executable, source / "licenses.py", folder / "LICENSES"], check=True)
    archive = ROOT / "artifacts" / "site-extensions" / f"math-ocr-{target}.zip"
    archive.parent.mkdir(parents=True, exist_ok=True)
    # Follow PyInstaller's internal file symlinks and keep the target file permissions.
    with ZipFile(archive, "w", compression=ZIP_DEFLATED) as bundle:
        for path in sorted(folder.rglob("*")):
            if path.is_file():
                bundle.write(path, path.relative_to(folder).as_posix())
    print(archive)


if __name__ == "__main__":
    build(sys.argv[1])
