use ferrite_audit_log::{AuditEventKind, PersistentAuditLog};
use ferrite_capability_broker::{
    BrokerDecision, CapabilityBroker, CapabilityType, DenialReason, Principal, PrincipalKind,
};
use ferrite_policy::PolicyEngine;
use ferrite_servo::session::HeadlessServoSession;
use ferrite_servo::shell::ServoShell;
use uuid::Uuid;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "ui" => ferrite_ui::launch().expect("Ferrite UI exited with error"),
        "window" => ServoShell::new().run(),
        "sandbox" => run_sandbox_smoke_test(),
        "jstest" => run_js_compat_test(),
        _ => run_smoke_test(),
    }
}

fn run_sandbox_smoke_test() {
    // Resolve the hello-ext Wasm path relative to the workspace root.
    // When run via `cargo run -p ferrite-shell`, the working directory is the
    // workspace root (Browser/), so this path resolves correctly.
    let wasm_path = "extensions/hello-ext/target/wasm32-unknown-unknown/release/hello_ext.wasm";

    println!("[ferrite] loading extension from: {}", wasm_path);

    let mut sandbox = ferrite_sandbox::ExtensionSandbox::new(wasm_path)
        .expect("failed to load hello-ext Wasm module — build it first:\n  cd extensions/hello-ext && cargo build --target wasm32-unknown-unknown --release");

    sandbox.run_demo();
}

fn run_smoke_test() {
    // 1. Create CapabilityBroker
    let mut broker = CapabilityBroker::new();

    // 2. Create PersistentAuditLog at temp dir
    let db_path = std::env::temp_dir().join("ferrite_smoke_test.db");
    let mut audit_log =
        PersistentAuditLog::new(db_path.to_str().unwrap()).expect("failed to open audit log");

    // 3. Create PolicyEngine
    let mut policy = PolicyEngine::new();

    // 4. Mint token: Extension principal, NetworkFetch, "https://example.com", no rate limit, 3600s
    let principal = Principal {
        id: Uuid::new_v4(),
        kind: PrincipalKind::Extension,
        label: "smoke-test-ext".to_string(),
    };
    let principal_id = principal.id;

    let token_id = broker.mint_token(
        principal,
        CapabilityType::NetworkFetch,
        "https://example.com".to_string(),
        vec![],
        None,
        3600,
    );

    // 5. Assert policy allows extension network.fetch on example.com
    assert!(
        policy.evaluate("extension", "network.fetch", "https://example.com"),
        "policy should allow extension network.fetch on https://example.com"
    );

    // 6. Assert broker grants for matching origin
    match broker.check(token_id, "https://example.com/path") {
        BrokerDecision::Granted { .. } => {}
        BrokerDecision::Denied { reason } => {
            panic!("expected Granted, got Denied({:?})", reason);
        }
    }

    // 7. Assert broker denies for wrong origin with OriginMismatch
    match broker.check(token_id, "https://evil.com") {
        BrokerDecision::Denied {
            reason: DenialReason::OriginMismatch,
        } => {}
        other => panic!("expected Denied(OriginMismatch), got {:?}", other),
    }

    // 8. Append CapabilityGranted event to audit log
    audit_log
        .append(
            AuditEventKind::CapabilityGranted,
            principal_id,
            Some("network.fetch".to_string()),
            Some("https://example.com".to_string()),
        )
        .expect("audit append failed");

    // 9. Assert audit chain is valid
    assert!(
        audit_log.log.verify_chain(),
        "audit chain integrity check failed"
    );

    // 10. All checks passed
    println!("Month 1-2 smoke test: ALL CHECKS PASSED");
}

fn run_js_compat_test() {
    const URLS: &[&str] = &[
        "https://example.com",
        "https://lite.duckduckgo.com",
        "https://doc.rust-lang.org",
    ];

    let mut session = match HeadlessServoSession::new(1280, 800) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[jstest] Failed to create Servo session: {}", e);
            eprintln!("[jstest] Servo feature may not be enabled — build with:");
            eprintln!("  cargo run -p ferrite-shell --features ferrite-servo/servo -- jstest");
            std::process::exit(1);
        }
    };

    let mut results = Vec::new();

    for url in URLS {
        print!("[jstest] Probing {} ... ", url);
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let result = session.test_js_compat(url);
        println!(
            "js={} title={:?} errors={}",
            result.js_executed,
            result.page_title.as_deref().unwrap_or("—"),
            result.console_errors.len()
        );
        results.push(result);
    }

    // ── Print results table ────────────────────────────────────────────────
    println!();
    println!(
        "{:<42} {:<12} {:<40} {}",
        "URL", "JS Executed", "Title", "Errors"
    );
    println!("{}", "-".repeat(110));
    for r in &results {
        let title = r.page_title.as_deref().unwrap_or("—");
        let title_truncated = if title.len() > 38 {
            format!("{}…", &title[..37])
        } else {
            title.to_string()
        };
        let errors_summary = if r.console_errors.is_empty() {
            "none".to_string()
        } else {
            format!("{}: {}", r.console_errors.len(), r.console_errors[0].chars().take(40).collect::<String>())
        };
        println!(
            "{:<42} {:<12} {:<40} {}",
            r.url,
            if r.js_executed { "yes" } else { "no" },
            title_truncated,
            errors_summary,
        );
    }
    println!();
    println!("JS compat baseline complete");

    // ── Save CSV ───────────────────────────────────────────────────────────
    let csv_dir = std::path::Path::new("../paper/data");
    if let Err(e) = std::fs::create_dir_all(csv_dir) {
        eprintln!("[jstest] Warning: could not create {}: {}", csv_dir.display(), e);
        return;
    }
    let csv_path = csv_dir.join("js_compat_baseline.csv");
    let mut csv = String::from("url,js_executed,page_title,error_count,first_error\n");
    for r in &results {
        let title = r.page_title.as_deref().unwrap_or("").replace('"', "\"\"");
        let first_error = r
            .console_errors
            .first()
            .map(|e| e.replace('"', "\"\""))
            .unwrap_or_default();
        csv.push_str(&format!(
            "\"{}\",{},\"{}\",{},\"{}\"\n",
            r.url,
            r.js_executed,
            title,
            r.console_errors.len(),
            first_error,
        ));
    }
    match std::fs::write(&csv_path, &csv) {
        Ok(_) => println!("[jstest] Results saved to {}", csv_path.display()),
        Err(e) => eprintln!("[jstest] Warning: could not write CSV: {}", e),
    }
}
