FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:$PATH

# ── System dependencies ────────────────────────────────────────────────────
# Servo requires: clang, lld, cmake, pkg-config, python3, gstreamer,
# libdbus, libfreetype, libfontconfig, libssl, libxcb + friends
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    git \
    pkg-config \
    cmake \
    clang \
    lld \
    llvm \
    python3 \
    python3-pip \
    libssl-dev \
    libdbus-1-dev \
    libfreetype6-dev \
    libfontconfig1-dev \
    libglib2.0-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-bad1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    libxcb1-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libx11-dev \
    libxext-dev \
    libxrandr-dev \
    libxi-dev \
    libxcursor-dev \
    sqlite3 \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ── Rust toolchain ─────────────────────────────────────────────────────────
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --no-modify-path && \
    rustup target add wasm32-unknown-unknown && \
    rustup component add clippy rustfmt rust-analyzer

# ── Pre-warm Cargo registry ────────────────────────────────────────────────
# Copy only manifests first so this layer is cached unless deps change.
WORKDIR /build/Browser
COPY Browser/Cargo.toml ./Cargo.toml
COPY Browser/crates/ferrite-shell/Cargo.toml          ./crates/ferrite-shell/Cargo.toml
COPY Browser/crates/ferrite-capability-broker/Cargo.toml ./crates/ferrite-capability-broker/Cargo.toml
COPY Browser/crates/ferrite-audit-log/Cargo.toml      ./crates/ferrite-audit-log/Cargo.toml
COPY Browser/crates/ferrite-policy/Cargo.toml         ./crates/ferrite-policy/Cargo.toml
COPY Browser/crates/ferrite-servo/Cargo.toml          ./crates/ferrite-servo/Cargo.toml
COPY Browser/crates/ferrite-ui/Cargo.toml             ./crates/ferrite-ui/Cargo.toml
COPY Browser/crates/ferrite-sandbox/Cargo.toml        ./crates/ferrite-sandbox/Cargo.toml

# Create stub lib.rs / main.rs for each crate so `cargo fetch` resolves fully
RUN for dir in \
        ferrite-capability-broker \
        ferrite-audit-log \
        ferrite-policy \
        ferrite-servo \
        ferrite-ui \
        ferrite-sandbox; do \
    mkdir -p crates/$dir/src && \
    echo "// stub" > crates/$dir/src/lib.rs; \
    done && \
    mkdir -p crates/ferrite-shell/src && \
    echo "fn main(){}" > crates/ferrite-shell/src/main.rs

# Fetch all dependencies (populates CARGO_HOME registry cache)
RUN cargo fetch

# ── Working directory for actual development ───────────────────────────────
WORKDIR /workspace
