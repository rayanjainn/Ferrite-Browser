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
Exit condition: `cargo run --bin ferrite-shell window` opens a blank window titled "Ferrite Browser" and closes cleanly when you press the 'X' button.![Result](image.png)

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
Exit condition: `cargo run -p ferrite-shell window` opens a window, Servo initialises without panicking (check terminal — no crash), and the window renders (even if blank/white).![Result](/output_images/image-1.png)

### Block 4: Navigate to a real URL
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

### Block 5: Wire the capability broker to Servo's network requests
What it does: Intercepts outgoing network requests from Servo and routes them through `CapabilityBroker::check()` before allowing them. Denied requests are blocked. This is the first real integration of the broker with the engine.

Prompt for Claude COde:
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

### Block 6: Log broker decisions to the audit log
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
### Block 1: `ferrite-ui` crate, Iced hello-world with dark theme
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

### Block 2: Tab bar component (add/close tabs, active tab highlights)
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

### Block 3: Address bar component (text input, URL submit on Enter)
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
### Block 1: `ferrite-sandbox` crate, Extism setup, Wasm module loading
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

### Block 2: Host functions for `dom.read`, `network.fetch`, `storage.read`
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

### Block 3: Demo extension: Wasm module that calls `host_dom_read` and receives a response
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
### Depends on Iced UI being done first (Single Block)