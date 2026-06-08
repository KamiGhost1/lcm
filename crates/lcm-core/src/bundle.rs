//! Parsing of a "certificate material" bundle: a leaf certificate, optional
//! chain, and optional private key — used by client identities and server
//! certificates.
//!
//! v1 accepts PEM input (one file holding any of: leaf cert, intermediate
//! certs, a private key). PKCS#12 (`.p12`/`.pfx`) is a planned follow-up.

use serde::Serialize;

use crate::cert::{self, CertInfo};
use crate::error::{Error, Result};

/// A parsed bundle of certificate material.
#[derive(Debug, Clone, Serialize)]
pub struct Material {
    /// The end-entity certificate (first non-CA cert, else the first cert).
    pub leaf: CertInfo,
    /// Any additional certificates (intermediates / root), in input order.
    pub chain: Vec<CertInfo>,
    /// Whether a private key was present alongside the certificate(s).
    pub has_key: bool,
    /// The leaf certificate as PEM (not serialized to the frontend).
    #[serde(skip)]
    pub leaf_pem: String,
    /// The chain as concatenated PEM, if any (not serialized).
    #[serde(skip)]
    pub chain_pem: String,
    /// The private key PEM block, if present (not serialized).
    #[serde(skip)]
    pub key_pem: Option<String>,
}

/// True if a PEM tag denotes a private key (any of the common variants).
fn is_private_key_tag(tag: &str) -> bool {
    tag.ends_with("PRIVATE KEY")
}

/// Parse certificate material into a [`Material`].
///
/// Accepts a PEM bundle, or a PKCS#12 (`.p12`/`.pfx`) blob when `password` is
/// supplied (use `Some("")` for password-less PKCS#12). Requires at least one
/// certificate.
pub fn parse_material(input: &[u8], password: Option<&str>) -> Result<Material> {
    if crate::pkcs12::looks_like_pem(input) {
        parse_pem(input)
    } else if crate::skb::looks_like_skb(input) {
        // Secutor key bundle → flatten to PEM (decrypting if needed).
        let pem = crate::skb::to_pem(input, password)?;
        parse_pem(&pem)
    } else {
        // Otherwise treat binary input as PKCS#12 and convert to PEM via openssl.
        let pem = crate::pkcs12::to_pem(input, password.unwrap_or(""))?;
        parse_pem(&pem)
    }
}

/// Parse a PEM bundle (cert chain + optional private key) into a [`Material`].
fn parse_pem(input: &[u8]) -> Result<Material> {
    let blocks = pem::parse_many(input).map_err(|e| Error::CertParse(e.to_string()))?;

    let mut certs: Vec<CertInfo> = Vec::new();
    let mut key_pem: Option<String> = None;

    for b in &blocks {
        if b.tag() == "CERTIFICATE" {
            certs.push(cert::parse_one(&pem::encode(b).into_bytes())?);
        } else if is_private_key_tag(b.tag()) && key_pem.is_none() {
            key_pem = Some(pem::encode(b));
        }
    }

    if certs.is_empty() {
        return Err(Error::NoCert);
    }

    // Leaf = first non-CA cert if present, otherwise the first certificate.
    let leaf_idx = certs.iter().position(|c| !c.is_ca).unwrap_or(0);
    let leaf = certs.remove(leaf_idx);
    let chain = certs;

    let leaf_pem = cert::to_pem(&leaf);
    let chain_pem = chain.iter().map(cert::to_pem).collect::<String>();

    Ok(Material {
        has_key: key_pem.is_some(),
        leaf,
        chain,
        leaf_pem,
        chain_pem,
        key_pem,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_key_tags() {
        assert!(is_private_key_tag("PRIVATE KEY"));
        assert!(is_private_key_tag("RSA PRIVATE KEY"));
        assert!(is_private_key_tag("EC PRIVATE KEY"));
        assert!(is_private_key_tag("ENCRYPTED PRIVATE KEY"));
        assert!(!is_private_key_tag("CERTIFICATE"));
        assert!(!is_private_key_tag("PUBLIC KEY"));
    }

    #[test]
    fn pem_without_cert_errors() {
        // A PEM blob carrying only a key (no CERTIFICATE) must be rejected.
        let key_only = "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n";
        assert!(parse_material(key_only.as_bytes(), None).is_err());
    }
}
