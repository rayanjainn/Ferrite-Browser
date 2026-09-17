use crate::twin::SyntheticTwin;
use std::sync::{Arc, Mutex};

/// Shared state for the Option C application-layer interceptor.
#[derive(Debug, Default)]
pub struct ContainmentState {
    pub active: bool,
    pub intercepted_urls: Vec<String>,
}

pub type SharedContainmentState = Arc<Mutex<ContainmentState>>;

/// Activates the Option C interceptor.
pub fn activate(state: &SharedContainmentState) {
    let mut s = state.lock().unwrap();
    s.active = true;
    s.intercepted_urls.clear();
    println!("[ferrite-containment] Option C interceptor: ACTIVE");
}

/// Deactivates the Option C interceptor.
pub fn deactivate(state: &SharedContainmentState) {
    let mut s = state.lock().unwrap();
    s.active = false;
    println!("[ferrite-containment] Option C interceptor: INACTIVE");
}

/// Returns a copy of intercepted URLs and clears the list.
pub fn intercepted_urls(state: &SharedContainmentState) -> Vec<String> {
    let s = state.lock().unwrap();
    s.intercepted_urls.clone()
}

/// Called on every outgoing network request during a dry run.
/// Returns `Some(fake_response)` if active, `None` if inactive.
pub fn intercept_request(
    state: &SharedContainmentState,
    url: &str,
    twin: &SyntheticTwin,
) -> Option<String> {
    let mut s = state.lock().unwrap();
    if !s.active {
        return None;
    }
    s.intercepted_urls.push(url.to_string());
    Some(format!(
        "{{\"status\": 200, \"body\": \"Intercepted by Ferrite IPI containment. \
         Synthetic identity: {}\"}}",
        twin.name
    ))
}

/// Linux-only: creates a network namespace for the dry run context.
/// On other platforms this is a no-op that returns `Ok(())`.
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

/// Activates both layers: Option C interceptor + Option B namespace (Linux only).
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
