//! Browser NSS database integration (user-level).
//!
//! Browsers do NOT use the system trust store — they keep their own NSS DB.
//! Chromium/Chrome use the shared `~/.pki/nssdb`; Firefox keeps a per-profile
//! DB. On Ubuntu both Firefox and Chromium are often **Snap** (or Flatpak)
//! packages whose profiles live under `~/snap/...` / `~/.var/app/...`, so we
//! scan those locations too. We drive `certutil` (CA certs) and `pk12util`
//! (client identities) from `libnss3-tools`.
//!
//! These operations must target the *invoking user's* home. They normally run
//! in the unprivileged front-end process; if invoked as root (e.g. `sudo lcm
//! … --nss`) we resolve the real user from `SUDO_USER` and run the tools via
//! `runuser` so the DB and its files stay owned by that user.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::bundle::Material;
use crate::error::{Error, Result};
use crate::exec::{self, OpResult};
use crate::identity;

/// A discovered NSS database.
#[derive(Debug, Clone, Serialize)]
pub struct NssDb {
    pub label: String,
    /// Directory holding `cert9.db` (used as `sql:<dir>` with the tools).
    pub dir: String,
}

/// Who we act as, and where their home is.
struct UserCtx {
    home: PathBuf,
    /// `Some((uid, username))` when we are root and must drop to a real user.
    run_as: Option<(u32, String)>,
}

fn home_env() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/root"))
}

/// Resolve a user's home directory from `/etc/passwd`.
fn passwd_home(user: &str) -> Option<PathBuf> {
    let data = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in data.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.first() == Some(&user) && f.len() >= 6 && !f[5].is_empty() {
            return Some(PathBuf::from(f[5]));
        }
    }
    None
}

fn user_ctx() -> UserCtx {
    if crate::util::is_elevated() {
        // Running as root: the NSS store belongs to the user who invoked us.
        if let Ok(user) = std::env::var("SUDO_USER") {
            if user != "root" {
                let uid = std::env::var("SUDO_UID").ok().and_then(|s| s.parse::<u32>().ok());
                let home = passwd_home(&user);
                if let (Some(uid), Some(home)) = (uid, home) {
                    return UserCtx { home, run_as: Some((uid, user)) };
                }
            }
        }
    }
    UserCtx { home: home_env(), run_as: None }
}

/// Build a command, dropping to the target user via `runuser` when required.
fn cmd_as(ctx: &UserCtx, program: &str, args: &[&str]) -> Command {
    if let Some((_, user)) = &ctx.run_as {
        let mut c = Command::new("runuser");
        c.arg("-u").arg(user).arg("--").arg(program).args(args);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    }
}

/// Hand a temp file we created as root to the target user so `runuser` can read it.
fn handoff(ctx: &UserCtx, path: &Path) {
    if let Some((uid, _)) = &ctx.run_as {
        let _ = Command::new("chown").arg(uid.to_string()).arg(path).status();
    }
}

fn firefox_profiles(root: &Path, label_prefix: &str, out: &mut Vec<NssDb>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            if e.path().join("cert9.db").exists() {
                let name = e.file_name().to_string_lossy().into_owned();
                out.push(NssDb { label: format!("{label_prefix}: {name}"), dir: e.path().display().to_string() });
            }
        }
    }
}

fn discover(ctx: &UserCtx) -> Vec<NssDb> {
    let h = &ctx.home;
    let mut out = Vec::new();

    // Shared NSS DBs used by Chromium-family browsers. Native .deb installs
    // (Chrome, Chromium, Brave, Vivaldi, Edge) all share ~/.pki/nssdb; Snap /
    // Flatpak builds are confined and keep their own under ~/snap | ~/.var/app.
    let shared: &[(&str, PathBuf)] = &[
        ("Shared (~/.pki/nssdb)", h.join(".pki/nssdb")),
        ("Chromium (snap)", h.join("snap/chromium/current/.pki/nssdb")),
        ("Chromium (flatpak)", h.join(".var/app/org.chromium.Chromium/.pki/nssdb")),
        ("Brave (flatpak)", h.join(".var/app/com.brave.Browser/.pki/nssdb")),
    ];
    for (label, dir) in shared {
        if dir.join("cert9.db").exists() {
            out.push(NssDb { label: label.to_string(), dir: dir.display().to_string() });
        }
    }

    // Gecko-family browsers (Firefox, Zen, LibreWolf, …) — one cert9.db per
    // profile under a per-browser profiles root.
    let gecko_roots: &[(&str, PathBuf)] = &[
        ("Firefox", h.join(".mozilla/firefox")),
        ("Firefox (snap)", h.join("snap/firefox/common/.mozilla/firefox")),
        ("Firefox (flatpak)", h.join(".var/app/org.mozilla.firefox/.mozilla/firefox")),
        ("Zen", h.join(".zen")),
        ("Zen (flatpak)", h.join(".var/app/app.zen_browser.zen/.zen")),
        ("LibreWolf", h.join(".librewolf")),
        ("LibreWolf (flatpak)", h.join(".var/app/io.gitlab.librewolf_community/.librewolf")),
    ];
    for (label, root) in gecko_roots {
        firefox_profiles(root, label, &mut out);
    }

    out
}

/// Discover NSS databases for the invoking user (for `lcm nss list`).
pub fn databases() -> Vec<NssDb> {
    discover(&user_ctx())
}

/// Ensure a shared `~/.pki/nssdb` exists (so Chrome/Chromium have a store and
/// there is always at least one import target). Best-effort.
fn ensure_shared_db(ctx: &UserCtx) -> Option<NssDb> {
    let dir = ctx.home.join(".pki/nssdb");
    let dir_s = dir.display().to_string();
    if dir.join("cert9.db").exists() {
        return Some(NssDb { label: "Shared (~/.pki/nssdb)".to_string(), dir: dir_s });
    }
    let _ = cmd_as(ctx, "mkdir", &["-p", &dir_s]).status();
    let ok = cmd_as(ctx, "certutil", &["-N", "-d", &format!("sql:{dir_s}"), "--empty-password"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok || dir.join("cert9.db").exists() {
        Some(NssDb { label: "Shared (~/.pki/nssdb)".to_string(), dir: dir_s })
    } else {
        None
    }
}

fn run(mut cmd: Command, what: &str) -> Result<()> {
    let out = cmd.output().map_err(|e| {
        let reason = if e.kind() == std::io::ErrorKind::NotFound {
            let tool = what.split_whitespace().next().unwrap_or(what);
            format!("'{tool}' not found — install libnss3-tools (Debian/Ubuntu) or nss-tools (Fedora)")
        } else {
            e.to_string()
        };
        Error::Command { cmd: what.to_string(), reason }
    })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Command { cmd: what.to_string(), reason: String::from_utf8_lossy(&out.stderr).trim().to_string() })
    }
}

fn import_ca(ctx: &UserCtx, db_dir: &str, cert_pem: &[u8], nickname: &str) -> Result<String> {
    let tmp = std::env::temp_dir().join(format!("lcm-nss-{}.crt", std::process::id()));
    std::fs::write(&tmp, cert_pem)?;
    crate::util::set_mode(&tmp, 0o644)?;
    handoff(ctx, &tmp);
    let res = run(
        cmd_as(ctx, "certutil", &[
            "-d", &format!("sql:{db_dir}"), "-A", "-n", nickname, "-t", "C,,", "-i", &tmp.to_string_lossy(),
        ]),
        "certutil -A",
    );
    let _ = std::fs::remove_file(&tmp);
    res.map(|_| format!("imported into {db_dir}"))
}

fn import_identity(ctx: &UserCtx, db_dir: &str, material: &Material, p12: &Path) -> Result<String> {
    run(
        cmd_as(ctx, "pk12util", &["-d", &format!("sql:{db_dir}"), "-i", &p12.to_string_lossy(), "-W", ""]),
        "pk12util -i",
    )
    .map(|_| format!("imported into {db_dir} ({})", material.leaf.subject))
}

/// Import a CA into a freshly-ensured shared DB plus every discovered DB.
pub fn import_ca_all(cert_pem: &[u8], nickname: &str) -> Vec<(String, Result<String>)> {
    let ctx = user_ctx();
    let mut dbs = Vec::new();
    if let Some(shared) = ensure_shared_db(&ctx) {
        dbs.push(shared);
    }
    for db in discover(&ctx) {
        if !dbs.iter().any(|d| d.dir == db.dir) {
            dbs.push(db);
        }
    }
    dbs.into_iter().map(|db| (db.label, import_ca(&ctx, &db.dir, cert_pem, nickname))).collect()
}

fn to_op(op: String, r: Result<String>) -> OpResult {
    match r {
        Ok(message) => OpResult { op, ok: true, message },
        Err(e) => OpResult { op, ok: false, message: e.to_string() },
    }
}

fn load_identity_material(dir: &str) -> Result<Material> {
    let dir = Path::new(dir);
    let mut combined = std::fs::read(dir.join("cert.pem"))?;
    if let Ok(key) = std::fs::read(dir.join("key.pem")) {
        combined.push(b'\n');
        combined.extend_from_slice(&key);
    }
    crate::bundle::parse_material(&combined, None)
}

/// Re-import every LCM-managed CA anchor and client identity into all currently
/// discovered browser NSS databases. Use this after installing a new browser so
/// it picks up certificates added earlier. Returns one result row per (item, DB).
pub fn sync() -> Vec<OpResult> {
    let mut results = Vec::new();

    if let Ok(anchors) = exec::list_installed() {
        for a in anchors {
            match std::fs::read(&a.path) {
                Ok(pem) => {
                    for (label, r) in import_ca_all(&pem, &format!("LCM {}", a.name)) {
                        results.push(to_op(format!("CA {} → {label}", a.name), r));
                    }
                }
                Err(e) => results.push(to_op(format!("CA {}", a.name), Err(Error::Io(e)))),
            }
        }
    }

    if let Ok(ids) = identity::list() {
        for id in ids {
            match load_identity_material(&id.path) {
                Ok(material) => {
                    for (label, r) in import_identity_all(&material) {
                        results.push(to_op(format!("ID {} → {label}", id.name), r));
                    }
                }
                Err(e) => results.push(to_op(format!("ID {}", id.name), Err(e))),
            }
        }
    }

    results
}

/// Import a client identity into the ensured shared DB plus every discovered DB.
pub fn import_identity_all(material: &Material) -> Vec<(String, Result<String>)> {
    let ctx = user_ctx();
    let Some(key) = material.key_pem.as_ref() else {
        return vec![("NSS".to_string(), Err(Error::MissingKey))];
    };

    // Build one transient PKCS#12 (empty password) and reuse it for every DB.
    let base = std::env::temp_dir().join(format!("lcm-nss-{}", std::process::id()));
    let crt = base.with_extension("crt");
    let keyf = base.with_extension("key");
    let p12 = base.with_extension("p12");
    let cleanup = || {
        let _ = std::fs::remove_file(&crt);
        let _ = std::fs::remove_file(&keyf);
        let _ = std::fs::remove_file(&p12);
    };
    if std::fs::write(&crt, format!("{}{}", material.leaf_pem, material.chain_pem)).is_err()
        || std::fs::write(&keyf, key).is_err()
    {
        cleanup();
        return vec![("NSS".to_string(), Err(Error::Command { cmd: "write temp".into(), reason: "io".into() }))];
    }
    let _ = crate::util::set_mode(&keyf, 0o600);
    if let Err(e) = run(
        {
            let mut c = Command::new("openssl");
            c.args([
                "pkcs12", "-export", "-inkey", &keyf.to_string_lossy(), "-in", &crt.to_string_lossy(),
                "-out", &p12.to_string_lossy(), "-passout", "pass:",
            ]);
            c
        },
        "openssl pkcs12 -export",
    ) {
        cleanup();
        return vec![("NSS".to_string(), Err(e))];
    }
    crate::util::set_mode(&p12, 0o644).ok();
    handoff(&ctx, &p12);

    let mut dbs = Vec::new();
    if let Some(shared) = ensure_shared_db(&ctx) {
        dbs.push(shared);
    }
    for db in discover(&ctx) {
        if !dbs.iter().any(|d| d.dir == db.dir) {
            dbs.push(db);
        }
    }
    let results = dbs
        .into_iter()
        .map(|db| (db.label, import_identity(&ctx, &db.dir, material, &p12)))
        .collect();
    cleanup();
    results
}
