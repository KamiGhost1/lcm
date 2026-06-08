//! PKCS#12 (`.p12` / `.pfx`) → PEM conversion via the system `openssl` binary.
//!
//! Parsing PKCS#12 natively in Rust is fiddly (nested ASN.1, multiple
//! encryption schemes). Shelling out to `openssl pkcs12` is robust and avoids a
//! heavy crypto dependency. The passphrase is passed through the environment so
//! it never appears in the process argument list (`ps`).

use std::process::Command;

use crate::error::{Error, Result};

/// Heuristic: PEM bundles start with an ASCII-armor header; PKCS#12 is binary.
pub fn looks_like_pem(input: &[u8]) -> bool {
    input.windows(b"-----BEGIN".len()).any(|w| w == b"-----BEGIN")
}

/// Convert PKCS#12 bytes to a PEM bundle (certificate chain + unencrypted
/// private key). `password` may be empty for password-less files.
pub fn to_pem(p12: &[u8], password: &str) -> Result<Vec<u8>> {
    // Write to a private temp file: `openssl pkcs12` wants a seekable input,
    // which a pipe is not. 0600 so the encrypted bundle isn't world-readable.
    let tmp = std::env::temp_dir().join(format!("lcm-{}-{}.p12", std::process::id(), unique()));
    std::fs::write(&tmp, p12)?;
    crate::util::set_mode(&tmp, 0o600)?;

    let result = Command::new("openssl")
        .args([
            "pkcs12",
            "-in",
            &tmp.to_string_lossy(),
            "-nodes",
            "-passin",
            "env:LCM_P12_PASS",
        ])
        .env("LCM_P12_PASS", password)
        .output();

    let _ = std::fs::remove_file(&tmp);

    let out = result.map_err(|e| Error::Command {
        cmd: "openssl pkcs12".to_string(),
        reason: e.to_string(),
    })?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(Error::CertParse(format!(
            "PKCS#12 decode failed (wrong password, or not a .p12?): {}",
            err.trim()
        )));
    }
    Ok(out.stdout)
}

/// A small per-call nonce so concurrent conversions don't collide on the temp path.
fn unique() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
