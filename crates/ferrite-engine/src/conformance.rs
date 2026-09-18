//! Engine-agnostic assertions shared between `MockEngine`'s own test suite
//! (`tests/conformance.rs`, always run) and `ferrite-engine-servo`'s
//! `ServoEngine` tests (feature `engine-servo`, `#[ignore]`d unless a real
//! Servo build is present).
//!
//! This is a genuinely small, shared subset — not the entire action
//! surface — by design. `MockEngine` can be scripted with exact DOM
//! content, cookies, and JS results with no page ever having existed;
//! `ServoEngine` needs a real page actually loaded (over real, if only
//! loopback, HTTP) before `dom_snapshot`/`query`/`cookies_read` mean
//! anything. Forcing both through one parametrized fixture trait would cost
//! more engineering than this charter's remaining budget justifies for the
//! genuinely-overlapping behavior, so the two engines' richer,
//! implementation-specific scripts live in their own crates' test files —
//! see `docs/handoffs/a09.md` for the reasoning. What lives here is what is
//! true of *every* `BrowserEngine`, regardless of implementation:
//! navigation updates the reported origin, an unknown tab id is a real
//! error (not a panic), and an idle wait completes without blocking
//! forever.

use ferrite_core::Origin;

use crate::{BrowserEngine, EngineError, TabId, WaitCondition};

/// Navigating to `url` makes `current_url()` report it and both calls agree
/// on `url`'s origin.
pub fn navigate_updates_current_url_and_origin<E: BrowserEngine>(engine: &mut E, url: &str) {
    let expected_origin = Origin::parse(url).expect("test fixture URL must be http(s)");

    let (_, nav_origin) = engine.navigate(url).expect("navigate must succeed");
    assert_eq!(
        nav_origin, expected_origin,
        "navigate's reported origin must match the URL navigated to"
    );

    let (reported_url, read_origin) = engine.current_url().expect("current_url must succeed");
    assert_eq!(
        reported_url, url,
        "current_url must reflect the last navigation"
    );
    assert_eq!(
        read_origin, expected_origin,
        "current_url's reported origin must match navigate's"
    );
}

/// Closing/switching to a tab id that was never opened is a typed error,
/// never a panic, for every `BrowserEngine` implementation.
pub fn unknown_tab_id_is_a_typed_error<E: BrowserEngine>(engine: &mut E) {
    let bogus = TabId(u64::MAX);
    assert!(
        matches!(engine.close_tab(bogus), Err(EngineError::NoSuchTab(_))),
        "closing an unopened tab must be EngineError::NoSuchTab, not a panic"
    );
    assert!(
        matches!(engine.switch_tab(bogus), Err(EngineError::NoSuchTab(_))),
        "switching to an unopened tab must be EngineError::NoSuchTab, not a panic"
    );
}

/// `wait_for(Idle)` completes without blocking indefinitely, for every
/// `BrowserEngine` implementation (a page that never loads anything is, by
/// definition, idle).
pub fn wait_idle_completes<E: BrowserEngine>(engine: &mut E) {
    engine
        .wait_for(WaitCondition::Idle)
        .expect("wait_for(Idle) must complete, not hang or error");
}
