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
| GitHub Actions CI pipeline | ✅ Done |
| `ferrite-servo` crate scaffolded | ✅ Done |
| Servo embedding shell (Month 1 R1 task) | 🔄 In progress |
| Iced UI shell (Month 1–2 R3 task) | ✅ Done |
| Extism Extension Sandbox (`ferrite-sandbox`) | ✅ Done |
| Audit Log Viewer | ⏳ Not started(Wait for Iced UI Shell to be complete for final decision) |

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

### `ferrite-servo` — SCAFFOLDED (2026-03-24)

**Files created:**
- `crates/ferrite-servo/Cargo.toml` — dependencies: `servo` (git tag v0.0.5), `raw-window-handle 0.6`, `winit 0.30`, `log 0.4`
- `crates/ferrite-servo/src/lib.rs` — declares `pub mod shell`
- `crates/ferrite-servo/src/shell.rs` — empty stub, awaiting Block 2 implementation

**Root `Cargo.toml`** — added `"crates/ferrite-servo"` to workspace members.

**Verified:** `cargo metadata --no-deps` lists all 5 workspace members correctly.

**Not yet built:** Servo git dependency not yet resolved — see Known Issues below.

### `ServoShell` — winit shell + Servo integration skeleton (2026-03-24)

**File:** `crates/ferrite-servo/src/shell.rs`

**API research finding:** The Servo v0.0.5 embedding API does **not** use a `WindowMethods` trait or methods like `get_coordinates()` / `get_gl_context()` — those are from an older prototype. The actual API (researched from upstream repo) is:
- `ServoBuilder::default()` — builder pattern, not `Servo::new()`
- `servo.set_delegate(ServoDelegate)` — global callbacks
- `WebView` — per-tab handle; created via Servo, accessed via `WebViewDelegate` callbacks
- `webview.load(ServoUrl)`, `webview.resize(Size2D)`, `webview.paint()`
- `servo.spin_event_loop()` — drives Servo's internal task queue

**What was implemented:**
- `FerriteWebViewDelegate` — implements `servo::WebViewDelegate`; `notify_new_frame_ready` calls `webview.paint()`
- `FerriteServoDelegate` — implements `servo::ServoDelegate`; all methods default no-ops for now
- `AppHandler` struct (winit 0.30 `ApplicationHandler`):
  - `resumed()` — creates winit window 1280×800; `#[cfg(feature = "servo")]`: builds Servo via `ServoBuilder`, sets delegate, creates WebView, loads `about:blank`, calls `spin_event_loop()` once
  - `window_event()` — `CloseRequested` → exit; `RedrawRequested` → `spin_event_loop()` + `webview.paint()` (servo feature) + `request_redraw()`
- `ServoShell` — `new()` + `run(self)`, `Default` impl

**`ferrite-shell/Cargo.toml`** — added `ferrite-servo = { path = "../ferrite-servo" }`

**`ferrite-shell/src/main.rs`** — smoke test moved to `fn run_smoke_test()`:
- First CLI arg `"window"` → `ServoShell::new().run()` (opens window)
- Anything else → `run_smoke_test()` (original 10-step check)

**Verified:**
- `cargo build -p ferrite-shell` — clean build, 26s
- `cargo run -p ferrite-shell` → `Month 1-2 smoke test: ALL CHECKS PASSED`
- `cargo run -p ferrite-shell window` → opens 1280×800 winit window

---

**Feature flag:** `ferrite-servo/Cargo.toml` has `[features] servo = []`. Servo code activates with `--features servo`. Build without the feature gives the bare winit shell.

**Verified:**
- `cargo build -p ferrite-shell` — clean, 3.87s
- `cargo run -p ferrite-shell` → `Month 1-2 smoke test: ALL CHECKS PASSED`
- `cargo run -p ferrite-shell window` → opens 1280×800 winit window

### `ferrite-ui` — address bar (2026-03-25)

**File:** `crates/ferrite-ui/src/lib.rs`

**What changed:**
- `FerriteBrowser`: added `address_bar_input: String` and `tab_urls: Vec<String>` (default `["about:blank"]`); `tab_urls` stays parallel to `tabs`
- `FerriteBrowserMessage`: added `AddressBarChanged(String)` and `NavigateRequested(String)`
- `update()`: `AddressBarChanged` sets `address_bar_input`; `NavigateRequested` commits to `tab_urls[active_tab]` + logs; `AddTab`/`CloseTab`/`SelectTab` now also keep `tab_urls` in sync and reflect the current tab's URL back into `address_bar_input` on every tab switch
- `view()`: address bar row inserted between tab bar and content — `text_input("Enter URL...", &address_bar_input)` with `.on_input(AddressBarChanged)` and `.on_submit(NavigateRequested(...))`; below the input, `text("Current URL: {tab_urls[active_tab]}")` at size 12

**Verified:** `cargo build -p ferrite-ui` — zero warnings

---

### `ferrite-ui` — tab bar (2026-03-25)

**File:** `crates/ferrite-ui/src/lib.rs`

**What changed:**
- `FerriteBrowser` state: added `tabs: Vec<String>` (initialised with `["New Tab"]`) and `active_tab: usize`; replaced `#[derive(Default)]` with explicit `impl Default`
- `FerriteBrowserMessage`: added `AddTab`, `CloseTab(usize)`, `SelectTab(usize)`
- `update()`: `AddTab` pushes "New Tab" + sets active; `CloseTab(i)` removes entry + clamps active (guarded: no-op when only one tab remains, close button disabled); `SelectTab(i)` sets active
- `view()`: tab strip rendered as a horizontal `row` of (label button + × button) groups followed by a "+" button, wrapped in a `container` with `background.weak` fill; content area is a centered placeholder text showing "Tab N content"
- Five styling functions using `theme.extended_palette()`: `tab_bar_style` (container, `background.weak`), `tab_inactive_style` (hover → `background.strong`), `tab_active_style` (`primary.strong` accent), `close_btn_style` (transparent, danger tint on hover), `add_tab_style` (transparent, `background.strong` on hover)
- Close button uses `.on_press_maybe()` — `None` when tab count is 1, disabling it without removing it

**Verified:** `cargo build -p ferrite-ui` — zero warnings, 4.2s

---

### `ferrite-ui` — Iced UI shell (2026-03-25)

**Files created:**
- `crates/ferrite-ui/Cargo.toml` — `iced = { version = "0.13", features = ["tokio"] }`
- `crates/ferrite-ui/src/lib.rs`

**Workspace changes:**
- `Browser/Cargo.toml` — added `"crates/ferrite-ui"` to workspace members
- `crates/ferrite-shell/Cargo.toml` — added `ferrite-ui = { path = "../ferrite-ui" }`
- `crates/ferrite-shell/src/main.rs` — added `"ui"` arm to CLI match → calls `ferrite_ui::launch()`

**Iced 0.13 API note:**
Iced 0.13 replaced the `Application` trait with a functional builder pattern.
`iced::application(title, update, view)` returns an `Application` builder; `.theme()`, `.window_size()`, `.centered()`, `.run()` are chained on it. There is no trait to implement — state, message, update fn, and view fn are free items wired together by the builder.

**What was implemented:**
- `FerriteBrowser` — state struct (`#[derive(Debug, Default)]`, currently empty)
- `FerriteBrowserMessage` — message enum (currently empty; empty enum = exhaustive match with zero arms, so `update` is a no-op by construction)
- `update(state, message) -> Task<FerriteBrowserMessage>` — no-op via `match message {}`
- `view(state) -> Element<'_, FerriteBrowserMessage>` — returns `center(text("Ferrite Browser — loading..."))`
- `pub fn launch() -> iced::Result` — calls `iced::application("Ferrite Browser", update, view).window_size(1280×800).centered().theme(|_| Theme::Dark).run()`

**Verified:**
- `cargo build -p ferrite-ui` — zero warnings
- `cargo build -p ferrite-shell` — zero warnings
- `cargo run -p ferrite-shell ui` → opens 1280×800 dark-themed Iced window with centered placeholder text

---

### `ferrite-sandbox` — broker-gated host functions (2026-03-25)

**Files changed:**
- `crates/ferrite-sandbox/Cargo.toml` — added `ferrite-capability-broker`, `uuid`
- `crates/ferrite-sandbox/src/lib.rs` — full rewrite with broker + host functions
- `extensions/hello-ext/src/lib.rs` — added three test exports

**What changed in `lib.rs`:**
- `BrokerState` struct holds `CapabilityBroker` + three token UUIDs; wrapped in `UserData<BrokerState>` (`Arc<Mutex<T>>` under the hood)
- Three tokens minted in `new()`: `DomRead`, `NetworkFetch`, `StorageRead`, all `origin="*"`, 3600s TTL, `PrincipalKind::Extension`
- Three host functions defined with `extism::host_fn!` macro:
  - `host_dom_read(selector: String) -> String` — broker check on `dom_read_token`; granted → stub DOM HTML; denied → error string
  - `host_network_fetch(url: String) -> String` — broker check on `network_fetch_token` with actual URL; granted → stub body; denied → error string
  - `host_storage_read(key: String) -> String` — broker check on `storage_read_token`; granted → stub value; denied → error string
- `Function::new("host_*", [PTR], [PTR], broker_state.clone(), fn_ptr)` registers each — no explicit namespace so extism defaults to `"extism:host/user"` (matches PDK)
- `Plugin::new(manifest, [f_dom_read, f_network_fetch, f_storage_read], false)`
- `call_test(export)` helper for host-side invocation of the test exports
- `with_broker(f)` accessor for token revocation from outside the sandbox

**What changed in `hello-ext/src/lib.rs`:**
- `#[host_fn("extism:host/user")] extern "ExtismHost"` block declares all three imports
- `test_dom_read`, `test_network_fetch`, `test_storage_read` — each calls its host function with a fixed argument and returns the result

**Extism host_fn! ABI note:**
- String args use `PTR = ValType::I64` — a pointer into Extism shared memory
- `host_fn!` macro auto-decodes via `plugin.memory_get_val(&inputs[n])` and encodes return via `plugin.memory_new(&output)` + `memory_to_val`

**Wasm build still blocked** by Homebrew Rust lacking `wasm32-unknown-unknown` stdlib (requires rustup). All host-side code compiles and checks clean.

**Verified:**
- `cargo check -p ferrite-sandbox` — zero warnings
- `cargo check -p ferrite-shell` — zero warnings

---

### `ferrite-sandbox` — audit log integration + `run_demo()` (2026-03-25)

**Files changed:**
- `crates/ferrite-sandbox/Cargo.toml` — added `ferrite-audit-log = { path = "../ferrite-audit-log" }`
- `crates/ferrite-sandbox/src/lib.rs` — `BrokerState` → `SandboxState`, added `PersistentAuditLog` + `run_demo()`
- `crates/ferrite-shell/src/main.rs` — `"sandbox"` arm now calls `sandbox.run_demo()` instead of `ping()`

**What changed:**
- `BrokerState` renamed to `SandboxState`; `PersistentAuditLog` added as a field alongside `CapabilityBroker`; opened at `$TMPDIR/ferrite_sandbox.db`
- `principal_id: Uuid` added to `SandboxState` so host functions can log which principal triggered each event
- Each host function (`host_dom_read`, `host_network_fetch`, `host_storage_read`) now:
  - Copies `principal_id` before the `append()` call (avoids borrow-checker split-borrow error)
  - Appends `AuditEventKind::CapabilityGranted` or `CapabilityDenied` to `state.audit_log` after every broker decision
- `run_demo(&mut self)` added to `ExtensionSandbox`:
  1. Calls `test_dom_read` → prints granted response
  2. Calls `test_network_fetch` → prints granted response
  3. Calls `test_storage_read` → prints granted response
  4. Revokes `dom_read_token` via `self.state.get()?.lock()`
  5. Calls `test_dom_read` again → prints denied error string
  6. Asserts `audit_log.log.verify_chain()` (panics on tampered chain)
  7. Prints entry count summary line

**Borrow fix:** `let principal_id = state.principal_id;` copied before `state.audit_log.append(...)` in all three host functions — needed because `append` takes `&mut self` on `audit_log`, creating a mutable borrow of `state`, which conflicts with the field read of `state.principal_id` in the same expression.

**Verified:**
- `cargo check -p ferrite-sandbox` — zero warnings
- `cargo check -p ferrite-shell` — zero warnings

---

### `ferrite-sandbox` + `hello-ext` — Extism sandbox scaffolding (2026-03-25)

**Files created:**
- `crates/ferrite-sandbox/Cargo.toml` — `extism = "1"`, `thiserror = "1"`, `log = "0.4"`
- `crates/ferrite-sandbox/src/lib.rs`
- `extensions/hello-ext/Cargo.toml` — `crate-type = ["cdylib"]`, `extism-pdk = "1"`, `[workspace]` to opt out of host workspace
- `extensions/hello-ext/src/lib.rs`

**Workspace + shell changes:**
- `Browser/Cargo.toml` — added `"crates/ferrite-sandbox"` to workspace members
- `crates/ferrite-shell/Cargo.toml` — added `ferrite-sandbox = { path = "../ferrite-sandbox" }`
- `crates/ferrite-shell/src/main.rs` — added `"sandbox"` arm → `run_sandbox_smoke_test()` which loads `hello_ext.wasm` and calls `ping()`

**Extism 1.x API (researched from extism-1.20.0 source):**
- `Plugin::new(manifest, [], false)` — loads Wasm from a `Manifest`, no host functions, WASI off
- `Manifest::new([Wasm::data(bytes)])` — constructs manifest from raw bytes
- `plugin.function_exists(name)` — checks export exists with extism-compatible signature
- `plugin.call::<(), String>(name, ())` — calls with no input, returns UTF-8 string output

**SandboxError enum:** `LoadFailed(String)`, `CallFailed(String)`, `CapabilityDenied(String)`

**ExtensionSandbox:**
- `new(wasm_path)` — reads file, wraps in `Manifest`, instantiates `Plugin`
- `ping()` — checks `function_exists("ping")`, calls it, returns `String`

**hello-ext guest:**
- `#[plugin_fn] pub fn ping(_: ()) -> FnResult<String>` returns `"pong"`
- `#![no_main]` required by extism-pdk

**Wasm build blocker:** The host machine uses Homebrew Rust which only ships the native target. The `wasm32-unknown-unknown` stdlib is not available without rustup. Build command once rustup is available:
```
cd extensions/hello-ext
cargo build --target wasm32-unknown-unknown --release
# Output: target/wasm32-unknown-unknown/release/hello_ext.wasm
```

**Verified:**
- `cargo check -p ferrite-sandbox` — zero warnings
- `cargo check -p ferrite-shell` — zero warnings

---

### `ServoShell` — audit log integration + exit summary (2026-03-25)

**File:** `crates/ferrite-servo/src/shell.rs`
**File:** `crates/ferrite-servo/Cargo.toml`

**Dependencies added:**
```toml
ferrite-audit-log = { path = "../ferrite-audit-log" }
```

**What changed:**
- `PersistentAuditLog` added to `ServoShell` and `AppHandler` behind `Rc<RefCell<>>`, opened at `$TMPDIR/ferrite_servo.db` in `ServoShell::new()`
- `FerriteWebViewDelegate` gains `principal_id: Uuid` and `audit_log: Rc<RefCell<PersistentAuditLog>>`
- `load_web_resource` now appends `CapabilityGranted` or `CapabilityDenied` to the audit log on every broker decision
- `AppHandler::print_exit_summary()` helper: calls `audit_log.log.verify_chain()`, prints verification result + entry count, then prints grant/denial counts computed from `entries`
- `CloseRequested` and Escape both call `print_exit_summary()` before `event_loop.exit()`
- WebViewBuilder comment updated to pass `principal_id` and `audit_log` into the delegate
- New test `audit_log_records_grants_and_denials`: writes 2 grants + 1 denial, asserts chain valid, asserts counts correct

**Verified:**
- `cargo check -p ferrite-servo` — zero warnings
- `cargo test -p ferrite-servo` — 2 tests, ok

---

### `ServoShell` — broker integration + network interception (2026-03-25)

**File:** `crates/ferrite-servo/src/shell.rs`
**File:** `crates/ferrite-servo/Cargo.toml`

**Dependencies added:**
```toml
ferrite-capability-broker = { path = "../ferrite-capability-broker" }
uuid = { version = "1", features = ["v4"] }
```

**What changed:**
- `CapabilityBroker` added to `ServoShell` behind `Rc<RefCell<>>` for shared interior-mutable access
- On `ServoShell::new()`, a `NetworkFetch` token scoped to `"*"` (wildcard) with 3600s TTL is minted for the `"servo-engine"` principal — the default-allow grant
- `ServoShell::broker()` and `ServoShell::network_token_id()` accessors added for external revoke/check
- `AppHandler` gains `broker` and `network_token_id` fields (with `#[cfg_attr(not(feature="servo"), allow(dead_code))]` suppression)
- `FerriteWebViewDelegate` now holds `broker: Rc<RefCell<CapabilityBroker>>` and `network_token_id: Uuid`
- `load_web_resource` implemented on `FerriteWebViewDelegate` — the Servo v0.0.5 fetch interception hook:
  - Fires for every outgoing request (navigation, sub-resource, XHR, fetch())
  - Calls `broker.borrow().check(token_id, url)` → `Granted` drops load (proceeds) or `Denied` intercepts + cancels (blocks)
  - On block: `println!("[ferrite] BLOCKED: {} reason: {:?}", url, reason)`
- Unit test `revoke_blocks_subsequent_requests`: mints wildcard token → check returns Granted → revoke → check returns Denied

**API finding documented in comments:**
- Servo v0.0.5 exposes `WebViewDelegate::load_web_resource` as the sole fetch interception point
- No lower-level `ResourceThread` hook is exposed — all interception goes through this delegate method

**Verified:**
- `cargo check -p ferrite-servo` — zero warnings
- `cargo test -p ferrite-servo` — 1 test, ok

---

### `ServoShell` — event loop improvements + load callback (2026-03-25)

**File:** `crates/ferrite-servo/src/shell.rs`

**What changed:**
- Initial URL updated from `about:blank` → `https://example.com` in the WebViewBuilder comment block (authoritative when RenderingContext is wired in Block 4)
- `notify_load_status_changed` added to `FerriteWebViewDelegate` — detects `LoadStatus::Complete` and prints `[ferrite] page load complete: <url>`
- `WindowEvent::KeyboardInput` arm added to `window_event()` — Escape key (pressed, non-repeat) calls `event_loop.exit()`
- `about_to_wait()` added to `AppHandler` — calls `servo.spin_event_loop()` on every winit event batch (equivalent to old `MainEventsCleared`), ensuring Servo makes progress even when no redraw is requested
- New imports: `winit::event::ElementState`, `winit::keyboard::{Key, NamedKey}`

**Verified:** `cargo check -p ferrite-servo` — clean, no warnings.

---

## What To Do Next (pick up here after plan refreshes)

1. **Build hello-ext Wasm** — requires `rustup target add wasm32-unknown-unknown`. Once available:
   ```
   cd extensions/hello-ext
   cargo build --target wasm32-unknown-unknown --release
   ```
   Then `cargo run -p ferrite-shell sandbox` will execute the full `run_demo()` flow.

2. **Enable servo feature** — dependency now correctly specified (`libservo`). Run `cargo build -p ferrite-servo --features servo` and fix any remaining compile errors (RenderingContext setup for WebViewBuilder).

3. **Block 4 RenderingContext** — wire `WindowRenderingContext::new(display_handle, window_handle, size)` into `WebViewBuilder` so Servo actually renders pages in the winit window.

---

## Known Issues / Notes

- `check.txt`, `check2.txt`, `check_output.txt` exist in `Browser/` root — scratch files from earlier testing. Consider deleting.
- Rate limiting is tracked on `CapabilityToken` via `rate_limit: Option<u32>` but not enforced in `CapabilityBroker::check()` yet — intentional, enforcement comes in Month 3.
- Servo dependency resolved: package is `libservo` (not `servo`). The servo repo root is a workspace-only manifest; the embedding library is at `components/servo/` with package name `libservo` and lib name `servo` (so Rust imports use `use servo::...`). Dep specified in `ferrite-servo/Cargo.toml` as `libservo = { git = "...", tag = "v0.0.5", optional = true }`, enabled via `--features servo`. Remaining work: wire `RenderingContext` for `WebViewBuilder` (Block 4).
- `rusqlite` upgraded from `0.31` → `0.37` in `ferrite-audit-log` to resolve `libsqlite3-sys` link conflict with `libservo` (which requires `rusqlite ^0.37`).
