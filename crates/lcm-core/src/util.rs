//! Small cross-platform helpers.

use std::path::Path;

/// Set a Unix file mode. No-op on non-Unix targets (Windows uses ACLs; the
/// Windows backend will handle permissions differently when implemented).
#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
pub(crate) fn set_mode(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

/// Whether the current process has administrative privileges.
#[cfg(unix)]
pub fn is_elevated() -> bool {
    // Safe: geteuid has no preconditions and never fails.
    unsafe { libc_geteuid() == 0 }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

#[cfg(not(unix))]
pub fn is_elevated() -> bool {
    // TODO(windows): detect an elevated token (UAC). Until the Windows backend
    // lands, report not-elevated so callers route through elevation.
    false
}
