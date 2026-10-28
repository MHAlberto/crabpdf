FROM rust:1.96.0-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential cmake pkg-config python3 ca-certificates \
    libssl-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev \
    libgl1-mesa-dev libegl1-mesa-dev libwayland-dev libxkbcommon-dev libdbus-1-dev \
    djvulibre-bin p7zip-full xvfb xauth \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /work
