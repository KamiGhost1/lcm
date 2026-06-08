//! Declarative plan of *privileged* operations.
//!
//! A front end never writes to the system trust store directly. Instead it
//! builds a [`Plan`] — a list of [`PrivilegedOp`] — and hands it to the root
//! helper (over stdin as JSON). The helper re-validates everything before
//! acting. This module is the shared contract between the two sides.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A single privileged operation the helper knows how to perform.
///
/// Note that an operation references a CA anchor only by a logical `name`; the
/// helper maps that to a path inside the backend-owned directory. A front end
/// cannot ask the helper to write to an arbitrary location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PrivilegedOp {
    /// Install `cert_der_b64` (base64 DER) as a system trust anchor named `name`.
    InstallAnchor { name: String, cert_der_b64: String },
    /// Remove a previously installed anchor by `name`.
    RemoveAnchor { name: String },
    /// Refresh the system trust store (`update-ca-certificates`, etc.).
    ApplyTrust,
    /// Deploy a server certificate (+ key, + chain) to a service and reload.
    DeployServer {
        name: String,
        service: String,
        cert_pem: String,
        key_pem: String,
        chain_pem: String,
    },
    /// Remove a previously deployed server certificate from a service.
    RemoveDeployment { name: String, service: String },
}

impl PrivilegedOp {
    /// Short label for previews and result reporting (never includes secrets).
    pub fn label(&self) -> String {
        match self {
            PrivilegedOp::InstallAnchor { name, .. } => format!("install anchor {name:?}"),
            PrivilegedOp::RemoveAnchor { name } => format!("remove anchor {name:?}"),
            PrivilegedOp::ApplyTrust => "apply trust store".to_string(),
            PrivilegedOp::DeployServer { name, service, .. } => {
                format!("deploy {name:?} to {service}")
            }
            PrivilegedOp::RemoveDeployment { name, service } => {
                format!("remove {name:?} from {service}")
            }
        }
    }
}

/// An ordered batch of privileged operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub ops: Vec<PrivilegedOp>,
}

impl Plan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, op: PrivilegedOp) {
        self.ops.push(op);
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

/// Prefix on every LCM-managed anchor file, so we can list and remove our own
/// anchors without ever touching distro- or admin-provided ones.
pub const ANCHOR_PREFIX: &str = "lcm-";

/// Reduce a user-supplied logical name to a safe filename stem.
///
/// Disallowed characters become `-`; the result is trimmed of leading/trailing
/// `-` and must be non-empty and not `.`/`..`. This is the single chokepoint
/// that prevents path traversal in anchor names.
pub fn sanitize_name(name: &str) -> Result<String> {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    // Trim separators and dots from both ends so traversal-ish inputs like
    // "../../etc/passwd" reduce to a clean "etc-passwd" rather than "..-..-…".
    let stem = stem.trim_matches(|c| c == '-' || c == '.').to_string();
    if stem.is_empty() || stem == "." || stem == ".." {
        return Err(Error::InvalidName(name.to_string()));
    }
    Ok(stem)
}

/// On-disk filename for an anchor with the given (already-sanitized) stem.
pub fn anchor_filename(stem: &str) -> String {
    format!("{ANCHOR_PREFIX}{stem}.crt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_traversal() {
        assert_eq!(sanitize_name("../../etc/passwd").unwrap(), "etc-passwd");
        assert_eq!(sanitize_name("corp root CA").unwrap(), "corp-root-CA");
        assert_eq!(sanitize_name("ok_name-1.2").unwrap(), "ok_name-1.2");
    }

    #[test]
    fn sanitize_rejects_empty_and_dots() {
        assert!(sanitize_name("").is_err());
        assert!(sanitize_name("///").is_err());
        assert!(sanitize_name("..").is_err());
    }

    #[test]
    fn anchor_filename_is_prefixed_crt() {
        assert_eq!(anchor_filename("corp"), "lcm-corp.crt");
    }

    #[test]
    fn plan_json_roundtrip() {
        let mut p = Plan::new();
        p.push(PrivilegedOp::InstallAnchor {
            name: "corp".into(),
            cert_der_b64: "AAAA".into(),
        });
        p.push(PrivilegedOp::ApplyTrust);
        let json = p.to_json().unwrap();
        let back = Plan::from_json(&json).unwrap();
        assert_eq!(back.ops, p.ops);
    }
}
