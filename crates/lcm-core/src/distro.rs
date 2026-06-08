//! Distribution-family detection from `os-release`.

use std::collections::HashMap;

use crate::osrelease;

/// A family of distributions that share a trust-store layout and tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistroFamily {
    /// Debian, Ubuntu, Mint, … — `/usr/local/share/ca-certificates` + `update-ca-certificates`.
    Debian,
    /// Fedora, RHEL, Rocky, Alma, … — `/etc/pki/ca-trust/source/anchors` + `update-ca-trust`.
    Fedora,
    /// Arch, Manjaro, … — `/etc/ca-certificates/trust-source/anchors` + `update-ca-trust`.
    Arch,
    /// openSUSE / SLE — `/etc/pki/trust/anchors` + `update-ca-certificates`.
    Suse,
    /// Alpine — `/usr/local/share/ca-certificates` + `update-ca-certificates`.
    Alpine,
    /// Anything we don't have a trust-store backend for.
    Unsupported,
}

impl DistroFamily {
    pub fn is_supported(self) -> bool {
        self != DistroFamily::Unsupported
    }
}

// `ID` values per family. Detection also consults `ID_LIKE` (see below).
const DEBIAN_IDS: &[&str] = &[
    "debian", "ubuntu", "linuxmint", "pop", "raspbian", "devuan", "kali", "elementary", "neon", "zorin",
];
const FEDORA_IDS: &[&str] = &[
    "fedora", "rhel", "centos", "rocky", "almalinux", "ol", "oracle", "amzn", "scientific",
];
const ARCH_IDS: &[&str] = &["arch", "archarm", "manjaro", "endeavouros", "garuda", "artix", "cachyos"];
const SUSE_IDS: &[&str] = &["opensuse", "opensuse-leap", "opensuse-tumbleweed", "sles", "sled", "sle-micro"];
const ALPINE_IDS: &[&str] = &["alpine", "postmarketos"];

/// Detect the family of the running system.
pub fn detect() -> DistroFamily {
    detect_from(&osrelease::read().unwrap_or_default())
}

/// Detect the family from an already-parsed `os-release` map (testable).
pub fn detect_from(os_release: &HashMap<String, String>) -> DistroFamily {
    let id = os_release.get("ID").map(|s| s.to_lowercase()).unwrap_or_default();
    let id_like = os_release.get("ID_LIKE").map(|s| s.to_lowercase()).unwrap_or_default();
    let like = |needles: &[&str]| id_like.split_whitespace().any(|t| needles.contains(&t));

    // Exact ID first, then ID_LIKE hints. Order matters only for distros that
    // could plausibly hint at several families; the ID check disambiguates.
    if DEBIAN_IDS.contains(&id.as_str()) || like(&["debian", "ubuntu"]) {
        DistroFamily::Debian
    } else if FEDORA_IDS.contains(&id.as_str()) || like(&["fedora", "rhel", "centos"]) {
        DistroFamily::Fedora
    } else if ARCH_IDS.contains(&id.as_str()) || like(&["arch"]) {
        DistroFamily::Arch
    } else if SUSE_IDS.contains(&id.as_str()) || like(&["suse", "opensuse"]) {
        DistroFamily::Suse
    } else if ALPINE_IDS.contains(&id.as_str()) || like(&["alpine"]) {
        DistroFamily::Alpine
    } else {
        DistroFamily::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn detects_each_family_by_id() {
        assert_eq!(detect_from(&map(&[("ID", "ubuntu")])), DistroFamily::Debian);
        assert_eq!(detect_from(&map(&[("ID", "fedora")])), DistroFamily::Fedora);
        assert_eq!(detect_from(&map(&[("ID", "arch")])), DistroFamily::Arch);
        assert_eq!(detect_from(&map(&[("ID", "opensuse-tumbleweed")])), DistroFamily::Suse);
        assert_eq!(detect_from(&map(&[("ID", "alpine")])), DistroFamily::Alpine);
    }

    #[test]
    fn detects_by_id_like() {
        assert_eq!(detect_from(&map(&[("ID", "linuxmint"), ("ID_LIKE", "ubuntu debian")])), DistroFamily::Debian);
        assert_eq!(detect_from(&map(&[("ID", "rocky"), ("ID_LIKE", "rhel centos fedora")])), DistroFamily::Fedora);
        assert_eq!(detect_from(&map(&[("ID", "manjaro"), ("ID_LIKE", "arch")])), DistroFamily::Arch);
        assert_eq!(detect_from(&map(&[("ID", "sled"), ("ID_LIKE", "suse")])), DistroFamily::Suse);
    }

    #[test]
    fn unknown_is_unsupported() {
        assert_eq!(detect_from(&map(&[("ID", "haiku")])), DistroFamily::Unsupported);
        assert_eq!(detect_from(&map(&[])), DistroFamily::Unsupported);
    }
}
