//! Read-only audit of the system trust store: every CA the machine trusts,
//! not just the ones LCM installed.
//!
//! Rather than reverse-engineer each distro's store, we parse the consolidated
//! PEM bundle that the trust tooling already generates. The first existing
//! candidate wins.

use std::path::{Path, PathBuf};

use crate::cert::{self, CertInfo};
use crate::error::Result;

/// Consolidated CA bundles, in priority order across distro families.
const BUNDLES: &[&str] = &[
    "/etc/ssl/certs/ca-certificates.crt",                 // Debian/Ubuntu/Alpine/openSUSE/Arch
    "/etc/pki/tls/certs/ca-bundle.crt",                   // Fedora/RHEL
    "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",  // Fedora (p11-kit extract)
    "/etc/ssl/cert.pem",                                  // misc / fallback
];

/// Path of the consolidated trust bundle on this system, if found.
pub fn bundle_path() -> Option<PathBuf> {
    BUNDLES.iter().map(Path::new).find(|p| p.exists()).map(Path::to_path_buf)
}

/// List every CA in the system trust bundle (deduplicated by fingerprint).
pub fn list_system_trust() -> Result<Vec<CertInfo>> {
    let Some(path) = bundle_path() else {
        return Ok(Vec::new());
    };
    let bytes = std::fs::read(&path)?;
    // A malformed cert in a 150-entry bundle shouldn't sink the whole audit, so
    // parse leniently: keep what parses.
    let mut certs = cert::parse(&bytes).unwrap_or_default();

    // Dedup by fingerprint (needs them adjacent), then sort by subject for display.
    certs.sort_by(|a, b| a.fingerprint_sha256.cmp(&b.fingerprint_sha256));
    certs.dedup_by(|a, b| a.fingerprint_sha256 == b.fingerprint_sha256);
    certs.sort_by_key(|c| c.subject.to_lowercase());
    Ok(certs)
}
