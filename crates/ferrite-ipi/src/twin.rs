use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};
use rand::Rng;

/// A fake but realistic set of user credentials used during dry runs.
/// All values are plausible but entirely fictitious.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyntheticTwin {
    pub name: String,
    pub email: String,
    pub password: String,
    pub phone: String,
    pub credit_card: String,
    pub ssn: String,
    pub address: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SyntheticTwin {
    /// Generates a new random synthetic twin.
    /// All values are plausible but fictitious.
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let id: u32 = rng.gen_range(1000..9999);
        Self {
            name: format!("Alex Ferrite-{}", id),
            email: format!("user{}@ferrite-test.invalid", id),
            password: format!("Synth!Pass{}#", id),
            phone: format!("+1-555-{:04}-{:04}", id, rng.gen_range(1000u32..9999u32)),
            credit_card: format!("4000-0000-0000-{:04}", id),
            ssn: format!("000-00-{:04}", id),
            address: format!("{} Synthetic Ave, Testville, CA 00000", id),
            created_at: chrono::Utc::now(),
        }
    }

    /// Returns true if the twin is older than `ttl_hours`.
    pub fn is_expired(&self, ttl_hours: i64) -> bool {
        let age = chrono::Utc::now() - self.created_at;
        age.num_hours() >= ttl_hours
    }
}

// 256-bit key derived from a fixed dev secret. Production would use keyring.
const DEV_KEY: &[u8; 32] = b"ferrite-ipi-twin-dev-key-32byte!";

/// Encrypts a `SyntheticTwin` with AES-256-GCM.
/// Returns nonce (12 bytes) prepended to ciphertext.
pub fn encrypt_twin(twin: &SyntheticTwin) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(twin).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new(GenericArray::from_slice(DEV_KEY));
    let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
    let nonce = GenericArray::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, json.as_ref())
        .map_err(|e| format!("encrypt: {}", e))?;
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypts bytes produced by `encrypt_twin` back into a `SyntheticTwin`.
pub fn decrypt_twin(data: &[u8]) -> Result<SyntheticTwin, String> {
    if data.len() < 12 {
        return Err("data too short".to_string());
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(DEV_KEY));
    let nonce = GenericArray::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("decrypt: {}", e))?;
    serde_json::from_slice(&plaintext).map_err(|e| e.to_string())
}

/// Manages the stored synthetic twin, including TTL rotation.
pub struct TwinManager {
    storage_path: std::path::PathBuf,
    ttl_hours: i64,
}

impl TwinManager {
    pub fn new(storage_path: std::path::PathBuf) -> Self {
        Self {
            storage_path,
            ttl_hours: 24,
        }
    }

    /// Loads the stored twin if present and unexpired.
    /// Generates and stores a fresh twin otherwise.
    pub fn load_or_generate(&self) -> SyntheticTwin {
        if let Ok(data) = std::fs::read(&self.storage_path) {
            if let Ok(twin) = decrypt_twin(&data) {
                if !twin.is_expired(self.ttl_hours) {
                    return twin;
                }
            }
        }
        let twin = SyntheticTwin::generate();
        if let Ok(enc) = encrypt_twin(&twin) {
            let _ = std::fs::write(&self.storage_path, enc);
        }
        twin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twin_generate_produces_unique_values() {
        let t1 = SyntheticTwin::generate();
        let t2 = SyntheticTwin::generate();
        // Email addresses should differ (different random IDs) — extremely likely
        assert_ne!(t1.email, t2.email);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let twin = SyntheticTwin::generate();
        let enc = encrypt_twin(&twin).expect("encrypt failed");
        let dec = decrypt_twin(&enc).expect("decrypt failed");
        assert_eq!(dec.email, twin.email);
        assert_eq!(dec.name, twin.name);
    }

    #[test]
    fn twin_not_expired_immediately() {
        let twin = SyntheticTwin::generate();
        assert!(!twin.is_expired(24));
    }

    #[test]
    fn twin_manager_load_or_generate_returns_valid_twin() {
        let path =
            std::env::temp_dir().join(format!("ferrite-twin-test-{}.enc", uuid::Uuid::new_v4()));
        let mgr = TwinManager::new(path.clone());
        let twin = mgr.load_or_generate();
        assert!(twin.email.contains("ferrite-test.invalid"));
        // Second call must load the same twin from disk
        let twin2 = mgr.load_or_generate();
        assert_eq!(twin.email, twin2.email);
        let _ = std::fs::remove_file(path);
    }
}
