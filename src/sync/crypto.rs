//! Zero-knowledge key hierarchy and per-record encryption.
//!
//! REQ-004 (key hierarchy) + REQ-005 (per-record blob encryption).
//!
//! A random 256-bit *master key* encrypts every record. The master key is
//! wrapped twice — once under a key derived from the user's passphrase, once
//! under a key derived from a one-time recovery code — and only those wrapped
//! copies (plus salts and KDF parameters) are ever stored on a server. The
//! server therefore holds no material that can decrypt vault content: losing
//! both the passphrase and the recovery code is unrecoverable by design.
//!
//! Each record is encrypted under a per-record key derived from the master key
//! and the record's stable ID via HKDF, so blobs can be added or replaced
//! individually without re-encrypting the whole vault, and a record's stable ID
//! is bound into the ciphertext as associated data (a blob cannot be silently
//! moved onto a different record). All encryption is AEAD (XChaCha20-Poly1305)
//! and fails closed on tamper.
//!
//! This is a self-contained, fully-tested building block. Its public API is not
//! yet called from the binary — it will be wired in by the sync transport and
//! enrollment flow (REQ-006/008) — so the module is allowed to carry currently
//! unused items rather than emit dead-code warnings for code that is exercised
//! by its test suite.
#![allow(dead_code)]

use anyhow::{Context, Result, anyhow, bail};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use data_encoding::BASE32_NOPAD;
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const XNONCE_LEN: usize = 24;
/// 160 bits of recovery-code entropy, comfortably above the 128-bit floor.
const RECOVERY_ENTROPY_BYTES: usize = 20;

/// Associated data binding wrapped-key blobs to their purpose.
const WRAP_AAD: &[u8] = b"starlee-keywrap-v1";
/// HKDF `info` prefix for per-record key derivation.
const RECORD_INFO_PREFIX: &str = "starlee-record-v1:";
/// Hash prefix for opaque object keys.
const OBJECT_PREFIX: &str = "starlee-object-v1:";

/// A 256-bit master key. Zeroized on drop so it does not linger in memory.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; KEY_LEN]);

impl MasterKey {
    fn random() -> Result<Self> {
        let mut bytes = [0u8; KEY_LEN];
        getrandom::fill(&mut bytes).context("generate master key")?;
        Ok(Self(bytes))
    }

    /// Reconstruct a master key from raw bytes (e.g. cached on a device).
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw key bytes, for persisting on-device in OS-protected storage.
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

/// Argon2id cost parameters. Stored alongside the wrapped vault so unwrap
/// reproduces the derivation, and so the cost can be raised over time.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub mem_kib: u32,
    /// Number of iterations (time cost).
    pub iters: u32,
    /// Degree of parallelism.
    pub lanes: u32,
}

impl KdfParams {
    /// Production parameters: 64 MiB memory, 3 iterations (meets the PRD floor
    /// of ≥64 MiB / ≥3 iterations and ~≥250ms on reference hardware).
    pub fn production() -> Self {
        Self {
            mem_kib: 65_536,
            iters: 3,
            lanes: 1,
        }
    }

    /// Cheap parameters for tests only — never use to protect real data.
    #[cfg(test)]
    fn fast() -> Self {
        Self {
            mem_kib: 1_024,
            iters: 1,
            lanes: 1,
        }
    }
}

/// Server-stored key material. Every field is opaque: without the user's
/// passphrase or recovery code, none of it yields the master key (REQ-004).
#[derive(Clone, Serialize, Deserialize)]
pub struct WrappedVault {
    pub kdf: KdfParams,
    #[serde(with = "serde_bytes_array")]
    pub passphrase_salt: [u8; SALT_LEN],
    #[serde(with = "serde_bytes_array")]
    pub recovery_salt: [u8; SALT_LEN],
    /// Master key wrapped under the passphrase-derived KEK (`nonce || ct`).
    pub passphrase_wrapped: Vec<u8>,
    /// Master key wrapped under the recovery-code-derived KEK (`nonce || ct`).
    pub recovery_wrapped: Vec<u8>,
}

/// The product of first-device enrollment.
pub struct Enrollment {
    /// Opaque material to store server-side.
    pub wrapped: WrappedVault,
    /// The recovery code, shown to the user exactly once. Not recoverable from
    /// the server.
    pub recovery_code: String,
    /// The master key, kept on this device.
    pub master: MasterKey,
}

/// Enroll a new vault: generate a master key and recovery code, and wrap the
/// master key under both the passphrase and the recovery code (REQ-004).
pub fn enroll(passphrase: &str, kdf: KdfParams) -> Result<Enrollment> {
    if passphrase.is_empty() {
        bail!("passphrase cannot be empty");
    }
    let master = MasterKey::random()?;
    let recovery_code = generate_recovery_code()?;

    let mut passphrase_salt = [0u8; SALT_LEN];
    getrandom::fill(&mut passphrase_salt).context("generate passphrase salt")?;
    let mut recovery_salt = [0u8; SALT_LEN];
    getrandom::fill(&mut recovery_salt).context("generate recovery salt")?;

    let passphrase_wrapped = wrap_master(passphrase.as_bytes(), &passphrase_salt, kdf, &master)?;
    let recovery_wrapped = wrap_master(
        normalize_recovery(&recovery_code).as_bytes(),
        &recovery_salt,
        kdf,
        &master,
    )?;

    Ok(Enrollment {
        wrapped: WrappedVault {
            kdf,
            passphrase_salt,
            recovery_salt,
            passphrase_wrapped,
            recovery_wrapped,
        },
        recovery_code,
        master,
    })
}

/// Recover the master key from the passphrase. Returns a decryption error
/// (distinct from any transport error) if the passphrase is wrong.
pub fn unwrap_with_passphrase(vault: &WrappedVault, passphrase: &str) -> Result<MasterKey> {
    let mut kek = derive_kek(passphrase.as_bytes(), &vault.passphrase_salt, vault.kdf)?;
    let result = open_master(&kek, &vault.passphrase_wrapped);
    kek.zeroize();
    result.context("incorrect passphrase")
}

/// Recover the master key from the recovery code (case- and hyphen-insensitive).
pub fn unwrap_with_recovery(vault: &WrappedVault, recovery_code: &str) -> Result<MasterKey> {
    let mut kek = derive_kek(
        normalize_recovery(recovery_code).as_bytes(),
        &vault.recovery_salt,
        vault.kdf,
    )?;
    let result = open_master(&kek, &vault.recovery_wrapped);
    kek.zeroize();
    result.context("incorrect recovery code")
}

/// Re-wrap the master key under a new passphrase without re-encrypting any
/// record blobs (REQ-004). The recovery code and KDF parameters are preserved,
/// so the recovery path keeps working.
pub fn change_passphrase(vault: &WrappedVault, old: &str, new: &str) -> Result<WrappedVault> {
    if new.is_empty() {
        bail!("new passphrase cannot be empty");
    }
    let master = unwrap_with_passphrase(vault, old)?;
    let mut passphrase_salt = [0u8; SALT_LEN];
    getrandom::fill(&mut passphrase_salt).context("generate passphrase salt")?;
    let passphrase_wrapped = wrap_master(new.as_bytes(), &passphrase_salt, vault.kdf, &master)?;
    Ok(WrappedVault {
        kdf: vault.kdf,
        passphrase_salt,
        recovery_salt: vault.recovery_salt,
        passphrase_wrapped,
        recovery_wrapped: vault.recovery_wrapped.clone(),
    })
}

/// Encrypt one record's bytes into a standalone blob (`nonce || ciphertext`).
/// The record's stable ID is mixed into the per-record key and bound as AAD, so
/// a blob cannot be decrypted with the wrong key or relabeled onto another
/// record (REQ-005).
pub fn encrypt_record(master: &MasterKey, record_id: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut key = record_key(master, record_id)?;
    let result = aead_seal(&key, plaintext, record_id.as_bytes());
    key.zeroize();
    result
}

/// Decrypt a record blob. Fails closed (no partial plaintext) on a wrong key or
/// any tampering (REQ-005).
pub fn decrypt_record(master: &MasterKey, record_id: &str, blob: &[u8]) -> Result<Vec<u8>> {
    let mut key = record_key(master, record_id)?;
    let result = aead_open(&key, blob, record_id.as_bytes());
    key.zeroize();
    result.context("record blob failed to decrypt (wrong key or tampered)")
}

/// Derive the opaque storage key for a record. Contains no plaintext: the
/// title, URL, and path never appear in a stored object's name (REQ-005).
pub fn object_key(record_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(OBJECT_PREFIX.as_bytes());
    hasher.update(record_id.as_bytes());
    BASE32_NOPAD.encode(&hasher.finalize()).to_lowercase()
}

// ---- internals ---------------------------------------------------------------

fn wrap_master(
    secret: &[u8],
    salt: &[u8; SALT_LEN],
    kdf: KdfParams,
    master: &MasterKey,
) -> Result<Vec<u8>> {
    let mut kek = derive_kek(secret, salt, kdf)?;
    let result = aead_seal(&kek, master.as_bytes(), WRAP_AAD);
    kek.zeroize();
    result
}

fn open_master(kek: &[u8; KEY_LEN], wrapped: &[u8]) -> Result<MasterKey> {
    let mut bytes = aead_open(kek, wrapped, WRAP_AAD)?;
    if bytes.len() != KEY_LEN {
        bytes.zeroize();
        bail!("unwrapped key has the wrong length");
    }
    let mut arr = [0u8; KEY_LEN];
    arr.copy_from_slice(&bytes);
    bytes.zeroize();
    Ok(MasterKey(arr))
}

fn derive_kek(secret: &[u8], salt: &[u8], kdf: KdfParams) -> Result<[u8; KEY_LEN]> {
    let params = Params::new(kdf.mem_kib, kdf.iters, kdf.lanes, Some(KEY_LEN))
        .map_err(|error| anyhow!("invalid argon2 parameters: {error}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; KEY_LEN];
    argon2
        .hash_password_into(secret, salt, &mut out)
        .map_err(|error| anyhow!("argon2 derivation failed: {error}"))?;
    Ok(out)
}

fn record_key(master: &MasterKey, record_id: &str) -> Result<[u8; KEY_LEN]> {
    let hk = Hkdf::<Sha256>::new(None, master.as_bytes());
    let mut okm = [0u8; KEY_LEN];
    hk.expand(
        format!("{RECORD_INFO_PREFIX}{record_id}").as_bytes(),
        &mut okm,
    )
    .map_err(|_| anyhow!("hkdf expansion failed"))?;
    Ok(okm)
}

fn aead_seal(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce = [0u8; XNONCE_LEN];
    getrandom::fill(&mut nonce).context("generate nonce")?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| anyhow!("AEAD encryption failed"))?;
    let mut out = Vec::with_capacity(XNONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn aead_open(key: &[u8; KEY_LEN], blob: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < XNONCE_LEN {
        bail!("ciphertext is too short to contain a nonce");
    }
    let (nonce, ciphertext) = blob.split_at(XNONCE_LEN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| anyhow!("AEAD decryption failed"))
}

fn generate_recovery_code() -> Result<String> {
    let mut bytes = [0u8; RECOVERY_ENTROPY_BYTES];
    getrandom::fill(&mut bytes).context("generate recovery code")?;
    let raw = BASE32_NOPAD.encode(&bytes);
    // Group into 4-character runs for legibility: XXXX-XXXX-...
    let grouped = raw
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("-");
    Ok(grouped)
}

/// Normalize a user-entered recovery code: strip hyphens/whitespace and
/// uppercase, so formatting differences do not change the derived key.
fn normalize_recovery(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect()
}

/// Serde helper for fixed-size byte arrays (serialized as a byte sequence).
mod serde_bytes_array {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[u8; N], D::Error> {
        let vec = Vec::<u8>::deserialize(deserializer)?;
        vec.as_slice()
            .try_into()
            .map_err(|_| D::Error::custom(format!("expected {N} bytes, got {}", vec.len())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASS: &str = "correct horse battery staple";

    #[test]
    fn production_params_meet_the_security_floor() {
        let p = KdfParams::production();
        assert!(p.mem_kib >= 65_536, "memory cost must be ≥64 MiB");
        assert!(p.iters >= 3, "iteration count must be ≥3");
    }

    #[test]
    fn enroll_then_unwrap_with_passphrase_recovers_master() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let recovered = unwrap_with_passphrase(&e.wrapped, PASS)?;
        assert_eq!(recovered.as_bytes(), e.master.as_bytes());
        Ok(())
    }

    #[test]
    fn unwrap_with_wrong_passphrase_fails() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        assert!(unwrap_with_passphrase(&e.wrapped, "wrong passphrase").is_err());
        Ok(())
    }

    #[test]
    fn recovery_code_recovers_master_and_is_format_insensitive() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        // Exact code works.
        assert_eq!(
            unwrap_with_recovery(&e.wrapped, &e.recovery_code)?.as_bytes(),
            e.master.as_bytes()
        );
        // Lowercased and hyphen-stripped variants work too.
        let messy = e.recovery_code.to_lowercase().replace('-', " ");
        assert_eq!(
            unwrap_with_recovery(&e.wrapped, &messy)?.as_bytes(),
            e.master.as_bytes()
        );
        Ok(())
    }

    #[test]
    fn wrong_recovery_code_fails() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        assert!(unwrap_with_recovery(&e.wrapped, "AAAA-BBBB-CCCC-DDDD").is_err());
        Ok(())
    }

    #[test]
    fn change_passphrase_rewraps_without_changing_master_or_recovery() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let rewrapped = change_passphrase(&e.wrapped, PASS, "a brand new passphrase")?;

        // New passphrase unwraps to the same master.
        assert_eq!(
            unwrap_with_passphrase(&rewrapped, "a brand new passphrase")?.as_bytes(),
            e.master.as_bytes()
        );
        // Old passphrase no longer works.
        assert!(unwrap_with_passphrase(&rewrapped, PASS).is_err());
        // Recovery code still works (unchanged) — no blob re-encryption needed.
        assert_eq!(
            unwrap_with_recovery(&rewrapped, &e.recovery_code)?.as_bytes(),
            e.master.as_bytes()
        );
        Ok(())
    }

    #[test]
    fn change_passphrase_requires_correct_old_passphrase() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        assert!(change_passphrase(&e.wrapped, "not the passphrase", "new").is_err());
        Ok(())
    }

    #[test]
    fn record_round_trips() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let plaintext = b"# A paywalled article body";
        let blob = encrypt_record(&e.master, "2026-ab12cd-ef34gh", plaintext)?;
        let recovered = decrypt_record(&e.master, "2026-ab12cd-ef34gh", &blob)?;
        assert_eq!(recovered, plaintext);
        Ok(())
    }

    #[test]
    fn decrypting_with_a_different_record_id_fails() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let blob = encrypt_record(&e.master, "record-a", b"secret")?;
        // Both the per-record key and the AAD differ, so this must fail closed.
        assert!(decrypt_record(&e.master, "record-b", &blob).is_err());
        Ok(())
    }

    #[test]
    fn tampering_with_a_blob_fails_closed() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let mut blob = encrypt_record(&e.master, "rec", b"secret body")?;
        // Flip a byte in the ciphertext region.
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(decrypt_record(&e.master, "rec", &blob).is_err());

        // Flip a byte in the nonce region.
        let mut blob2 = encrypt_record(&e.master, "rec", b"secret body")?;
        blob2[0] ^= 0x01;
        assert!(decrypt_record(&e.master, "rec", &blob2).is_err());
        Ok(())
    }

    #[test]
    fn short_blob_is_rejected() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        assert!(decrypt_record(&e.master, "rec", b"too short").is_err());
        Ok(())
    }

    #[test]
    fn nonce_is_randomized_per_encryption() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let a = encrypt_record(&e.master, "rec", b"same plaintext")?;
        let b = encrypt_record(&e.master, "rec", b"same plaintext")?;
        // Identical plaintext must not produce identical ciphertext...
        assert_ne!(a, b);
        // ...yet both decrypt correctly.
        assert_eq!(decrypt_record(&e.master, "rec", &a)?, b"same plaintext");
        assert_eq!(decrypt_record(&e.master, "rec", &b)?, b"same plaintext");
        Ok(())
    }

    #[test]
    fn ciphertext_leaks_no_plaintext_or_id() -> Result<()> {
        // Goal 2 mini-audit: a known plaintext marker must not appear in the blob,
        // and the opaque object key must not embed the record id.
        let e = enroll(PASS, KdfParams::fast())?;
        let marker = b"PAYWALLED-SECRET-MARKER";
        let mut body = Vec::new();
        body.extend_from_slice(b"intro ");
        body.extend_from_slice(marker);
        body.extend_from_slice(b" outro");
        let blob = encrypt_record(&e.master, "the-record-id", &body)?;
        assert!(
            !contains_subslice(&blob, marker),
            "plaintext marker leaked into ciphertext"
        );
        let key = object_key("the-record-id");
        assert!(
            !key.contains("the-record-id"),
            "object key leaked record id"
        );
        Ok(())
    }

    #[test]
    fn object_key_is_deterministic_and_distinct() {
        assert_eq!(object_key("rec-1"), object_key("rec-1"));
        assert_ne!(object_key("rec-1"), object_key("rec-2"));
        assert!(!object_key("rec-1").is_empty());
    }

    #[test]
    fn recovery_code_has_expected_entropy_shape() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let compact = normalize_recovery(&e.recovery_code);
        // 20 bytes of entropy → 32 base32 chars (≥128 bits).
        assert_eq!(compact.len(), 32);
        assert!(
            compact
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
            "recovery code must be base32"
        );
        assert!(
            e.recovery_code.contains('-'),
            "recovery code should be grouped"
        );
        Ok(())
    }

    #[test]
    fn empty_passphrase_is_rejected() {
        assert!(enroll("", KdfParams::fast()).is_err());
    }

    #[test]
    fn wrapped_vault_serializes_for_server_storage() -> Result<()> {
        let e = enroll(PASS, KdfParams::fast())?;
        let json = serde_json::to_vec(&e.wrapped)?;
        let restored: WrappedVault = serde_json::from_slice(&json)?;
        // After a round-trip through the server's storage format, the passphrase
        // still unwraps the master key.
        assert_eq!(
            unwrap_with_passphrase(&restored, PASS)?.as_bytes(),
            e.master.as_bytes()
        );
        Ok(())
    }

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }
}
