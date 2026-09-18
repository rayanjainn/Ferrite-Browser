//! Full action-suite conformance tests against [`MockEngine`] — the
//! directive's exit gate #1 for A9/T-109: every `BrowserEngine` trait
//! method exercised at least once, origin-tracking asserted (not just that
//! a call succeeds), `js_execute`'s special status noted, and
//! `cookies_read`/`storage_read`'s scoping actually shown to restrict what
//! comes back.

use ferrite_core::Origin;
use ferrite_engine::{
    conformance, BrowserEngine, Cookie, DomNode, DomSnapshot, ElementHandle, EngineError,
    MockEngine, TabId, WaitCondition,
};

fn origin(url: &str) -> Origin {
    Origin::parse(url).expect("fixture origin must be http(s)")
}

// ── Shared, engine-agnostic assertions (see `ferrite_engine::conformance`) ──

#[test]
fn shared_navigate_updates_current_url_and_origin() {
    let mut engine = MockEngine::new();
    conformance::navigate_updates_current_url_and_origin(&mut engine, "https://a.example/page");
}

#[test]
fn shared_unknown_tab_id_is_a_typed_error() {
    let mut engine = MockEngine::new();
    conformance::unknown_tab_id_is_a_typed_error(&mut engine);
}

#[test]
fn shared_wait_idle_completes() {
    let mut engine = MockEngine::new();
    conformance::wait_idle_completes(&mut engine);
}

// ── Navigation: navigate / go_back / go_forward / reload / current_url ──

#[test]
fn navigate_go_back_go_forward_and_reload_track_origin_through_history() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    let b = origin("https://b.example/");

    let (_, o) = engine.navigate("https://a.example/").unwrap();
    assert_eq!(o, a);

    let (_, o) = engine.navigate("https://b.example/").unwrap();
    assert_eq!(o, b);

    let (_, o) = engine.go_back().unwrap();
    assert_eq!(o, a, "go_back must return to the prior origin");

    let (_, o) = engine.go_forward().unwrap();
    assert_eq!(o, b, "go_forward must return to the origin it left");

    let (_, o) = engine.reload().unwrap();
    assert_eq!(o, b, "reload does not change the origin");
}

#[test]
fn go_back_past_the_start_of_history_is_a_typed_error() {
    let mut engine = MockEngine::new();
    assert!(matches!(engine.go_back(), Err(EngineError::Internal(_))));
}

#[test]
fn navigating_again_truncates_forward_history() {
    let mut engine = MockEngine::new();
    engine.navigate("https://a.example/").unwrap();
    engine.navigate("https://b.example/").unwrap();
    engine.go_back().unwrap();
    // Forward history to b.example is now discarded by this new navigation.
    engine.navigate("https://c.example/").unwrap();
    assert!(matches!(engine.go_forward(), Err(EngineError::Internal(_))));
}

// ── dom_snapshot / query / read_text ──

#[test]
fn dom_snapshot_returns_the_seeded_tree_for_the_current_origin() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    engine.navigate(a.as_str()).unwrap();
    engine.seed_dom_snapshot(
        &a,
        DomSnapshot {
            root: DomNode {
                role: "document".into(),
                label: Some("Example".into()),
                ..DomNode::default()
            },
        },
    );

    let (snap, o) = engine.dom_snapshot().unwrap();
    assert_eq!(o, a);
    assert_eq!(snap.root.role, "document");
    assert_eq!(snap.root.label.as_deref(), Some("Example"));
}

#[test]
fn query_resolves_seeded_selectors_scoped_to_the_current_origin() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    let b = origin("https://b.example/");
    engine.navigate(a.as_str()).unwrap();
    engine.seed_query(
        &a,
        "#button",
        vec![ElementHandle {
            selector: "#button".into(),
            role: Some("button".into()),
            text: Some("Go".into()),
        }],
    );

    let (handles, o) = engine.query("#button").unwrap();
    assert_eq!(o, a);
    assert_eq!(handles.len(), 1);
    assert_eq!(handles[0].role.as_deref(), Some("button"));

    // Same selector at a *different* origin is not the same query result —
    // proves query results are origin-scoped, not global by selector text.
    engine.navigate(b.as_str()).unwrap();
    let (handles, _) = engine.query("#button").unwrap();
    assert!(
        handles.is_empty(),
        "unseeded selector at a new origin must resolve to nothing"
    );
}

#[test]
fn read_text_of_an_unseeded_selector_is_element_not_found() {
    let mut engine = MockEngine::new();
    let err = engine.read_text("#missing").unwrap_err();
    assert!(matches!(err, EngineError::ElementNotFound(sel) if sel == "#missing"));
}

#[test]
fn dom_response_queue_serves_each_entry_then_repeats_the_last() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    engine.navigate(a.as_str()).unwrap();
    engine.seed_read_text(&a, "#status", "loading");
    engine.seed_read_text(&a, "#status", "done");

    assert_eq!(engine.read_text("#status").unwrap().0, "loading");
    assert_eq!(engine.read_text("#status").unwrap().0, "done");
    assert_eq!(
        engine.read_text("#status").unwrap().0,
        "done",
        "once the queue is down to one entry it repeats rather than erroring"
    );
}

// ── click / type_text / fill_form / select_option / scroll ──

#[test]
fn click_type_text_fill_form_select_option_and_scroll_all_report_the_current_origin() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    engine.navigate(a.as_str()).unwrap();

    assert_eq!(engine.click("#button").unwrap().1, a);
    assert_eq!(engine.type_text("#input", "hello").unwrap().1, a);
    assert_eq!(
        engine
            .fill_form(&[("#a".to_string(), "1".to_string())])
            .unwrap()
            .1,
        a
    );
    assert_eq!(engine.select_option("#select", "b").unwrap().1, a);
    assert_eq!(engine.scroll(0, 100).unwrap().1, a);
}

// ── wait_for ──

#[test]
fn wait_for_selector_succeeds_once_seeded_and_times_out_otherwise() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    engine.navigate(a.as_str()).unwrap();

    assert!(matches!(
        engine.wait_for(WaitCondition::Selector("#late".into())),
        Err(EngineError::WaitTimedOut)
    ));

    engine.seed_query(
        &a,
        "#late",
        vec![ElementHandle {
            selector: "#late".into(),
            role: None,
            text: None,
        }],
    );
    assert!(engine
        .wait_for(WaitCondition::Selector("#late".into()))
        .is_ok());
}

#[test]
fn wait_for_timeout_completes_without_a_real_sleep() {
    // R8/no-real-time-in-tests: MockEngine must not actually block for the
    // requested duration.
    let mut engine = MockEngine::new();
    let started = std::time::Instant::now();
    engine
        .wait_for(WaitCondition::Timeout(std::time::Duration::from_secs(30)))
        .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

// ── screenshot / download ──

#[test]
fn screenshot_returns_the_configured_frame_and_current_origin() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    engine.navigate(a.as_str()).unwrap();
    engine.set_screenshot(2, 2, vec![1, 2, 3, 4]);

    let ((w, h, bytes), o) = engine.screenshot().unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(bytes, vec![1, 2, 3, 4]);
    assert_eq!(o, a);
}

#[test]
fn download_reports_the_seeded_path_and_current_origin() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    engine.navigate(a.as_str()).unwrap();
    engine.seed_download("https://a.example/report.pdf", "/tmp/report.pdf");

    let (path, o) = engine.download("https://a.example/report.pdf").unwrap();
    assert_eq!(path, "/tmp/report.pdf");
    assert_eq!(o, a);
}

// ── tabs: open / close / switch ──

#[test]
fn open_close_and_switch_tab_move_the_active_origin() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    let b = origin("https://b.example/");
    engine.navigate(a.as_str()).unwrap();

    let (tab_b, o) = engine.open_tab(Some(b.as_str())).unwrap();
    assert_eq!(
        o, b,
        "opening a tab with a URL makes it active at that origin"
    );

    engine.switch_tab(TabId(0)).unwrap();
    assert_eq!(engine.current_url().unwrap().1, a);

    engine.switch_tab(tab_b).unwrap();
    assert_eq!(engine.current_url().unwrap().1, b);

    let (_, o) = engine.close_tab(tab_b).unwrap();
    assert_eq!(
        o, a,
        "closing the active tab falls back to a remaining tab, here tab 0's origin"
    );
}

#[test]
fn open_tab_with_no_url_starts_at_the_mock_home_origin() {
    let mut engine = MockEngine::new();
    let (_, o) = engine.open_tab(None).unwrap();
    assert_eq!(o, origin(ferrite_engine::MOCK_HOME));
}

// ── cookies_read / storage_read: scoping actually restricts what comes back ──

#[test]
fn cookies_read_is_scoped_and_does_not_leak_other_origins_cookies() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    let b = origin("https://b.example/");
    engine.seed_cookie(
        &a,
        Cookie {
            name: "session".into(),
            value: "a-secret".into(),
        },
    );
    engine.seed_cookie(
        &b,
        Cookie {
            name: "session".into(),
            value: "b-secret".into(),
        },
    );

    let (a_cookies, _) = engine.cookies_read(&a).unwrap();
    assert_eq!(a_cookies.len(), 1);
    assert_eq!(a_cookies[0].value, "a-secret");

    let (b_cookies, _) = engine.cookies_read(&b).unwrap();
    assert_eq!(b_cookies.len(), 1);
    assert_eq!(b_cookies[0].value, "b-secret");

    let unrelated = origin("https://c.example/");
    let (c_cookies, _) = engine.cookies_read(&unrelated).unwrap();
    assert!(
        c_cookies.is_empty(),
        "an origin with no seeded cookies must get none, not every origin's cookies"
    );
}

#[test]
fn storage_read_is_scoped_and_does_not_leak_other_origins_storage() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");
    let b = origin("https://b.example/");
    engine.seed_storage(&a, "token", "a-token");
    engine.seed_storage(&b, "token", "b-token");

    let (a_kv, _) = engine.storage_read(&a).unwrap();
    assert_eq!(a_kv, vec![("token".to_string(), "a-token".to_string())]);

    let (b_kv, _) = engine.storage_read(&b).unwrap();
    assert_eq!(b_kv, vec![("token".to_string(), "b-token".to_string())]);
}

// ── clipboard ──

#[test]
fn clipboard_write_then_read_round_trips() {
    let mut engine = MockEngine::new();
    engine.clipboard_write("copied text").unwrap();
    let (text, _) = engine.clipboard_read().unwrap();
    assert_eq!(text, "copied text");
}

// ── js_execute: exercised, and its privileged status documented ──

#[test]
fn js_execute_returns_the_seeded_result_for_the_exact_script() {
    let mut engine = MockEngine::new();
    engine.navigate("https://a.example/").unwrap();
    engine.seed_js("document.title", Ok("Example Domain".to_string()));

    let (result, origin_now) = engine.js_execute("document.title").unwrap();
    assert_eq!(result, "Example Domain");
    assert_eq!(origin_now, origin("https://a.example/"));

    // js_execute is on the trait — the trait itself does not gate it; that
    // is `ferrite-ipi::comparator`'s job upstream (see the crate's module
    // docs). This test asserts only that the method runs, not that it is
    // "safe" to call — no code path in this crate makes that claim.
}

#[test]
fn js_execute_failure_is_reported_not_panicked() {
    let mut engine = MockEngine::new();
    engine.seed_js(
        "throw 1",
        Err("ReferenceError: x is not defined".to_string()),
    );
    let err = engine.js_execute("throw 1").unwrap_err();
    assert!(matches!(err, EngineError::Internal(_)));
}

// ── Opaque origins: a real, typed outcome, not a panic ──

#[test]
fn navigating_to_an_opaque_scheme_reports_a_typed_error_not_a_fabricated_origin() {
    let mut engine = MockEngine::new();
    // navigate() itself must resolve the new origin to report it (the
    // directive's "every action returns (result, origin)" contract), so
    // the opaque-scheme failure surfaces immediately, not on some later
    // call.
    let err = engine.navigate("data:text/plain,hello").unwrap_err();
    assert!(matches!(err, EngineError::OpaqueOrigin(_)));
    // The tab's history still recorded the navigation attempt (state
    // change is not rolled back just because the origin can't be
    // reported) — the next *http(s)* navigation resolves normally.
    let (_, o) = engine.navigate("https://a.example/").unwrap();
    assert_eq!(o, origin("https://a.example/"));
}

// ── Call log: every action is actually recorded ──

#[test]
fn every_trait_method_is_visible_in_the_call_log() {
    let mut engine = MockEngine::new();
    let a = origin("https://a.example/");

    let _ = engine.navigate(a.as_str());
    let _ = engine.go_back();
    let _ = engine.go_forward();
    let _ = engine.reload();
    let _ = engine.current_url();
    let _ = engine.dom_snapshot();
    let _ = engine.query("#x");
    let _ = engine.read_text("#x");
    let _ = engine.click("#x");
    let _ = engine.type_text("#x", "y");
    let _ = engine.fill_form(&[]);
    let _ = engine.select_option("#x", "y");
    let _ = engine.scroll(0, 0);
    let _ = engine.wait_for(WaitCondition::Idle);
    let _ = engine.screenshot();
    let _ = engine.download("https://a.example/f");
    let (tab, _) = engine.open_tab(None).unwrap();
    let _ = engine.switch_tab(TabId(0));
    let _ = engine.close_tab(tab);
    let _ = engine.cookies_read(&a);
    let _ = engine.storage_read(&a);
    let _ = engine.clipboard_read();
    let _ = engine.clipboard_write("z");
    let _ = engine.js_execute("1+1");

    let calls = engine.calls();
    // 24 distinct trait methods called above, each pushed exactly once.
    assert_eq!(
        calls.len(),
        24,
        "every call above must be logged exactly once: {calls:?}"
    );
}
