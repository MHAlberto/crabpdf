"""Exercise the distributed OCR worker with an empty profile and a known equation."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory
from zipfile import ZipFile


def test(archive, cached_model=None):
    with TemporaryDirectory(prefix="crabpdf-ocr-smoke-") as folder:
        root = Path(folder)
        package = root / "package"
        with ZipFile(archive) as bundle:
            for item in bundle.infolist():
                file = Path(bundle.extract(item, package))
                if sys.platform == "linux":
                    file.chmod((item.external_attr >> 16) & 0o777)
        executable = package / ("ocr.exe" if sys.platform == "win32" else "ocr")
        env = dict(os.environ, CRABPDF_EXTENSION_DATA=str(root / "data"),
                   CRABPDF_EXTENSION_SCRATCH=str(root / "scratch"))
        model = root / "data" / "rapidlatex-v0"
        model.mkdir(parents=True)
        if cached_model:
            shutil.copytree(cached_model, model, dirs_exist_ok=True)
        (model / "encoder.onnx.download").write_bytes(b"Interrumpido")
        (root / "scratch").mkdir()
        request = root / "request.json"
        response = root / "response.json"
        equation = Path(__file__).with_name("equation.png").resolve()

        def run(command, request_id, **extra):
            request.write_text(json.dumps({
                "api_version": 1, "request_id": request_id, "command": command,
                "page": 1, "region": {"left": 0, "top": 0, "right": 1, "bottom": 1}, **extra,
            }), encoding="utf-8")
            response.unlink(missing_ok=True)
            subprocess.run([executable, "--crabpdf-request", request, "--crabpdf-response", response],
                           cwd=package, env=env, check=True, timeout=420)
            result = json.loads(response.read_text(encoding="utf-8"))
            assert result["request_id"] == request_id and "Ok" in result["result"], result
            return result["result"]["Ok"]["text"]

        run("prepare", 1)
        assert not list((root / "scratch").iterdir())
        assert not list(model.glob("*.download"))
        assert "178.95 MB" in run("model-size", 2)
        recognized = run("recognize", 3, image={"png_path": str(equation)})
        assert recognized == r"{\frac{x^{2}}{a^{2}}}-{\frac{y^{2}}{b^{2}}}=1", recognized
        assert "178.95 MB" in run("remove-model", 4)
        assert not list(model.iterdir())
        assert "0.00 MB" in run("model-size", 5)
        print("OCR: pesos verificados, inferencia LaTeX y limpieza.", recognized)


if __name__ == "__main__":
    test(Path(sys.argv[1]), Path(sys.argv[2]) if len(sys.argv) > 2 else None)
