# Ferrite Browser — Build, Run & Test Commands

All commands are run from the **workspace root** (`Browser/`) unless noted otherwise.

---

## Prerequisites

### Install Rust toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install stable
rustup target add wasm32-unknown-unknown
rustup component add clippy rustfmt
```

### System dependencies (Linux / devcontainer)
```bash
sudo apt-get update && sudo apt-get install -y \
  pkg-config cmake clang lld llvm python3 \
  libssl-dev libdbus-1-dev libfreetype6-dev libfontconfig1-dev \
  libglib2.0-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libx11-dev \
  sqlite3 libsqlite3-dev
```

### Windows (native)
The workspace builds natively on Windows with the MSVC toolchain.
Servo (`--features servo`) requires additional native libs — use the
devcontainer for full Servo builds on Windows.

---

## 1. Fetch dependencies

```bash
cargo fetch
```

---

## 2. Build

### Build all crates (no Servo engine)
```bash
cargo build --workspace
```

### Build all crates (release)
```bash
cargo build --workspace --release
```

### Build individual crates
```bash
cargo build -p ferrite-capability-broker
cargo build -p ferrite-audit-log
cargo build -p ferrite-policy
cargo build -p ferrite-sandbox
cargo build -p ferrite-servo
cargo build -p ferrite-ui
cargo build -p ferrite-shell
```

### Build with Servo engine enabled
Requires Linux / devcontainer. Enables the real headless WebView.
```bash
cargo build -p ferrite-servo --features servo
cargo build -p ferrite-shell --features ferrite-servo/servo
cargo build -p ferrite-ui   --features ferrite-servo/servo
```

### Build the hello-ext Wasm extension
Required before running the sandbox demo.
```bash
cd extensions/hello-ext
cargo build --target wasm32-unknown-unknown --release
cd ../..
```

---

## 3. Run

### Iced UI shell (full browser window)
```bash
cargo run -p ferrite-shell -- ui
```
With Servo engine:
```bash
cargo run -p ferrite-shell --features ferrite-servo/servo -- ui
```

### Servo window (winit window, no Iced)
```bash
cargo run -p ferrite-shell -- window
```

### Sandbox demo (Wasm extension + capability broker + audit log)
Build hello-ext first (see section 2), then:
```bash
cargo run -p ferrite-shell -- sandbox
```

### Broker + audit smoke test (Month 1–2 integration check)
```bash
cargo run -p ferrite-shell
```
Expected output: `Month 1-2 smoke test: ALL CHECKS PASSED`

---

## 4. Test

### Run all unit tests across the workspace
```bash
cargo test --workspace
```

### Run tests for individual crates
```bash
cargo test -p ferrite-capability-broker
cargo test -p ferrite-audit-log
cargo test -p ferrite-policy
cargo test -p ferrite-sandbox
cargo test -p ferrite-servo
cargo test -p ferrite-ui
cargo test -p ferrite-shell
```

### Run tests with output visible (don't capture stdout)
```bash
cargo test --workspace -- --nocapture
```

---

## 5. Lint & format

### Check formatting (CI-style, no changes written)
```bash
cargo fmt --check
```

### Apply formatting
```bash
cargo fmt
```

### Clippy (zero-warning policy)
```bash
cargo clippy --workspace -- -D warnings
```

### Clippy for a single crate
```bash
cargo clippy -p ferrite-capability-broker -- -D warnings
cargo clippy -p ferrite-audit-log -- -D warnings
cargo clippy -p ferrite-servo -- -D warnings
cargo clippy -p ferrite-ui -- -D warnings
```

---

## 6. Check (fast compilation check, no binary produced)

```bash
cargo check --workspace
cargo check -p ferrite-servo -p ferrite-ui
```

---

## 7. Full CI sequence (mirrors GitHub Actions)

Run these in order to reproduce exactly what CI checks:

```bash
# 1. Fetch
cargo fetch

# 2. Format check
cargo fmt --check

# 3. Clippy
cargo clippy --workspace -- -D warnings

# 4. Unit tests
cargo test --workspace

# 5. Build hello-ext Wasm
cd extensions/hello-ext
cargo build --target wasm32-unknown-unknown --release
cd ../..

# 6. Smoke test
cargo run -p ferrite-shell
```

---

## 8. Docker devcontainer

### Build the devcontainer image
Run from the **repo root** (`Major Project/`), not `Browser/`:
```bash
docker build -f .devcontainer/Dockerfile -t ferrite-dev .
```

### Run a shell inside the container
```bash
docker run --rm -it -v "%cd%":/workspace ferrite-dev bash
```

---

## 9. Module summary

| Crate | Purpose | Key feature flag |
|-------|---------|-----------------|
| `ferrite-capability-broker` | Token minting, policy checks, revocation | — |
| `ferrite-audit-log` | Hash-chained append-only log + SQLite index | — |
| `ferrite-policy` | Rego policy engine stub | — |
| `ferrite-sandbox` | Extism/Wasm extension host with capability-gated host functions | — |
| `ferrite-servo` | Servo WebView session (headless, CPU-rendered) | `--features servo` |
| `ferrite-ui` | Iced UI shell (tabs, address bar, audit panel) | `--features ferrite-servo/servo` |
| `ferrite-shell` | Main binary — CLI dispatcher for all modes | `--features ferrite-servo/servo` |
| `extensions/hello-ext` | Demo Wasm extension (dom.read / network.fetch / storage.read) | wasm32 target |
