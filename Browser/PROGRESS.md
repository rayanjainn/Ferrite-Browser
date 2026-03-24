# Ferrite Browser — Progress Tracker

> **How to use this file:**
> Update this file every time a feature is implemented, a crate is modified, or a milestone is reached.
> Each entry should include the date, what changed, and what the current state is.
> This file lives in `Browser/` (the Cargo workspace root) and is tracked by git.

---

## Project Structure

```
Major Project/                  ← git repo root, reference docs, PDFs
├── PROJECT_REFERENCE.md        ← full architecture + 8-month plan reference
├── CLAUDE.md (in Browser/)     ← Claude Code context file
└── Browser/                    ← Cargo workspace root, all Rust code
    ├── Cargo.toml              ← workspace manifest
    ├── CLAUDE.md               ← Claude Code CLI context file
    ├── PROGRESS.md             ← this file
    └── crates/
        ├── ferrite-shell/              ← binary: browser shell + smoke tests
        ├── ferrite-capability-broker/  ← lib: token minting, broker logic
        ├── ferrite-audit-log/          ← lib: hash-chained log + SQLite
        └── ferrite-policy/             ← lib: Rego policy engine (stub)
```

---

## Milestone Status

| Milestone | Status |
|-----------|--------|
| Cargo workspace scaffolded | ✅ Done |
| `ferrite-capability-broker` — types + broker | ✅ Done |
| `ferrite-audit-log` — hash chain + SQLite | ✅ Done |
| `ferrite-policy` — Regorus policy engine stub | ✅ Done |
| `ferrite-shell` — integration smoke test | ✅ Done |
| GitHub Actions CI pipeline | ⏳ Not started |
| Servo embedding shell (Month 1 R1 task) | ⏳ Not started |
| Iced UI shell (Month 1–2 R3 task) | ⏳ Not started |

---

## Change Log

### 2026-03-24 — Workspace scaffolded + Broker + Audit Log implemented

**Environment**
- IDE: Google Antigravity (installed, VS Code fork)
- Terminal: PowerShell (Windows, no WSL2)
- Rust toolchain: stable MSVC
- Build tools: Visual Studio C++ Build Tools installed

**Workspace**
- Initialized Cargo workspace at `Browser/` with `resolver = "2"`
- Four crates created under `Browser/crates/`:
  - `ferrite-shell` (binary)
  - `ferrite-capability-broker` (lib)
  - `ferrite-audit-log` (lib)
  - `ferrite-policy` (lib)
- `cargo build` passes cleanly

---

### `ferrite-capability-broker` — COMPLETE

**File:** `crates/ferrite-capability-broker/src/lib.rs`

**Dependencies added:**
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
```

**What was implemented:**
- `CapabilityType` enum — 7 Wave 1 capability types: `DomRead`, `DomWrite`, `NetworkFetch`, `StorageRead`, `StorageWrite`, `CookieRead`, `CookieWrite`
- `PrincipalKind` enum — `Extension`, `Agent`, `WebContent`
- `Principal` struct — `id: Uuid`, `kind: PrincipalKind`, `label: String`
- `CapabilityToken` struct — `token_id`, `principal`, `capability`, `origin_scope`, `url_allowlist`, `rate_limit`, `issued_at`, `expires_at`
- `CapabilityToken::is_expired()` — compares `expires_at` against `Utc::now()`
- `CapabilityToken::matches_origin()` — checks `origin_scope == "*"` or exact match
- `DenialReason` enum — `PolicyRejected`, `TokenExpired`, `OriginMismatch`, `RateLimitExceeded`, `UnknownPrincipal`
- `BrokerDecision` enum — `Granted { token }` or `Denied { reason }`
- `CapabilityBroker` struct — `HashMap<Uuid, CapabilityToken>` storage
- `CapabilityBroker::mint_token()` — creates token, stores it, returns `token_id`
- `CapabilityBroker::check()` — validates expiry + origin, returns `BrokerDecision`
- `CapabilityBroker::revoke()` — removes token from map

**What is NOT yet implemented (deferred to Month 3):**
- Policy engine integration inside `check()` (currently no Rego evaluation on grant)
- Rate limit enforcement (field exists on token, not yet checked in `check()`)
- IPC / async channels (all in-process for now)

---

### `ferrite-audit-log` — COMPLETE

**File:** `crates/ferrite-audit-log/src/lib.rs`

**Dependencies added:**
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
rusqlite = { version = "0.31", features = ["bundled"] }
thiserror = "1"
```

**What was implemented:**
- `AuditEventKind` enum — `CapabilityGranted`, `CapabilityDenied`, `CapabilityExercised`, `ContentBlocked`
- `AuditEntry` struct — full entry with `entry_id`, `sequence`, `timestamp`, `kind`, `principal_id`, `capability`, `url`, `prev_hash`, `entry_hash`
- `AuditError` enum (thiserror) — `HashMismatch`, `ChainBroken`, `Sql`, `Parse`, `Serialization`, `ChainBrokenLoad`
- `AuditLog` struct — in-memory log with `Vec<AuditEntry>` and `sequence_counter`
- `AuditLog::append()` — computes SHA-256 over `"{sequence}{timestamp}{kind:?}{principal_id}{prev_hash}"`, stores entry
- `AuditLog::verify_chain()` — recomputes every hash and validates prev_hash linkage, returns `bool`
- `PersistentAuditLog` struct — wraps `AuditLog` + `rusqlite::Connection`
- `PersistentAuditLog::new()` — opens SQLite, creates `audit_entries` table if not exists
- `PersistentAuditLog::append()` — appends to in-memory log AND inserts row into SQLite atomically
- `PersistentAuditLog::load()` — reads all rows ordered by sequence, reconstructs log, calls `verify_chain()`, errors if chain broken

**What is NOT yet implemented (future months):**
- Merkle tree upgrade (planned Month 5)
- Proof generation / verification API
- Connection to broker events (broker does not yet call audit log on grant/deny)

---

### `ferrite-policy` — COMPLETE (2026-03-24)

**File:** `crates/ferrite-policy/src/lib.rs`

**Dependencies added:**
```toml
regorus = "0.2"
serde_json = "1"
thiserror = "1"
```

**What was implemented:**
- `PolicyEngine` struct wrapping `regorus::Engine`
- `PolicyEngine::new()` — creates engine, loads default Rego package `ferrite.capability` with `default allow = true`
- `PolicyEngine::evaluate(&mut self, principal_kind, capability, origin) -> bool` — sets input JSON, evaluates `data.ferrite.capability.allow`, returns `true` on any error (fail-open)
- `Default` impl for `PolicyEngine`
- Unit test: `default_policy_allows_all` verifies extension and agent requests both return `true`

---

### `ferrite-shell` — COMPLETE (2026-03-24)

**File:** `crates/ferrite-shell/src/main.rs`

**Dependencies added:**
```toml
ferrite-capability-broker = { path = "../ferrite-capability-broker" }
ferrite-audit-log = { path = "../ferrite-audit-log" }
ferrite-policy = { path = "../ferrite-policy" }
uuid = { version = "1", features = ["v4"] }
```

**What was implemented:**
- Integration smoke test wiring all three crates together
- Creates `CapabilityBroker`, `PersistentAuditLog` (at temp dir), `PolicyEngine`
- Mints Extension/NetworkFetch token scoped to `https://example.com` for 3600s
- Asserts `policy.evaluate("extension", "network.fetch", "https://example.com") == true`
- Asserts `broker.check(token_id, "https://example.com/path")` → `Granted`
- Asserts `broker.check(token_id, "https://evil.com")` → `Denied(OriginMismatch)`
- Appends `CapabilityGranted` event to `PersistentAuditLog`
- Asserts `audit_log.log.verify_chain() == true`
- Prints: `Month 1-2 smoke test: ALL CHECKS PASSED`

**Verified:** `cargo run -p ferrite-shell` prints the success message cleanly.

---

## What To Do Next (pick up here after plan refreshes)

1. **CI pipeline** — create `.github/workflows/ci.yml`

2. **Month 1 R1 task** — Servo embedding shell (separate work stream, see `PROJECT_REFERENCE.md`)

---

## Known Issues / Notes

- `check.txt`, `check2.txt`, `check_output.txt` exist in `Browser/` root — these appear to be scratch files from earlier testing. Consider deleting or moving them.
- `ferrite-policy/src/lib.rs` still has the default cargo placeholder — do not confuse this with a real implementation.
- Rate limiting is tracked on `CapabilityToken` via the `rate_limit: Option<u32>` field but is not enforced in `CapabilityBroker::check()` yet. This is intentional — enforcement comes in Month 3.
