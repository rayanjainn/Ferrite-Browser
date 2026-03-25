// ferrite-sandbox — Wasm extension sandbox using Extism.
//
// ## Extism 1.x host function API
//
// Host functions are defined with the `host_fn!` macro:
//
//   host_fn!(fn_name(user_data: StateType; arg: ArgType) -> RetType { body })
//
// The macro generates a function with signature:
//   fn(plugin: &mut CurrentPlugin, inputs: &[Val], outputs: &mut [Val],
//      user_data: UserData<StateType>) -> Result<(), Error>
//
// It auto-decodes each `arg` from the Wasm memory via `memory_get_val` and
// encodes the return value back via `memory_new` + `memory_to_val`.  The
// Wasm ABI for string arguments is a single PTR (ValType::I64) that points
// into the Extism shared memory block.
//
// Functions are registered on `Plugin::new` as `Function::new(name, [PTR],
// [PTR], user_data, fn_ptr)`.  Host functions default to the namespace
// `"extism:host/user"` (EXTISM_USER_MODULE), which is the same namespace
// declared on the guest side with `#[host_fn("extism:host/user")]`.
//
// ## Shared state design
//
// `SandboxState` bundles the `CapabilityBroker`, three token IDs, and the
// `PersistentAuditLog` into a single type wrapped in `UserData<SandboxState>`
// (`Arc<Mutex<T>>`).  All three host functions clone the same `UserData` and
// lock it per-call, giving them access to both the broker and the audit log
// without a second synchronisation primitive.

use std::sync::{Arc, Mutex};

use extism::{Function, Manifest, Plugin, UserData, Wasm, PTR};
use ferrite_audit_log::{AuditEventKind, PersistentAuditLog};
use ferrite_capability_broker::{
    BrokerDecision, CapabilityBroker, CapabilityType, Principal, PrincipalKind,
};
use thiserror::Error;
use uuid::Uuid;

/// Errors produced by the extension sandbox.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// The Wasm file could not be read or the plugin could not be instantiated.
    #[error("failed to load extension: {0}")]
    LoadFailed(String),

    /// A call into the Wasm plugin failed (export missing, trap, bad output).
    #[error("extension call failed: {0}")]
    CallFailed(String),

    /// The extension attempted to use a capability it has not been granted.
    #[error("capability denied: {0}")]
    CapabilityDenied(String),
}

// ---------------------------------------------------------------------------
// Shared state across all host functions
// ---------------------------------------------------------------------------

/// All mutable state shared between the sandbox and its host functions.
///
/// Wrapped in `UserData<SandboxState>` so every host function gets an
/// `Arc<Mutex<SandboxState>>` — one lock covers both the broker and the audit
/// log, keeping the two always consistent.
struct SandboxState {
    broker: CapabilityBroker,
    audit_log: PersistentAuditLog,
    principal_id: Uuid,
    dom_read_token: Uuid,
    network_fetch_token: Uuid,
    storage_read_token: Uuid,
}

// ---------------------------------------------------------------------------
// Host functions (defined with extism's host_fn! macro)
// ---------------------------------------------------------------------------

extism::host_fn!(host_dom_read(user_data: SandboxState; selector: String) -> String {
    let state: Arc<Mutex<SandboxState>> = user_data.get()?;
    let mut state = state.lock().unwrap();
    let principal_id = state.principal_id;
    match state.broker.check(state.dom_read_token, "*") {
        BrokerDecision::Granted { .. } => {
            let _ = state.audit_log.append(
                AuditEventKind::CapabilityGranted,
                principal_id,
                Some("dom.read".to_string()),
                Some(selector.clone()),
            );
            Ok(format!("<div>stub DOM content for selector: {}</div>", selector))
        }
        BrokerDecision::Denied { .. } => {
            let _ = state.audit_log.append(
                AuditEventKind::CapabilityDenied,
                principal_id,
                Some("dom.read".to_string()),
                Some(selector.clone()),
            );
            Ok("ERROR: dom.read denied".to_string())
        }
    }
});

extism::host_fn!(host_network_fetch(user_data: SandboxState; url: String) -> String {
    let state: Arc<Mutex<SandboxState>> = user_data.get()?;
    let mut state = state.lock().unwrap();
    let principal_id = state.principal_id;
    match state.broker.check(state.network_fetch_token, &url) {
        BrokerDecision::Granted { .. } => {
            let _ = state.audit_log.append(
                AuditEventKind::CapabilityGranted,
                principal_id,
                Some("network.fetch".to_string()),
                Some(url.clone()),
            );
            Ok(format!("stub response body for: {}", url))
        }
        BrokerDecision::Denied { .. } => {
            let _ = state.audit_log.append(
                AuditEventKind::CapabilityDenied,
                principal_id,
                Some("network.fetch".to_string()),
                Some(url.clone()),
            );
            Ok("ERROR: network.fetch denied".to_string())
        }
    }
});

extism::host_fn!(host_storage_read(user_data: SandboxState; key: String) -> String {
    let state: Arc<Mutex<SandboxState>> = user_data.get()?;
    let mut state = state.lock().unwrap();
    let principal_id = state.principal_id;
    match state.broker.check(state.storage_read_token, "*") {
        BrokerDecision::Granted { .. } => {
            let _ = state.audit_log.append(
                AuditEventKind::CapabilityGranted,
                principal_id,
                Some("storage.read".to_string()),
                Some(key.clone()),
            );
            Ok(format!("stub value for key: {}", key))
        }
        BrokerDecision::Denied { .. } => {
            let _ = state.audit_log.append(
                AuditEventKind::CapabilityDenied,
                principal_id,
                Some("storage.read".to_string()),
                Some(key.clone()),
            );
            Ok("ERROR: storage.read denied".to_string())
        }
    }
});

// ---------------------------------------------------------------------------
// ExtensionSandbox
// ---------------------------------------------------------------------------

/// Wraps an Extism `Plugin` with a `CapabilityBroker`, a `PersistentAuditLog`,
/// and three pre-minted capability tokens.
///
/// Three host functions are registered in the Wasm import namespace
/// `"extism:host/user"`:
///
/// | Host function       | Capability    | Token field          |
/// |---------------------|---------------|----------------------|
/// | `host_dom_read`     | `DomRead`     | `dom_read_token`     |
/// | `host_network_fetch`| `NetworkFetch`| `network_fetch_token`|
/// | `host_storage_read` | `StorageRead` | `storage_read_token` |
///
/// Each token is scoped to origin `"*"` (wildcard) with a 3600-second TTL.
/// All broker decisions are appended to the audit log before the host function
/// returns its response string to the Wasm module.
pub struct ExtensionSandbox {
    plugin: Plugin,
    /// Shared state — also held inside each host function's `UserData`.
    state: UserData<SandboxState>,
    pub dom_read_token: Uuid,
    pub network_fetch_token: Uuid,
    pub storage_read_token: Uuid,
}

impl ExtensionSandbox {
    /// Load a Wasm extension from `wasm_path`, open the audit log at
    /// `$TMPDIR/ferrite_sandbox.db`, mint capability tokens, and register the
    /// three capability host functions.
    pub fn new(wasm_path: &str) -> Result<Self, SandboxError> {
        let bytes = std::fs::read(wasm_path)
            .map_err(|e| SandboxError::LoadFailed(format!("{}: {}", wasm_path, e)))?;

        // ── Audit log ──────────────────────────────────────────────────────
        let db_path = std::env::temp_dir()
            .join("ferrite_sandbox.db")
            .to_string_lossy()
            .into_owned();
        let audit_log = PersistentAuditLog::new(&db_path)
            .map_err(|e| SandboxError::LoadFailed(format!("audit log: {}", e)))?;

        // ── Mint tokens ────────────────────────────────────────────────────
        let mut broker = CapabilityBroker::new();
        let principal_id = Uuid::new_v4();

        let make_principal = |label: &str| Principal {
            id: principal_id,
            kind: PrincipalKind::Extension,
            label: label.to_string(),
        };

        let dom_read_token = broker.mint_token(
            make_principal("hello-ext"),
            CapabilityType::DomRead,
            "*".to_string(),
            vec![],
            None,
            3600,
        );
        let network_fetch_token = broker.mint_token(
            make_principal("hello-ext"),
            CapabilityType::NetworkFetch,
            "*".to_string(),
            vec![],
            None,
            3600,
        );
        let storage_read_token = broker.mint_token(
            make_principal("hello-ext"),
            CapabilityType::StorageRead,
            "*".to_string(),
            vec![],
            None,
            3600,
        );

        // ── Shared state ───────────────────────────────────────────────────
        let state = UserData::new(SandboxState {
            broker,
            audit_log,
            principal_id,
            dom_read_token,
            network_fetch_token,
            storage_read_token,
        });

        // ── Host function registrations ────────────────────────────────────
        //
        // params/results = [PTR] for string-in / string-out.
        // No explicit namespace → extism defaults to "extism:host/user".
        let f_dom_read = Function::new("host_dom_read", [PTR], [PTR], state.clone(), host_dom_read);
        let f_network_fetch = Function::new(
            "host_network_fetch",
            [PTR],
            [PTR],
            state.clone(),
            host_network_fetch,
        );
        let f_storage_read = Function::new(
            "host_storage_read",
            [PTR],
            [PTR],
            state.clone(),
            host_storage_read,
        );

        // ── Build plugin ───────────────────────────────────────────────────
        let wasm = Wasm::data(bytes);
        let manifest = Manifest::new([wasm]);
        let plugin = Plugin::new(
            manifest,
            [f_dom_read, f_network_fetch, f_storage_read],
            false,
        )
        .map_err(|e| SandboxError::LoadFailed(e.to_string()))?;

        Ok(Self {
            plugin,
            state,
            dom_read_token,
            network_fetch_token,
            storage_read_token,
        })
    }

    /// Run the capability demo sequence:
    ///
    /// 1. Call `test_dom_read`, `test_network_fetch`, `test_storage_read` —
    ///    all should be granted.
    /// 2. Revoke the `DomRead` token.
    /// 3. Call `test_dom_read` again — should return the denied error string.
    /// 4. Verify the audit chain and print a summary.
    ///
    /// Panics if any Wasm call fails unexpectedly (trap, missing export, etc.)
    /// — this is intentional for a smoke-test demo path.
    pub fn run_demo(&mut self) {
        println!("[ferrite-sandbox] --- capability demo ---");

        let dom = self
            .call_test("test_dom_read")
            .expect("test_dom_read failed");
        println!("[ferrite-sandbox] test_dom_read     → {}", dom);

        let net = self
            .call_test("test_network_fetch")
            .expect("test_network_fetch failed");
        println!("[ferrite-sandbox] test_network_fetch → {}", net);

        let stor = self
            .call_test("test_storage_read")
            .expect("test_storage_read failed");
        println!("[ferrite-sandbox] test_storage_read  → {}", stor);

        // Revoke the DomRead token through the shared state.
        {
            let arc = self.state.get().expect("sandbox state poisoned");
            let mut guard = arc.lock().unwrap();
            guard.broker.revoke(self.dom_read_token);
        }
        println!("[ferrite-sandbox] dom_read token revoked");

        let dom2 = self
            .call_test("test_dom_read")
            .expect("test_dom_read (post-revoke) failed");
        println!("[ferrite-sandbox] test_dom_read (post-revoke) → {}", dom2);

        // Verify audit chain and print summary.
        let arc = self.state.get().expect("sandbox state poisoned");
        let guard = arc.lock().unwrap();
        let entries = &guard.audit_log.log.entries;
        let chain_ok = guard.audit_log.log.verify_chain();

        assert!(chain_ok, "audit chain verification failed");

        println!(
            "[ferrite-sandbox] demo complete — {} audit entries, chain verified",
            entries.len()
        );
    }

    /// Call a named export that takes no input and returns a `String`.
    pub fn call_test(&mut self, export: &str) -> Result<String, SandboxError> {
        if !self.plugin.function_exists(export) {
            return Err(SandboxError::CallFailed(format!(
                "export '{}' not found in Wasm module",
                export
            )));
        }
        self.plugin
            .call::<(), String>(export, ())
            .map_err(|e| SandboxError::CallFailed(e.to_string()))
    }

    /// Call the `"ping"` export — smoke-tests basic extension loading.
    pub fn ping(&mut self) -> Result<String, SandboxError> {
        if !self.plugin.function_exists("ping") {
            return Err(SandboxError::CallFailed(
                "export 'ping' not found in Wasm module".to_string(),
            ));
        }
        self.plugin
            .call::<(), String>("ping", ())
            .map_err(|e| SandboxError::CallFailed(e.to_string()))
    }
}
