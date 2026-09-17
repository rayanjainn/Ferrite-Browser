use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityType {
    DomRead,
    DomWrite,
    NetworkFetch,
    StorageRead,
    StorageWrite,
    CookieRead,
    CookieWrite,
    /// Execute arbitrary JavaScript in the active WebView — High risk.
    JsExecute,
}

/// Risk classification for a capability type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl CapabilityType {
    /// Returns the default risk level for this capability.
    pub fn classify_risk(&self) -> RiskLevel {
        match self {
            CapabilityType::DomRead => RiskLevel::Low,
            CapabilityType::NetworkFetch => RiskLevel::Medium,
            CapabilityType::StorageRead => RiskLevel::Medium,
            CapabilityType::StorageWrite => RiskLevel::Medium,
            CapabilityType::CookieRead => RiskLevel::Medium,
            CapabilityType::DomWrite => RiskLevel::High,
            CapabilityType::CookieWrite => RiskLevel::High,
            CapabilityType::JsExecute => RiskLevel::High,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrincipalKind {
    Extension,
    Agent,
    WebContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    pub id: Uuid,
    pub kind: PrincipalKind,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub token_id: Uuid,
    pub principal: Principal,
    pub capability: CapabilityType,
    pub origin_scope: String,
    pub url_allowlist: Vec<String>,
    pub rate_limit: Option<u32>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl CapabilityToken {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn matches_origin(&self, origin: &str) -> bool {
        self.origin_scope == "*" || self.origin_scope == origin
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DenialReason {
    PolicyRejected,
    TokenExpired,
    OriginMismatch,
    RateLimitExceeded,
    UnknownPrincipal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrokerDecision {
    Granted { token: CapabilityToken },
    Denied { reason: DenialReason },
}

#[derive(Debug, Default)]
pub struct CapabilityBroker {
    tokens: HashMap<Uuid, CapabilityToken>,
}

impl CapabilityBroker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mint_token(
        &mut self,
        principal: Principal,
        capability: CapabilityType,
        origin_scope: String,
        url_allowlist: Vec<String>,
        rate_limit: Option<u32>,
        duration_secs: i64,
    ) -> Uuid {
        let token_id = Uuid::new_v4();
        let issued_at = Utc::now();
        let expires_at = issued_at + Duration::seconds(duration_secs);

        let token = CapabilityToken {
            token_id,
            principal,
            capability,
            origin_scope,
            url_allowlist,
            rate_limit,
            issued_at,
            expires_at,
        };

        self.tokens.insert(token_id, token);
        token_id
    }

    pub fn check(&self, token_id: Uuid, url: &str) -> BrokerDecision {
        let token = match self.tokens.get(&token_id) {
            Some(t) => t,
            None => {
                return BrokerDecision::Denied {
                    reason: DenialReason::UnknownPrincipal,
                };
            }
        };

        if token.is_expired() {
            BrokerDecision::Denied {
                reason: DenialReason::TokenExpired,
            }
        } else if !url.starts_with(&token.origin_scope) && token.origin_scope != "*" {
            BrokerDecision::Denied {
                reason: DenialReason::OriginMismatch,
            }
        } else {
            BrokerDecision::Granted {
                token: token.clone(),
            }
        }
    }

    pub fn revoke(&mut self, token_id: Uuid) {
        self.tokens.remove(&token_id);
    }
}
