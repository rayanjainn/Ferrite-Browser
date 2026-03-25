# Ferrite Browser

> Capability-Governed Agentic Browsing with Verifiable Audit Trails

A developer-focused browser written in Rust, built on the [Servo](https://servo.org/) engine with a security-first architecture. Ferrite is the first browser designed to treat AI agents as **first-class principals** — with formal capability-based access control, tamper-evident audit logs, and a sandboxed Wasm extension system.

---

## What Is Ferrite?

Modern browsers have no formal model for AI agents. When an LLM-driven agent browses the web, it operates with the same ambient authority as the user — no scoping, no auditability, no revocation. Ferrite fixes this.

**Core guarantees:**
- Every privileged action (DOM reads, network requests, storage access) requires an **unforgeable capability token** minted by the broker after policy evaluation.
- Every grant and denial is written to a **hash-chained, SQLite-backed audit log** that cannot be silently tampered with.
- Extensions and agents run in an **Extism/Wasm sandbox** — they can only call host functions for capabilities explicitly declared in their manifest.
- The **Rego policy engine** (Regorus) evaluates per-principal, per-capability policies before any token is minted.

---

## Architecture Overview

```
┌────────────────────────────── Trusted Zone ──────────────────────────────────┐
│  Iced UI Shell ──── Capability Broker ──── Audit System ──── Policy Engine  │
│       │                    │                    │                  │          │
│    Tab bar              Token mint          Hash chain          Rego eval     │
│    Address bar          Token check         SQLite index        Risk class.   │
│    Consent dialogs      Token revoke        Verify chain                      │
└───────────────────────────────────────────────────────────────────────────────┘
         │                    ↑ capability tokens only
┌────────────────────────────── Untrusted Zone ─────────────────────────────────┐
│  Servo Engine ──── Extension Sandbox (Extism/Wasm) ──── Agent Runtime        │
│  (renders pages)   (runs .wasm plugins)                  (LLM command exec)   │
└───────────────────────────────────────────────────────────────────────────────┘
```

The **Capability Broker** is the single privileged boundary. All untrusted code accesses sensitive operations *only* through tokens minted by the broker. No ambient authority exists anywhere else in the system.

---

## Workspace Layout

```
Browser/                            ← Cargo workspace root
├── Cargo.toml                      ← workspace manifest
├── PROGRESS.md                     ← detailed change log and milestone tracker
├── TO-DO.md                        ← implementation task queue with prompts
└── crates/
    ├── ferrite-shell/              ← main binary: CLI entry point + integration smoke tests
    ├── ferrite-capability-broker/  ← token minting, broker logic, denial reasons
    ├── ferrite-audit-log/          ← hash-chained append-only log + SQLite persistence
    ├── ferrite-policy/             ← Regorus Rego policy engine integration
    ├── ferrite-servo/              ← winit window shell + Servo embedding (feature-gated)
    ├── ferrite-ui/                 ← Iced UI shell: tab bar, address bar, audit log viewer
    └── ferrite-sandbox/            ← Extism/Wasm extension sandbox + host function stubs
```

---

## Current Status (Month 1–2)

| Component | Status |
|-----------|--------|
| Cargo workspace scaffolded | Done |
| `ferrite-capability-broker` — token system + broker | Done |
| `ferrite-audit-log` — hash chain + SQLite | Done |
| `ferrite-policy` — Regorus policy engine stub | Done |
| `ferrite-shell` — integration smoke test | Done |
| GitHub Actions CI pipeline | Done |
| `ferrite-servo` crate scaffolded | Done |
| Servo embedding shell — WindowRenderingContext + WebView + https://example.com | Done |
| Iced UI shell (`ferrite-ui` — tab bar, address bar) | Done |
| Extism extension sandbox (`ferrite-sandbox`) | Done |
| Audit log viewer panel in Iced UI | Done |
| Broker ↔ Servo integration (request interception) | Not started |
| Agent runtime | Not started |

---

## Prerequisites

### Rust (all platforms)

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Minimum version: **Rust 1.77 stable**.

> The `rusqlite` crate uses the `bundled` feature, so no separate SQLite installation is needed on any platform.

---

### Windows 11

| Tool | Notes |
|------|-------|
| Rust stable MSVC | `rustup default stable-x86_64-pc-windows-msvc` |
| Visual Studio C++ Build Tools 2019+ | [Download](https://visualstudio.microsoft.com/downloads/) — select the **"Desktop development with C++"** workload |
| Git | [git-scm.com](https://git-scm.com/) |

No WSL2 required. PowerShell or Command Prompt both work.

**Verify C++ tools are on PATH:**
```powershell
cl /?
```

If `cl` is not found, open a **"Developer PowerShell for VS"** session or run `vcvarsall.bat x64` before building.

---

### Ubuntu 22.04 / 24.04

Install system dependencies:

```bash
sudo apt update
sudo apt install -y \
    build-essential git pkg-config \
    libssl-dev libglib2.0-dev \
    libx11-dev libxext-dev libxrender-dev libxi-dev \
    libxrandr-dev libxcursor-dev libxinerama-dev \
    libwayland-dev libwayland-egl1 libwayland-cursor0 \
    libxkbcommon-dev libxkbcommon-x11-0 \
    libegl1-mesa-dev libgl1-mesa-dev \
    libgles2-mesa-dev libgbm-dev \
    cmake ninja-build python3 python3-pip
```

Then install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

**Additional packages required only for the Servo feature** (`--features servo`):

```bash
sudo apt install -y \
    libfreetype6-dev libfontconfig1-dev \
    libdbus-1-dev libpulse-dev libudev-dev \
    libharfbuzz-dev libpango1.0-dev
```

> **Display note:** winit requires a display server. On a headless server, set `DISPLAY=:0` (X11) or `WAYLAND_DISPLAY=wayland-0` (Wayland) before running. For CI without a display, use `Xvfb`:
> ```bash
> sudo apt install -y xvfb
> Xvfb :99 &
> DISPLAY=:99 cargo run -p ferrite-shell
> ```

---

### macOS 13+ (Ventura / Sonoma / Sequoia)

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

Install [Homebrew](https://brew.sh/), then system dependencies:

```bash
brew install pkg-config cmake ninja python3 \
    openssl@3 freetype fontconfig harfbuzz \
    libxkbcommon
```

Then install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

**Apple Silicon (M1/M2/M3):** the default `aarch64-apple-darwin` target works. No extra steps needed.

**OpenSSL path** — if cargo fails to find OpenSSL, export:
```bash
export OPENSSL_DIR=$(brew --prefix openssl@3)
```

---

### Optional: Servo feature (all platforms)

The full Servo embedding (`--features servo`) compiles Servo from source via a git dependency. First build takes **10–20 minutes** and requires a C++ compiler and all platform-native libraries listed above.

The Servo feature is **off by default** — the smoke test and bare winit window build and run without it in under 60 seconds.

---

## Setup & Build

```bash
# 1. Clone the repository
git clone <repo-url>
cd "Major Project/Browser"

# 2. Verify the Rust toolchain
rustup show

# 3. Build all crates (no Servo feature — fast build ~30–60 seconds)
cargo build
```

This builds all seven crates: `ferrite-shell`, `ferrite-capability-broker`, `ferrite-audit-log`, `ferrite-policy`, `ferrite-servo`, `ferrite-ui`, and `ferrite-sandbox`.

If `cargo build` succeeds with no errors, the environment is correctly configured.

---

## Running the Project

### Sandbox Demo — Extension Capability Demo

Loads `hello-ext` (a Wasm extension) into the Extism sandbox, exercises `dom.read`, `network.fetch`, and `storage.read` through the capability broker, then revokes the `dom.read` token and verifies the denial is enforced.

**First time only** — install the Wasm target and build the extension:

```bash
rustup target add wasm32-unknown-unknown
cd extensions/hello-ext && cargo build --target wasm32-unknown-unknown --release
cd ../..   # back to Browser/
```

Then run the demo:

```bash
cargo run -p ferrite-shell sandbox
```

Expected output:
```
[ferrite] loading extension from: extensions/hello-ext/target/wasm32-unknown-unknown/release/hello_ext.wasm
[ferrite-sandbox] --- capability demo ---
[ferrite-sandbox] test_dom_read     → <div>stub DOM content for selector: h1</div>
[ferrite-sandbox] test_network_fetch → stub response body for: https://example.com/api
[ferrite-sandbox] test_storage_read  → stub value for key: user_prefs
[ferrite-sandbox] dom_read token revoked
[ferrite-sandbox] test_dom_read (post-revoke) → ERROR: dom.read denied
[ferrite-sandbox] demo complete — 4 audit entries, chain verified
```

The demo exercises:
- `CapabilityBroker` minting tokens for `DomRead`, `NetworkFetch`, `StorageRead`
- Wasm extension calling host functions via Extism PDK
- Token revocation blocking subsequent requests
- Audit log recording 4 entries with a verified hash chain

---

### Smoke Test — Integration Check (no window)

Runs the core integration test: mints a capability token, checks broker decisions, appends to the audit log, and verifies the hash chain.

```bash
cargo run -p ferrite-shell
```

Expected output:
```
Month 1-2 smoke test: ALL CHECKS PASSED
```

The smoke test exercises:
- `CapabilityBroker::mint_token()` — Extension principal, `NetworkFetch`, scoped to `https://example.com`
- `PolicyEngine::evaluate()` — Rego evaluation returns `true` for allowed principal/capability
- `broker.check()` → `Granted` for matching origin, `Denied(OriginMismatch)` for wrong origin
- `PersistentAuditLog::append()` — writes to SQLite at `%TEMP%\ferrite_smoke_test.db`
- `audit_log.log.verify_chain()` — verifies SHA-256 hash chain integrity

### Windowed Shell — Blank Window (no Servo)

Opens a native 1280×800 window using winit. No browser engine — just the windowing layer.

```bash
cargo run -p ferrite-shell window
```

Expected result: A blank window titled "Ferrite Browser" opens. Close with X or Escape. On exit, prints the audit chain summary.

### Windowed Shell — With Servo Engine (slow first build)

Builds and runs the full Servo embedding. First build downloads and compiles Servo from git (~10–20 min).

```bash
cargo run -p ferrite-shell --features ferrite-servo/servo window
```

Expected result:
- A window opens and Servo initialises its internal event loop
- `WindowRenderingContext` is created from the native window handles
- A WebView is created and immediately navigates to `https://example.com`
- Every network request is checked against the `CapabilityBroker` via `load_web_resource`
- Terminal prints: `[ferrite] page load complete: https://example.com/`
- On Escape or X: audit chain is verified and a grant/denial summary is printed

The `FerriteWebViewDelegate` intercepts all outgoing fetches. To test the block path, revoke the `network_token_id` on a running shell — subsequent requests will print `[ferrite] BLOCKED: <url> reason: ...`.

---

## Running Tests

### Unit Tests (all crates)

```bash
cargo test
```

### Single Crate Tests

```bash
cargo test -p ferrite-capability-broker
cargo test -p ferrite-audit-log
cargo test -p ferrite-policy
```

### Notable Tests

| Test | Crate | What it verifies |
|------|-------|-----------------|
| `default_policy_allows_all` | `ferrite-policy` | Regorus evaluates default Rego policy — extensions and agents are allowed |
| `audit_log_records_grants_and_denials` | `ferrite-shell` | Broker decisions are persisted to the audit log with correct event kinds |
| `revoke_blocks_subsequent_requests` | `ferrite-shell` | Revoking a token causes all subsequent `check()` calls to return `Denied` |

---

## Building Individual Crates

```bash
# Capability broker only
cargo build -p ferrite-capability-broker

# Audit log only
cargo build -p ferrite-audit-log

# Policy engine only
cargo build -p ferrite-policy

# Iced UI shell only
cargo build -p ferrite-ui

# Extension sandbox only
cargo build -p ferrite-sandbox

# Servo embedding (no Servo feature — fast)
cargo build -p ferrite-servo

# Servo embedding (with Servo feature — slow first build)
cargo build -p ferrite-servo --features servo
```

---

## Crate Reference

### `ferrite-capability-broker`

Token minting and capability enforcement.

Key types:
- `CapabilityBroker` — stores tokens in a `HashMap`, provides `mint_token()`, `check()`, `revoke()`
- `CapabilityToken` — scoped, time-bounded, per-principal token
- `BrokerDecision` — `Granted { token }` or `Denied { reason }`
- `DenialReason` — `PolicyRejected | TokenExpired | OriginMismatch | RateLimitExceeded | UnknownPrincipal`
- `CapabilityType` — `DomRead | DomWrite | NetworkFetch | StorageRead | StorageWrite | CookieRead | CookieWrite`

### `ferrite-audit-log`

Append-only, hash-chained audit log with SQLite persistence.

Key types:
- `AuditLog` — in-memory log with `append()` and `verify_chain()`
- `PersistentAuditLog` — wraps `AuditLog` + `rusqlite::Connection`; writes every entry to `audit_entries` table atomically
- `AuditEventKind` — `CapabilityGranted | CapabilityDenied | CapabilityExercised | ContentBlocked`
- `AuditEntry` — includes `entry_hash` (SHA-256 over sequence + timestamp + kind + principal + prev_hash) and `prev_hash`

Each entry's hash includes the previous entry's hash, forming a chain. `verify_chain()` recomputes and validates every link — any tampering breaks the chain.

### `ferrite-policy`

Rego policy evaluation via [Regorus](https://github.com/microsoft/regorus).

Key types:
- `PolicyEngine` — wraps `regorus::Engine`, loads default package `ferrite.capability` with `default allow = true`
- `PolicyEngine::evaluate(principal_kind, capability, origin) -> bool` — evaluates `data.ferrite.capability.allow`; errors → `true` (fail-open for now)

### `ferrite-servo`

Native window shell and Servo browser engine embedding.

Key types:
- `ServoShell` — `new()` + `run(self)` using winit 0.30 `ApplicationHandler`
- `AppHandler` — handles `resumed` (creates window, optionally initialises Servo) and `window_event` (close, redraw)
- Servo feature (`--features servo`): `FerriteWebViewDelegate`, `FerriteServoDelegate`, `ServoBuilder`-based initialisation

### `ferrite-shell`

Main binary. Dispatches based on CLI argument:
- No arg / any other → `run_smoke_test()` — integration check
- `window` → `ServoShell::new().run()` — opens the windowed shell

---

## Key Design Decisions

**Why capability tokens instead of permission prompts?**
Prompts can be spoofed or fatigue-clicked. Tokens are unforgeable, scoped, time-bounded, and cryptographically linked to the audit log. An agent or extension that loses its token loses the capability — no ambient authority remains.

**Why Servo instead of Chromium/CEF?**
Servo is written in Rust, memory-safe by default, and has a well-defined embedding API that allows intercepting network requests at a structured boundary. CEF is planned as a Track B fallback.

**Why Regorus (pure-Rust Rego) instead of OPA?**
OPA requires a separate process or CGO. Regorus is a pure-Rust Rego evaluator that compiles into the broker binary — no IPC overhead on every capability check.

**Why hash-chained audit log instead of a plain database?**
A plain database can be silently edited. The hash chain makes any modification to a past entry detectable — `verify_chain()` will return `false`. This property is essential for the audit log to serve as a tamper-evident record for the research paper.

---

## Development Notes

- Run `cargo fmt` before every commit
- Run `cargo clippy -- -D warnings` — zero warnings policy
- Update `PROGRESS.md` after every meaningful change
- The `rusqlite` version is pinned to `0.37` with `features = ["bundled"]` to avoid a link conflict with `libservo` (which also depends on rusqlite `^0.37`)

---

## License

Dual-licensed under MIT and Apache 2.0 — see LICENSE-MIT and LICENSE-APACHE.
