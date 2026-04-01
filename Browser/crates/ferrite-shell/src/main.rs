use ferrite_servo::session::HeadlessServoSession;
use ferrite_servo::shell::ServoShell;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "ui" => ferrite_ui::launch().expect("Ferrite UI exited with error"),
        "window" => ServoShell::new().run(),
        "jstest" => run_js_compat_test(),
        _ => run_smoke_test(),
    }
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

fn run_smoke_test() {
    use ferrite_audit_log::{AuditEventKind, PersistentAuditLog};
    use uuid::Uuid;

    let db_path = std::env::temp_dir()
        .join("ferrite_smoke_test.db")
        .to_string_lossy()
        .into_owned();

    let mut audit_log =
        PersistentAuditLog::new(&db_path).expect("failed to open audit log");

    let principal_id = Uuid::new_v4();

    audit_log
        .append(
            AuditEventKind::CapabilityGranted,
            principal_id,
            Some("network.fetch".to_string()),
            Some("https://example.com".to_string()),
        )
        .expect("audit append failed");

    audit_log
        .append(
            AuditEventKind::CapabilityDenied,
            principal_id,
            Some("network.fetch".to_string()),
            Some("https://blocked.example".to_string()),
        )
        .expect("audit append failed");

    assert!(
        audit_log.log.verify_chain(),
        "audit chain integrity check failed"
    );

    println!(
        "[ferrite] audit chain verified: {} entries",
        audit_log.log.entries.len()
    );
    println!("Smoke test: ALL CHECKS PASSED");
}
