use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use rusqlite::{params, Connection};
use std::str::FromStr;#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditEventKind {
    CapabilityGranted,
    CapabilityDenied,
    CapabilityExercised,
    ContentBlocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub entry_id: Uuid,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub kind: AuditEventKind,
    pub principal_id: Uuid,
    pub capability: Option<String>,
    pub url: Option<String>,
    pub prev_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone)]
pub struct AuditLog {
    pub entries: Vec<AuditEntry>,
    pub sequence_counter: u64,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Hash mismatch: entry {sequence} hash {expected} does not match computed {actual}")]
    HashMismatch {
        sequence: u64,
        expected: String,
        actual: String,
    },
    #[error("Chain broken at entry {sequence}: prev_hash {actual_prev} != previous entry_hash {expected_prev}")]
    ChainBroken {
        sequence: u64,
        expected_prev: String,
        actual_prev: String,
    },
    #[error("Database error: {0}")]
    Sql(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Chain broken upon load")]
    ChainBrokenLoad,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            sequence_counter: 0,
        }
    }

    pub fn append(
        &mut self,
        kind: AuditEventKind,
        principal_id: Uuid,
        capability: Option<String>,
        url: Option<String>,
    ) {
        let sequence = self.sequence_counter;
        let timestamp = Utc::now();
        let prev_hash = self
            .entries
            .last()
            .map(|e| e.entry_hash.clone())
            .unwrap_or_default();

        // Computes SHA-256 over format!("{}{}{:?}{}{}", sequence, timestamp.to_rfc3339(), kind, principal_id, prev_hash)
        let hash_input = format!(
            "{}{}{:?}{}{}",
            sequence,
            timestamp.to_rfc3339(),
            kind,
            principal_id,
            prev_hash
        );

        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let digest = hasher.finalize();
        let entry_hash = hex::encode(digest);

        let entry = AuditEntry {
            entry_id: Uuid::new_v4(),
            sequence,
            timestamp,
            kind,
            principal_id,
            capability,
            url,
            prev_hash,
            entry_hash,
        };

        self.entries.push(entry);
        self.sequence_counter += 1;
    }

    pub fn verify_chain(&self) -> bool {
        let mut expected_prev = String::new();

        for entry in &self.entries {
            if entry.prev_hash != expected_prev {
                return false;
            }

            let hash_input = format!(
                "{}{}{:?}{}{}",
                entry.sequence,
                entry.timestamp.to_rfc3339(),
                entry.kind,
                entry.principal_id,
                entry.prev_hash
            );
            let mut hasher = Sha256::new();
            hasher.update(hash_input.as_bytes());
            let computed_hash = hex::encode(hasher.finalize());

            if computed_hash != entry.entry_hash {
                return false;
            }
            expected_prev = entry.entry_hash.clone();
        }

        true
    }
}

pub struct PersistentAuditLog {
    pub log: AuditLog,
    pub conn: Connection,
}

impl PersistentAuditLog {
    pub fn new(db_path: &str) -> Result<Self, AuditError> {
        let conn = Connection::open(db_path).map_err(|e| AuditError::Sql(e.to_string()))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_entries (
                entry_id TEXT PRIMARY KEY,
                sequence INTEGER,
                timestamp TEXT,
                kind TEXT,
                principal_id TEXT,
                capability TEXT,
                url TEXT,
                prev_hash TEXT,
                entry_hash TEXT
            )",
            [],
        ).map_err(|e| AuditError::Sql(e.to_string()))?;

        Ok(Self {
            log: AuditLog::new(),
            conn,
        })
    }

    pub fn append(
        &mut self,
        kind: AuditEventKind,
        principal_id: Uuid,
        capability: Option<String>,
        url: Option<String>,
    ) -> Result<(), AuditError> {
        self.log.append(kind.clone(), principal_id, capability.clone(), url.clone());
        let entry = self.log.entries.last().unwrap();
        
        let kind_str = serde_json::to_string(&entry.kind).map_err(|e| AuditError::Serialization(e.to_string()))?;

        self.conn.execute(
            "INSERT INTO audit_entries (entry_id, sequence, timestamp, kind, principal_id, capability, url, prev_hash, entry_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.entry_id.to_string(),
                entry.sequence,
                entry.timestamp.to_rfc3339(),
                kind_str,
                entry.principal_id.to_string(),
                entry.capability,
                entry.url,
                entry.prev_hash,
                entry.entry_hash,
            ],
        ).map_err(|e| AuditError::Sql(e.to_string()))?;

        Ok(())
    }

    pub fn load(db_path: &str) -> Result<Self, AuditError> {
        let conn = Connection::open(db_path).map_err(|e| AuditError::Sql(e.to_string()))?;
        
        let entries = {
            let mut stmt = conn.prepare("SELECT entry_id, sequence, timestamp, kind, principal_id, capability, url, prev_hash, entry_hash FROM audit_entries ORDER BY sequence ASC").map_err(|e| AuditError::Sql(e.to_string()))?;
            
            let entry_iter = stmt.query_map([], |row| {
                let entry_id_str: String = row.get(0)?;
                let sequence: u64 = row.get(1)?;
                let timestamp_str: String = row.get(2)?;
                let kind_str: String = row.get(3)?;
                let principal_id_str: String = row.get(4)?;
                let capability: Option<String> = row.get(5)?;
                let url: Option<String> = row.get(6)?;
                let prev_hash: String = row.get(7)?;
                let entry_hash: String = row.get(8)?;
                
                Ok((entry_id_str, sequence, timestamp_str, kind_str, principal_id_str, capability, url, prev_hash, entry_hash))
            }).map_err(|e| AuditError::Sql(e.to_string()))?;

            let mut entries = Vec::new();
            for row_result in entry_iter {
                let (entry_id_str, sequence, timestamp_str, kind_str, principal_id_str, capability, url, prev_hash, entry_hash) = row_result.map_err(|e| AuditError::Sql(e.to_string()))?;
                
                let entry_id = Uuid::from_str(&entry_id_str).map_err(|e| AuditError::Parse(e.to_string()))?;
                let timestamp = DateTime::parse_from_rfc3339(&timestamp_str).map_err(|e| AuditError::Parse(e.to_string()))?.with_timezone(&Utc);
                let kind: AuditEventKind = serde_json::from_str(&kind_str).map_err(|e| AuditError::Serialization(e.to_string()))?;
                let principal_id = Uuid::from_str(&principal_id_str).map_err(|e| AuditError::Parse(e.to_string()))?;

                entries.push(AuditEntry {
                    entry_id,
                    sequence,
                    timestamp,
                    kind,
                    principal_id,
                    capability,
                    url,
                    prev_hash,
                    entry_hash,
                });
            }
            entries
        };

        let sequence_counter = entries.last().map(|e| e.sequence + 1).unwrap_or(0);
        
        let log = AuditLog {
            entries,
            sequence_counter,
        };

        if !log.verify_chain() {
            return Err(AuditError::ChainBrokenLoad);
        }

        Ok(Self { log, conn })
    }
}
