use regorus::Engine;
use serde_json::json;

const DEFAULT_POLICY: &str = r#"
package ferrite.capability

default allow = true
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
    fn default_policy_allows_all() {
        let mut engine = PolicyEngine::new();
        assert!(engine.evaluate("extension", "dom.read", "https://example.com"));
        assert!(engine.evaluate("agent", "network.fetch", "https://other.com"));
    }
}
