//! Import of Secutor key bundles (`.skb`).
//!
//! `.skb` is Secutor's portable container for moving certificate material
//! between machines. We read it and flatten it to a PEM blob (cert chain +
//! private key) that the rest of LCM already understands.
//!
//! Wire format (from Secutor's `keyBundle.ts`):
//! - magic `SECUTOR_KB` (10 bytes), variant byte (`P`=plain / `E`=encrypted),
//!   version byte (1).
//! - Plain: `header(12)` + body. Body = `u32 BE manifest_len` + manifest JSON +
//!   trailing payload.
//! - Encrypted: `prefix(16: magic+variant+ver+logN+r+p+reserved)` + `salt(16)` +
//!   `iv(12)` + ciphertext + `tag(16)`, where the key is
//!   `scrypt(password, salt, N=2^logN, r, p) -> 32 bytes` and the cipher is
//!   AES-256-GCM over the same body a plain bundle would carry.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use serde::Deserialize;

use crate::error::{Error, Result};

const MAGIC: &[u8] = b"SECUTOR_KB";
const VARIANT_PLAIN: u8 = 0x50;
const VARIANT_ENC: u8 = 0x45;
const VERSION: u8 = 1;
const HEADER_LEN: usize = 12;
const ENC_PREFIX_LEN: usize = 16;
const SALT_LEN: usize = 16;
const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;

#[derive(Deserialize)]
struct Manifest {
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    role: String,
    encoding: String,
    data: String,
}

/// True if `bytes` is a Secutor key bundle (magic check).
pub fn looks_like_skb(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER_LEN && &bytes[..MAGIC.len()] == MAGIC
}

/// Flatten a `.skb` bundle to a PEM blob (cert chain + private key). `password`
/// is required for encrypted bundles.
pub fn to_pem(bytes: &[u8], password: Option<&str>) -> Result<Vec<u8>> {
    if !looks_like_skb(bytes) {
        return Err(Error::CertParse("not a Secutor key bundle".into()));
    }
    if bytes[11] != VERSION {
        return Err(Error::CertParse(format!("unsupported .skb version {}", bytes[11])));
    }
    let body = match bytes[10] {
        VARIANT_PLAIN => bytes[HEADER_LEN..].to_vec(),
        VARIANT_ENC => decrypt_body(bytes, password.unwrap_or(""))?,
        v => return Err(Error::CertParse(format!("unknown .skb variant 0x{v:02x}"))),
    };
    let (manifest, payload) = decode_body(&body)?;
    items_to_pem(&manifest, &payload, password)
}

fn decrypt_body(bytes: &[u8], password: &str) -> Result<Vec<u8>> {
    if password.is_empty() {
        return Err(Error::CertParse("the .skb bundle is encrypted; a password is required".into()));
    }
    let log_n = bytes[12];
    let r = bytes[13] as u32;
    let p = bytes[14] as u32;
    let iv_start = ENC_PREFIX_LEN + SALT_LEN;
    let ct_start = iv_start + IV_LEN;
    if bytes.len() <= ct_start + TAG_LEN {
        return Err(Error::CertParse("encrypted .skb too short".into()));
    }
    let salt = &bytes[ENC_PREFIX_LEN..iv_start];
    let iv = &bytes[iv_start..ct_start];
    let ct_and_tag = &bytes[ct_start..]; // aes-gcm wants ciphertext||tag

    let params = scrypt::Params::new(log_n, r, p, 32)
        .map_err(|e| Error::CertParse(format!("bad scrypt params: {e}")))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(password.as_bytes(), salt, &params, &mut key)
        .map_err(|e| Error::CertParse(format!("scrypt failed: {e}")))?;

    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| Error::CertParse(format!("key init: {e}")))?;
    cipher
        .decrypt(Nonce::from_slice(iv), ct_and_tag)
        .map_err(|_| Error::CertParse("wrong password or corrupted .skb".into()))
}

fn decode_body(body: &[u8]) -> Result<(Manifest, Vec<u8>)> {
    if body.len() < 4 {
        return Err(Error::CertParse(".skb body too short".into()));
    }
    let len = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
    if len > body.len().saturating_sub(4) {
        return Err(Error::CertParse(".skb manifest length out of range".into()));
    }
    let manifest: Manifest = serde_json::from_slice(&body[4..4 + len])
        .map_err(|e| Error::CertParse(format!(".skb manifest: {e}")))?;
    Ok((manifest, body[4 + len..].to_vec()))
}

fn items_to_pem(manifest: &Manifest, payload: &[u8], password: Option<&str>) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let b64 = base64::engine::general_purpose::STANDARD;

    for item in &manifest.items {
        match (item.role.as_str(), item.encoding.as_str()) {
            // PEM text items (certs and keys): data is base64 of the PEM text.
            ("cert" | "parent" | "child" | "key", "pem") => {
                let pem = b64.decode(&item.data).map_err(|e| Error::CertParse(format!("base64: {e}")))?;
                out.extend_from_slice(&pem);
                if !pem.ends_with(b"\n") {
                    out.push(b'\n');
                }
            }
            // DER cert → wrap into PEM.
            ("cert" | "parent" | "child", "der") => {
                let der = b64.decode(&item.data).map_err(|e| Error::CertParse(format!("base64: {e}")))?;
                out.extend_from_slice(pem::encode(&pem::Pem::new("CERTIFICATE", der)).as_bytes());
            }
            // PKCS#12 (item data or trailing payload) → PEM via openssl.
            ("p12", _) => {
                let der = if item.data.is_empty() {
                    payload.to_vec()
                } else {
                    b64.decode(&item.data).map_err(|e| Error::CertParse(format!("base64: {e}")))?
                };
                out.extend_from_slice(&crate::pkcs12::to_pem(&der, password.unwrap_or(""))?);
            }
            // ssh-* and anything else: not relevant to cert integration.
            _ => {}
        }
    }

    // Fallback: a bundle whose material lives entirely in the trailing payload.
    if out.is_empty() && !payload.is_empty() {
        if let Ok(pem) = crate::pkcs12::to_pem(payload, password.unwrap_or("")) {
            out.extend_from_slice(&pem);
        }
    }

    if out.is_empty() {
        return Err(Error::NoCert);
    }
    Ok(out)
}
