# PyInstaller links to the builder's glibc. Match the reader's Linux floor.
FROM python:3.12-slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends binutils libgl1 libglib2.0-0 libgomp1 libstdc++6 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY extensions/math-ocr/requirements.txt extensions/math-ocr/requirements.txt
RUN python -m pip install --no-cache-dir --only-binary=:all: -r extensions/math-ocr/requirements.txt
# RapidLaTeXOCR imports cv2 for image processing. Use the equivalent wheel without GUI libraries.
RUN python -m pip uninstall --yes opencv-python \
    && python -m pip install --no-cache-dir --only-binary=:all: opencv-python-headless==4.11.0.86
COPY LICENSE .
COPY extensions/math-ocr extensions/math-ocr/
RUN python extensions/math-ocr/package.py x86_64-unknown-linux-gnu

FROM scratch AS archive
COPY --from=build /src/artifacts/site-extensions/ /
