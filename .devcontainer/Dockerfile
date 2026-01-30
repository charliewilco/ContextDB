FROM rust:1.78-bullseye

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libsqlite3-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
