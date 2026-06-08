//! Execution of a [`Plan`] and read-only auditing.
//!
//! [`execute_plan`] is the privileged side: it runs inside the root helper
//! (or directly when already root). It re-derives the backend, re-validates
//! every certificate, and only writes inside the backend's anchor directory.
//! [`list_installed`] is unprivileged and just reads that directory.

use std::io::Write;
use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::backend::{self, TrustStoreBackend};
use crate::cert::{self, CertInfo};
use crate::error::{Error, Result};
use crate::plan::{anchor_filename, sanitize_name, Plan, PrivilegedOp, ANCHOR_PREFIX};

/// Outcome of a single [`PrivilegedOp`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpResult {
    pub op: String,
    pub ok: bool,
    pub message: String,
}

/// Execute every operation in `plan`, continuing past failures and reporting
/// each outcome. Returns an error only if no backend is available at all.
pub fn execute_plan(plan: &Plan) -> Result<Vec<OpResult>> {
    // The trust-store backend is resolved lazily, per op, so a plan that only
    // deploys a server certificate still works on systems without one.
    let mut results = Vec::with_capacity(plan.ops.len());
    for op in &plan.ops {
        let outcome = match op {
            PrivilegedOp::InstallAnchor { name, cert_der_b64 } => {
                backend::detect().and_then(|b| install_anchor(&b.anchor_dir(), name, cert_der_b64))
            }
            PrivilegedOp::RemoveAnchor { name } => {
                backend::detect().and_then(|b| remove_anchor(&b.anchor_dir(), name))
            }
            PrivilegedOp::ApplyTrust => backend::detect().and_then(|b| apply_trust(b.as_ref())),
            PrivilegedOp::DeployServer {
                name,
                service,
                cert_pem,
                key_pem,
                chain_pem,
            } => crate::service::deploy(name, service, cert_pem, key_pem, chain_pem),
            PrivilegedOp::RemoveDeployment { name, service } => {
                crate::service::remove_deployment(name, service)
            }
        };
        results.push(match outcome {
            Ok(message) => OpResult {
                op: op.label(),
                ok: true,
                message,
            },
            Err(e) => OpResult {
                op: op.label(),
                ok: false,
                message: e.to_string(),
            },
        });
    }
    Ok(results)
}

fn install_anchor(dir: &Path, name: &str, cert_der_b64: &str) -> Result<String> {
    let stem = sanitize_name(name)?;

    let der = base64::engine::general_purpose::STANDARD
        .decode(cert_der_b64)
        .map_err(|e| Error::CertParse(format!("base64: {e}")))?;

    // Re-validate as a real X.509 cert before writing anything as root.
    let info = cert::parse_one(&der)?;
    let pem = cert::to_pem(&info);

    std::fs::create_dir_all(dir)?;
    let filename = anchor_filename(&stem);
    let target = dir.join(&filename);
    let tmp = dir.join(format!(".{filename}.tmp"));

    // Atomic write: temp file + rename, world-readable (CA certs are public).
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(pem.as_bytes())?;
        f.sync_all()?;
    }
    crate::util::set_mode(&tmp, 0o644)?;
    std::fs::rename(&tmp, &target)?;

    Ok(format!("wrote {}", target.display()))
}

fn remove_anchor(dir: &Path, name: &str) -> Result<String> {
    let stem = sanitize_name(name)?;
    let filename = anchor_filename(&stem);

    // Defense in depth: only ever delete files carrying our prefix.
    debug_assert!(filename.starts_with(ANCHOR_PREFIX));
    let target = dir.join(&filename);

    if target.exists() {
        std::fs::remove_file(&target)?;
        Ok(format!("removed {}", target.display()))
    } else {
        Ok(format!("not present: {}", target.display()))
    }
}

fn apply_trust(backend: &dyn TrustStoreBackend) -> Result<String> {
    let label = backend.apply_command_str();
    let status = backend.apply_command().status().map_err(Error::Io)?;
    if status.success() {
        Ok(format!("{label} succeeded"))
    } else {
        Err(Error::Command {
            cmd: label,
            reason: format!("exit status {status}"),
        })
    }
}

/// A CA anchor that LCM installed in the system trust store.
#[derive(Debug, Clone, Serialize)]
pub struct InstalledAnchor {
    pub name: String,
    pub path: String,
    /// Parsed certificate metadata, if the file could be read and parsed.
    pub cert: Option<CertInfo>,
}

/// List the LCM-managed anchors currently installed (read-only, unprivileged).
pub fn list_installed() -> Result<Vec<InstalledAnchor>> {
    let backend = backend::detect()?;
    let dir = backend.anchor_dir();

    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().into_owned();
        if !filename.starts_with(ANCHOR_PREFIX) || !filename.ends_with(".crt") {
            continue;
        }
        let name = filename
            .trim_start_matches(ANCHOR_PREFIX)
            .trim_end_matches(".crt")
            .to_string();
        let cert = std::fs::read(entry.path())
            .ok()
            .and_then(|bytes| cert::parse_one(&bytes).ok());
        out.push(InstalledAnchor {
            name,
            path: entry.path().display().to_string(),
            cert,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
