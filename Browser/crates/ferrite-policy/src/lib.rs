use regorus::Engine;
use serde_json::json;

const DEFAULT_POLICY: &str = r#"
package ferrite.capability

default allow = false

# Low-risk read capabilities are allowed for all principal kinds.
allow if {
    input.capability == "dom_read"
}

# Network fetch and storage access are allowed for extensions and agents.
allow if {
    input.capability == "network_fetch"
    input.principal_kind in ["extension", "agent", "web_content"]
}
allow if {
    input.capability == "storage_read"
    input.principal_kind in ["extension", "agent"]
}
allow if {
    input.capability == "storage_write"
    input.principal_kind in ["extension", "agent"]
}
allow if {
    input.capability == "cookie_read"
    input.principal_kind in ["extension", "agent"]
}

# High-risk write capabilities require the extension or agent principal kind.
allow if {
    input.capability == "dom_write"
    input.principal_kind in ["extension", "agent"]
}
allow if {
    input.capability == "cookie_write"
    input.principal_kind in ["extension", "agent"]
}

# JS execution is High risk; only agents may request it.
# Step-up consent is enforced via the High risk classification in the broker.
allow if {
    input.principal_kind == "agent"
    input.capability == "js_execute"
}
"#;

/// Wraps a Regorus policy engine with a default allow-all Rego policy.
pub struct PolicyEngine {
    engine: Engine,
}

impl PolicyEngine {
    /// Creates a new `PolicyEngine` with the default `ferrite.capability` package loaded.
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine
            .add_policy("ferrite.capability".to_string(), DEFAULT_POLICY.to_string())
            .expect("default policy must parse");
        PolicyEngine { engine }
    }

    /// Evaluates whether `principal_kind` may use `capability` on `origin`.
    ///
    /// Returns `true` on any evaluation error (fail-open).
    pub fn evaluate(&mut self, principal_kind: &str, capability: &str, origin: &str) -> bool {
        let input = json!({
            "principal_kind": principal_kind,
            "capability": capability,
            "origin": origin,
        });

        let input_value = match regorus::Value::from_json_str(&input.to_string()) {
            Ok(v) => v,
            Err(_) => return true,
        };

        self.engine.set_input(input_value);

        match self
            .engine
            .eval_rule("data.ferrite.capability.allow".to_string())
        {
            Ok(regorus::Value::Bool(b)) => b,
            _ => true,
        }
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_allows_standard_capabilities() {
        let mut engine = PolicyEngine::new();
        assert!(engine.evaluate("extension", "dom_read", "https://example.com"));
        assert!(engine.evaluate("agent", "network_fetch", "https://other.com"));
        assert!(engine.evaluate("extension", "storage_read", "https://example.com"));
    }

    #[test]
    fn policy_allows_agent_js_execute() {
        let mut engine = PolicyEngine::new();
        assert!(engine.evaluate("agent", "js_execute", "*"));
    }

    #[test]
    fn policy_denies_extension_js_execute() {
        let mut engine = PolicyEngine::new();
        assert!(!engine.evaluate("extension", "js_execute", "*"));
    }
}
