FROM rust:1.78-bullseye

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libsqlite3-dev \
        sqlite3 \
        ca-certificates \
        curl \
        git \
        jq \
        ripgrep \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

WORKDIR /workspace
