//! Client identities — a leaf certificate + private key kept in a managed,
//! per-user store. This is all user-level (no root, no helper): browsers and
//! apps that do mTLS read identities the user owns.
//!
//! Store layout: `$XDG_DATA_HOME/lcm/identities/<name>/` with `cert.pem`
//! (leaf + chain) and, when present, `key.pem` (mode 0600).
//!
//! Importing into the browser NSS database (`certutil`/`pk12util`) is a planned
//! follow-up; v1 manages the local store.

use std::path::PathBuf;

use serde::Serialize;

use crate::bundle::Material;
use crate::cert::{self, CertInfo};
use crate::error::Result;
use crate::plan::sanitize_name;

/// A client identity in the managed store.
#[derive(Debug, Clone, Serialize)]
pub struct ClientIdentity {
    pub name: String,
    pub cert: CertInfo,
    pub has_key: bool,
    pub path: String,
}

fn store_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/root"));
            home.join(".local/share")
        });
    base.join("lcm/identities")
}

/// Import a client identity into the managed store under `name`.
pub fn import(name: &str, material: &Material) -> Result<ClientIdentity> {
    let stem = sanitize_name(name)?;
    let dir = store_dir().join(&stem);
    std::fs::create_dir_all(&dir)?;

    let cert_path = dir.join("cert.pem");
    std::fs::write(&cert_path, format!("{}{}", material.leaf_pem, material.chain_pem))?;
    crate::util::set_mode(&cert_path, 0o644)?;

    if let Some(key) = &material.key_pem {
        let key_path = dir.join("key.pem");
        std::fs::write(&key_path, key)?;
        crate::util::set_mode(&key_path, 0o600)?;
    }

    Ok(ClientIdentity {
        name: stem,
        cert: material.leaf.clone(),
        has_key: material.has_key,
        path: dir.display().to_string(),
    })
}

/// List identities currently in the managed store.
pub fn list() -> Result<Vec<ClientIdentity>> {
    let base = store_dir();
    let mut out = Vec::new();
    if !base.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&base)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let cert = std::fs::read(entry.path().join("cert.pem"))
            .ok()
            .and_then(|b| cert::parse_one(&b).ok());
        if let Some(cert) = cert {
            out.push(ClientIdentity {
                name,
                cert,
                has_key: entry.path().join("key.pem").exists(),
                path: entry.path().display().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Remove an identity from the managed store.
pub fn remove(name: &str) -> Result<()> {
    let stem = sanitize_name(name)?;
    let dir = store_dir().join(&stem);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
