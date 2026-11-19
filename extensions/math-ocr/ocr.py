"""Independent CrabPDF native protocol adapter for RapidLaTeXOCR."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import urllib.request

BASE = "https://github.com/RapidAI/RapidLaTeXOCR/releases/download/v0.0.0/"
MODELS = {
    "image_resizer.onnx": (38967751, "e0b075c39700f64d50400f39c8fc186bbb3b5d84d31864008313f376603aca9d"),
    "encoder.onnx": (89008136, "01bf5dc25539ca0cd5b1bd29296ea495977a6ba5f629dc4178277809d26e5e7d"),
    "decoder.onnx": (50952726, "bd695497bf1b22279b7626f5916c79226e1e244c84355f8da7edfd2d921d0072"),
    "tokenizer.json": (24174, "1dc27b18d6a518d0d5ff3f4bb7bd98521fe80ad39e5b2a246d4109f1bb9d5019"),
}


def valid(path, size, digest):
    if not path.is_file() or path.stat().st_size != size:
        return False
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest() == digest


def prepare(folder):
    folder.mkdir(parents=True, exist_ok=True)
    for name, (size, digest) in MODELS.items():
        path = folder / name
        if valid(path, size, digest):
            continue
        part = path.with_suffix(path.suffix + ".download")
        try:
            request = urllib.request.Request(BASE + name, headers={"User-Agent": "CrabPDF-OCR"})
            with urllib.request.urlopen(request, timeout=45) as stream, part.open("wb") as output:
                remaining = size
                while remaining:
                    chunk = stream.read(min(remaining, 1024 * 1024))
                    if not chunk:
                        raise ValueError("La descarga del modelo quedó incompleta.")
                    output.write(chunk)
                    remaining -= len(chunk)
                if stream.read(1):
                    raise ValueError("El modelo supera el tamaño publicado.")
            if not valid(part, size, digest):
                raise ValueError(f"El SHA-256 de {name} no coincide.")
            part.replace(path)
        finally:
            part.unlink(missing_ok=True)


def run(request):
    folder = Path(os.environ["CRABPDF_EXTENSION_DATA"]) / "rapidlatex-v0"
    if request["command"] == "prepare":
        prepare(folder)
        return {"kind": "text", "text": "Modelo descargado y verificado. Selecciona una ecuación."}
    if request["command"] != "recognize" or not request.get("image"):
        raise ValueError("Selecciona un recorte con una ecuación.")
    if not all(valid(folder / name, size, digest) for name, (size, digest) in MODELS.items()):
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
