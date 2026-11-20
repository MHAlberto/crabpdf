"""Independent CrabPDF native protocol adapter for RapidLaTeXOCR."""
import argparse
import hashlib
import json
import os
import stat
from pathlib import Path
import urllib.request

BASE = "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/"
MODELS = {
    "image_resizer.onnx": (38967751, "e0b075c39700f64d50400f39c8fc186bbb3b5d84d31864008313f376603aca9d"),
    "encoder.onnx": (89008136, "01bf5dc25539ca0cd5b1bd29296ea495977a6ba5f629dc4178277809d26e5e7d"),
    "decoder.onnx": (50952726, "bd695497bf1b22279b7626f5916c79226e1e244c84355f8da7edfd2d921d0072"),
    "tokenizer.json": (24174, "1dc27b18d6a518d0d5ff3f4bb7bd98521fe80ad39e5b2a246d4109f1bb9d5019"),
}


RECEIPT = "verified-model.json"


def stamp(path, size):
    try:
        data = path.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(data.st_mode) or data.st_size != size:
        return None
    return [data.st_dev, data.st_ino, data.st_size, data.st_mtime_ns, data.st_ctime_ns]


def checked_stamp(path, size, digest, cached=None):
    before = stamp(path, size)
    if before is None:
        return None
    if cached == [digest, before]:
        return before
    with path.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual == digest and stamp(path, size) == before:
        return before
    return None


def read_receipt(folder):
    try:
        with (folder / RECEIPT).open("rb") as stream:
            data = stream.read(8193)
        if len(data) <= 8192:
            receipt = json.loads(data)
            if isinstance(receipt, dict):
                return receipt
    except (OSError, ValueError):
        pass
    return {}


def save_receipt(folder, receipt):
    temporary = folder / (RECEIPT + ".tmp")
    try:
        temporary.write_text(json.dumps(receipt), encoding="utf-8")
        temporary.replace(folder / RECEIPT)
    except OSError:
        # Cached verification is optional when the model directory is read-only.
        try:
            temporary.unlink(missing_ok=True)
        except OSError:
            pass


def model_ready(folder):
    cached = read_receipt(folder)
    receipt = {}
    for name, (size, digest) in MODELS.items():
        found = checked_stamp(folder / name, size, digest, cached.get(name))
        if found is None:
            return False
        receipt[name] = [digest, found]
    if receipt != cached:
        save_receipt(folder, receipt)
    return True


def clean_partials(folder):
    for name in MODELS:
        (folder / (name + ".download")).unlink(missing_ok=True)
    (folder / (RECEIPT + ".tmp")).unlink(missing_ok=True)


def remove_model(folder):
    (folder / RECEIPT).unlink(missing_ok=True)
    removed = 0
    for name in MODELS:
        path = folder / name
        try:
            size = path.lstat().st_size
            path.unlink()
        except FileNotFoundError:
            continue
        removed += size
    return removed


def prepare(folder, scratch):
    folder.mkdir(parents=True, exist_ok=True)
    cached = read_receipt(folder)
    receipt = {}
    for name, (size, digest) in MODELS.items():
        path = folder / name
        if (found := checked_stamp(path, size, digest, cached.get(name))) is not None:
            receipt[name] = [digest, found]
            continue
        part = scratch / (name + ".download")
        try:
            request = urllib.request.Request(BASE + name, headers={"User-Agent": "CrabPDF-OCR"})
            with urllib.request.urlopen(request, timeout=45) as stream, part.open("xb") as output:
                remaining = size
                while remaining:
                    chunk = stream.read(min(remaining, 1024 * 1024))
                    if not chunk:
                        raise ValueError("La descarga del modelo quedó incompleta.")
                    output.write(chunk)
                    remaining -= len(chunk)
                if stream.read(1):
                    raise ValueError("El modelo supera el tamaño publicado.")
            if checked_stamp(part, size, digest) is None:
                raise ValueError(f"El SHA-256 de {name} no coincide.")
            part.replace(path)
            receipt[name] = [digest, stamp(path, size)]
        finally:
            part.unlink(missing_ok=True)
    if receipt != cached:
        save_receipt(folder, receipt)


def run(request):
    folder = Path(os.environ["CRABPDF_EXTENSION_DATA"]) / "rapidlatex-v0"
    clean_partials(folder)
    if request["command"] == "remove-model":
        removed = remove_model(folder)
        return {"kind": "text", "text": f"Modelo eliminado. Espacio liberado: {removed / 1_000_000:.2f} MB."}
    if request["command"] == "model-size":
        used = sum((folder / name).stat().st_size for name in MODELS if (folder / name).is_file())
        return {"kind": "text", "text": f"Espacio usado por el modelo: {used / 1_000_000:.2f} MB."}
    if request["command"] == "prepare":
        scratch = Path(os.environ["CRABPDF_EXTENSION_SCRATCH"]) if "CRABPDF_EXTENSION_SCRATCH" in os.environ else folder
        prepare(folder, scratch)
        return {"kind": "text", "text": "Modelo descargado y verificado. Selecciona una ecuación."}
    if request["command"] != "recognize" or not request.get("image"):
        raise ValueError("Selecciona un recorte con una ecuación.")
    if not model_ready(folder):
        raise ValueError("Pulsa Descargar modelo antes de reconocer una ecuación.")
    from rapid_latex_ocr import LaTeXOCR

    model = LaTeXOCR(
        image_resizer_path=folder / "image_resizer.onnx",
        encoder_path=folder / "encoder.onnx",
        decoder_path=folder / "decoder.onnx",
        tokenizer_json=folder / "tokenizer.json",
    )
    image = Path(request["image"]["png_path"]).read_bytes()
    result, _ = model(image)
    if not result.strip():
        raise ValueError("No se reconoció una ecuación. Prueba con un recorte más ajustado.")
    return {"kind": "latex", "text": result}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--crabpdf-request", required=True, type=Path)
    parser.add_argument("--crabpdf-response", required=True, type=Path)
    args = parser.parse_args()
    with args.crabpdf_request.open("r", encoding="utf-8") as stream:
        request = json.load(stream)
    try:
        if request["api_version"] != 1:
            raise ValueError("API incompatible.")
        result = {"Ok": run(request)}
    except Exception as error:
        result = {"Err": str(error)[:1500] or "No se completó el OCR."}
    with args.crabpdf_response.open("x", encoding="utf-8") as stream:
        json.dump({"api_version": 1, "request_id": request["request_id"], "result": result}, stream, ensure_ascii=False)


if __name__ == "__main__":
    main()
