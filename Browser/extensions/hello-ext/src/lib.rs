// hello-ext — Ferrite extension for sandbox smoke-testing.
//
// Exports:
//   ping              — returns "pong"  (basic load test)
//   test_dom_read     — calls host_dom_read("h1")
//   test_network_fetch— calls host_network_fetch("https://example.com/api")
//   test_storage_read — calls host_storage_read("user_prefs")
//   test_js_execute   — calls host_js_execute("1 + 1")
//
// Build:
//   cargo build --target wasm32-unknown-unknown --release
//
// Output:
//   target/wasm32-unknown-unknown/release/hello_ext.wasm

#![no_main]

use extism_pdk::*;

// ---------------------------------------------------------------------------
// Host function imports
//
// The namespace "extism:host/user" is the default namespace extism registers
// host functions under when no explicit namespace is set on Function::new().
// Each declaration must match the name registered on the host side exactly.
// ---------------------------------------------------------------------------
#[host_fn("extism:host/user")]
extern "ExtismHost" {
    fn host_dom_read(selector: String) -> String;
    fn host_network_fetch(url: String) -> String;
    fn host_storage_read(key: String) -> String;
    fn host_js_execute(script: String) -> String;
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

/// Smoke-test: returns "pong".
#[plugin_fn]
pub fn ping(_input: ()) -> FnResult<String> {
    Ok("pong".to_string())
}

/// Calls `host_dom_read("h1")` and returns the host's response.
#[plugin_fn]
pub fn test_dom_read(_input: ()) -> FnResult<String> {
    let result = unsafe { host_dom_read("h1".to_string())? };
    Ok(result)
}

/// Calls `host_network_fetch("https://example.com/api")` and returns the
/// host's response.
#[plugin_fn]
pub fn test_network_fetch(_input: ()) -> FnResult<String> {
    let result = unsafe { host_network_fetch("https://example.com/api".to_string())? };
    Ok(result)
}

/// Calls `host_storage_read("user_prefs")` and returns the host's response.
#[plugin_fn]
pub fn test_storage_read(_input: ()) -> FnResult<String> {
    let result = unsafe { host_storage_read("user_prefs".to_string())? };
    Ok(result)
}

/// Calls `host_js_execute("1 + 1")` and returns the host's response.
///
/// This exercises the High-risk `JsExecute` capability path through the broker.
#[plugin_fn]
pub fn test_js_execute(_input: ()) -> FnResult<String> {
    let result = unsafe { host_js_execute("1 + 1".to_string())? };
    Ok(result)
}
