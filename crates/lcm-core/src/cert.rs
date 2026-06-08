//! X.509 certificate parsing and inspection.
//!
//! Accepts either PEM (one cert or a chain) or a single DER blob, and surfaces
//! the handful of fields LCM displays and validates against.

use serde::Serialize;
use sha2::{Digest, Sha256};
// Import specific items rather than glob-importing the prelude: the prelude
// re-exports x509_parser's own `pem` module, which would shadow the `pem` crate
// we use for PEM encoding below.
use x509_parser::certificate::X509Certificate;
use x509_parser::prelude::FromDer;

use crate::error::{Error, Result};

/// Human-facing metadata for a single certificate.
#[derive(Debug, Clone, Serialize)]
pub struct CertInfo {
    pub subject: String,
    pub issuer: String,
    pub serial: String,
    pub not_before: String,
    pub not_after: String,
    /// `not_before` / `not_after` as Unix timestamps (seconds), for programmatic
    /// expiry math (e.g. "expires in N days") without parsing the display strings.
    pub not_before_ts: i64,
    pub not_after_ts: i64,
    /// Uppercase hex SHA-256 of the DER, colon-separated.
    pub fingerprint_sha256: String,
    /// Whether BasicConstraints marks this as a CA.
    pub is_ca: bool,
    /// Raw DER bytes (not serialized).
    #[serde(skip)]
    pub der: Vec<u8>,
}

/// Parse certificates from `input`, which may be PEM (possibly a chain) or a
/// single DER certificate.
pub fn parse(input: &[u8]) -> Result<Vec<CertInfo>> {
    let ders = if looks_like_pem(input) {
        pem::parse_many(input)
            .map_err(|e| Error::CertParse(e.to_string()))?
            .into_iter()
            .filter(|p| p.tag() == "CERTIFICATE")
            .map(|p| p.contents().to_vec())
            .collect::<Vec<_>>()
    } else {
        vec![input.to_vec()]
    };

    if ders.is_empty() {
        return Err(Error::NoCert);
    }

    ders.into_iter().map(parse_der).collect()
}

/// Parse exactly one certificate, erroring if the input holds none.
pub fn parse_one(input: &[u8]) -> Result<CertInfo> {
    parse(input)?.into_iter().next().ok_or(Error::NoCert)
}

fn looks_like_pem(input: &[u8]) -> bool {
    input
        .windows(b"-----BEGIN".len())
        .any(|w| w == b"-----BEGIN")
}

fn parse_der(der: Vec<u8>) -> Result<CertInfo> {
    let (_, cert) = X509Certificate::from_der(&der).map_err(|e| Error::CertParse(e.to_string()))?;

    let fingerprint_sha256 = {
        let digest = Sha256::digest(&der);
        digest
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    };

    let is_ca = cert
        .basic_constraints()
        .ok()
        .flatten()
        .map(|bc| bc.value.ca)
        .unwrap_or(false);

    Ok(CertInfo {
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        serial: cert.raw_serial_as_string(),
        not_before: cert.validity().not_before.to_string(),
        not_after: cert.validity().not_after.to_string(),
        not_before_ts: cert.validity().not_before.timestamp(),
        not_after_ts: cert.validity().not_after.timestamp(),
        fingerprint_sha256,
        is_ca,
        der,
    })
}

/// Re-encode a certificate's DER as a single PEM block (what the Debian trust
/// store expects on disk).
pub fn to_pem(info: &CertInfo) -> String {
    pem::encode(&pem::Pem::new("CERTIFICATE", info.der.clone()))
}
