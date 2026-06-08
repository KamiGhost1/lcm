//! Trust-store backends.
//!
//! A backend encapsulates everything distro-specific about the *system* trust
//! store: where CA anchors are dropped and which command refreshes the store.
//! The privileged helper picks a backend itself (never trusting a front end)
//! and writes only inside [`TrustStoreBackend::anchor_dir`].

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::distro::DistroFamily;
use crate::error::{Error, Result};

pub trait TrustStoreBackend {
    /// The family this backend serves.
    fn family(&self) -> DistroFamily;

    /// Directory CA anchors are dropped into. The helper only ever writes here.
    fn anchor_dir(&self) -> PathBuf;

    /// A ready-to-run command that refreshes the system trust store.
    fn apply_command(&self) -> Command;

    /// Human-readable form of [`Self::apply_command`], for plan previews.
    fn apply_command_str(&self) -> String;
}

/// Per-family trust-store layout: where anchors live and how to apply them.
struct Layout {
    family: DistroFamily,
    anchor_dir: &'static str,
    apply: &'static str,
}

const LAYOUTS: &[Layout] = &[
    Layout { family: DistroFamily::Debian, anchor_dir: "/usr/local/share/ca-certificates", apply: "update-ca-certificates" },
    Layout { family: DistroFamily::Fedora, anchor_dir: "/etc/pki/ca-trust/source/anchors", apply: "update-ca-trust" },
    Layout { family: DistroFamily::Arch, anchor_dir: "/etc/ca-certificates/trust-source/anchors", apply: "update-ca-trust" },
    Layout { family: DistroFamily::Suse, anchor_dir: "/etc/pki/trust/anchors", apply: "update-ca-certificates" },
    Layout { family: DistroFamily::Alpine, anchor_dir: "/usr/local/share/ca-certificates", apply: "update-ca-certificates" },
];

/// A backend instantiated from a [`Layout`] row.
struct GenericBackend(&'static Layout);

impl TrustStoreBackend for GenericBackend {
    fn family(&self) -> DistroFamily {
        self.0.family
    }
    fn anchor_dir(&self) -> PathBuf {
        Path::new(self.0.anchor_dir).to_path_buf()
    }
    fn apply_command(&self) -> Command {
        Command::new(self.0.apply)
    }
    fn apply_command_str(&self) -> String {
        self.0.apply.to_string()
    }
}

/// Resolve the backend for a given family.
pub fn for_family(family: DistroFamily) -> Result<Box<dyn TrustStoreBackend>> {
    LAYOUTS
        .iter()
        .find(|l| l.family == family)
        .map(|l| Box::new(GenericBackend(l)) as Box<dyn TrustStoreBackend>)
        .ok_or_else(|| Error::UnsupportedDistro("no trust-store backend for this system".to_string()))
}

/// Resolve the backend for the running system.
pub fn detect() -> Result<Box<dyn TrustStoreBackend>> {
    for_family(crate::distro::detect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_family_has_a_backend() {
        for f in [
            DistroFamily::Debian,
            DistroFamily::Fedora,
            DistroFamily::Arch,
            DistroFamily::Suse,
            DistroFamily::Alpine,
        ] {
            let b = for_family(f).expect("backend");
            assert_eq!(b.family(), f);
            assert!(b.anchor_dir().is_absolute());
            assert!(!b.apply_command_str().is_empty());
        }
        assert!(for_family(DistroFamily::Unsupported).is_err());
    }
}
