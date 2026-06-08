//! Server-certificate deployment to web / proxy services.
//!
//! Each service has a fixed, helper-owned directory and a reload command. The
//! privileged side ([`deploy`] / [`remove_deployment`]) writes the leaf+chain
//! and the private key (mode 0600) there and best-effort reloads the service.
//! [`list_deployments`] is an unprivileged read of what LCM has deployed.
//!
//! Deploy layout: `<service-dir>/lcm-<name>/` with `fullchain.crt` and
//! `privkey.key`.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

use crate::cert::{self, CertInfo};
use crate::error::{Error, Result};
use crate::plan::{sanitize_name, ANCHOR_PREFIX};

struct Svc {
    id: &'static str,
    label: &'static str,
    bin: &'static str,
    dir: &'static str,
    reload: &'static str,
    /// Optional config-validation command, run before reloading.
    test: Option<&'static str>,
}

const SERVICES: &[Svc] = &[
    Svc { id: "nginx", label: "nginx", bin: "nginx", dir: "/etc/nginx/certs", reload: "systemctl reload nginx", test: Some("nginx -t") },
    Svc { id: "apache", label: "Apache", bin: "apache2", dir: "/etc/ssl/lcm/apache", reload: "systemctl reload apache2", test: Some("apache2ctl configtest") },
    Svc { id: "haproxy", label: "HAProxy", bin: "haproxy", dir: "/etc/haproxy/certs", reload: "systemctl reload haproxy", test: None },
];

/// A deployment target the UI can offer.
#[derive(Debug, Clone, Serialize)]
pub struct ServiceTarget {
    pub id: String,
    pub label: String,
    /// Whether the service binary is present on this system.
    pub available: bool,
    pub cert_dir: String,
    pub reload: String,
}

/// A server certificate LCM has deployed to a service.
#[derive(Debug, Clone, Serialize)]
pub struct ServerDeployment {
    pub name: String,
    pub service: String,
    pub cert: Option<CertInfo>,
    pub paths: Vec<String>,
}

fn in_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(bin).is_file()))
        .unwrap_or(false)
}

fn lookup(id: &str) -> Option<&'static Svc> {
    SERVICES.iter().find(|s| s.id == id)
}

/// Enumerate deployment targets with availability detected from `$PATH`.
pub fn targets() -> Vec<ServiceTarget> {
    SERVICES
        .iter()
        .map(|s| ServiceTarget {
            id: s.id.to_string(),
            label: s.label.to_string(),
            available: in_path(s.bin),
            cert_dir: s.dir.to_string(),
            reload: s.reload.to_string(),
        })
        .collect()
}

fn deploy_dir(svc: &Svc, stem: &str) -> PathBuf {
    PathBuf::from(svc.dir).join(format!("{ANCHOR_PREFIX}{stem}"))
}

/// Deploy `cert_pem` (+ `chain_pem`) and `key_pem` for `name` to `service_id`,
/// then validate the service config and reload. Privileged.
///
/// Safety measures: the certificate and private key are checked to be a pair
/// *before* anything is written; files are staged then swapped into place
/// atomically (the previous deployment is restored if the swap fails); and the
/// service config is validated (e.g. `nginx -t`) before reloading — a failing
/// config is reported and the reload is skipped rather than breaking a running
/// service.
pub fn deploy(
    name: &str,
    service_id: &str,
    cert_pem: &str,
    key_pem: &str,
    chain_pem: &str,
) -> Result<String> {
    let svc = lookup(service_id).ok_or_else(|| Error::UnknownService(service_id.to_string()))?;
    let stem = sanitize_name(name)?;

    // 1. Reject a mismatched cert/key pair up front.
    validate_pair(cert_pem, key_pem)?;

    let base = PathBuf::from(svc.dir);
    let dir = deploy_dir(svc, &stem);
    let staging = base.join(format!(".{ANCHOR_PREFIX}{stem}.staging"));
    let backup = base.join(format!(".{ANCHOR_PREFIX}{stem}.bak"));
    std::fs::create_dir_all(&base)?;
    let _ = std::fs::remove_dir_all(&staging);

    // 2. Write into a staging dir on the same filesystem.
    std::fs::create_dir_all(&staging)?;
    let fullchain = staging.join("fullchain.crt");
    std::fs::write(&fullchain, format!("{cert_pem}{chain_pem}"))?;
    crate::util::set_mode(&fullchain, 0o644)?;
    let privkey = staging.join("privkey.key");
    std::fs::write(&privkey, key_pem)?;
    crate::util::set_mode(&privkey, 0o600)?;

    // 3. Atomically swap staging into place, backing up any previous deployment.
    let had_existing = dir.exists();
    if had_existing {
        let _ = std::fs::remove_dir_all(&backup);
        std::fs::rename(&dir, &backup)?;
    }
    if let Err(e) = std::fs::rename(&staging, &dir) {
        // Roll back: restore the previous deployment, drop staging.
        if had_existing {
            let _ = std::fs::rename(&backup, &dir);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(Error::Io(e));
    }
    let _ = std::fs::remove_dir_all(&backup);

    // 4. Validate config, then reload.
    let reload = test_and_reload(svc);
    Ok(format!("deployed to {} · {}", dir.display(), reload))
}

/// Verify the certificate and private key share the same public key.
fn validate_pair(cert_pem: &str, key_pem: &str) -> Result<()> {
    let cert_pub = openssl_pubkey(&["x509", "-pubkey", "-noout"], cert_pem)?;
    let key_pub = openssl_pubkey(&["pkey", "-pubout"], key_pem)?;
    if cert_pub.trim() == key_pub.trim() && !cert_pub.trim().is_empty() {
        Ok(())
    } else {
        Err(Error::CertParse(
            "the private key does not match the certificate".to_string(),
        ))
    }
}

/// Extract a PEM public key from PEM `input` using `openssl <args>` (stdin/out).
fn openssl_pubkey(args: &[&str], input: &str) -> Result<String> {
    use std::io::Write;
    let mut child = Command::new("openssl")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| Error::Command { cmd: format!("openssl {}", args.join(" ")), reason: e.to_string() })?;
    child.stdin.take().unwrap().write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(Error::CertParse("could not read public key (invalid PEM?)".to_string()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run the service config test (if any) then reload. All best-effort: a missing
/// service binary is fine; a *failing* config test blocks the reload.
fn test_and_reload(svc: &Svc) -> String {
    if let Some(test) = svc.test {
        match run_status(test) {
            CmdOutcome::Failed(msg) => {
                return format!("files in place, but config test failed — not reloaded ({msg})");
            }
            CmdOutcome::Missing => {} // service not installed here; nothing to reload
            CmdOutcome::Ok => {}
        }
    }
    reload_service(svc)
}

enum CmdOutcome {
    Ok,
    Failed(String),
    Missing,
}

fn run_status(cmdline: &str) -> CmdOutcome {
    let mut parts = cmdline.split_whitespace();
    let Some(program) = parts.next() else {
        return CmdOutcome::Missing;
    };
    match Command::new(program).args(parts).output() {
        Ok(o) if o.status.success() => CmdOutcome::Ok,
        Ok(o) => CmdOutcome::Failed(format!("{cmdline} exited {}", o.status).trim().to_string()
            + &{
                let e = String::from_utf8_lossy(&o.stderr);
                if e.trim().is_empty() { String::new() } else { format!(": {}", e.trim()) }
            }),
        Err(_) => CmdOutcome::Missing,
    }
}

/// Remove a previously deployed server certificate and reload. Privileged.
pub fn remove_deployment(name: &str, service_id: &str) -> Result<String> {
    let svc = lookup(service_id).ok_or_else(|| Error::UnknownService(service_id.to_string()))?;
    let stem = sanitize_name(name)?;
    let dir = deploy_dir(svc, &stem);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let reload = reload_service(svc);
    Ok(format!("removed {} · {}", dir.display(), reload))
}

fn reload_service(svc: &Svc) -> String {
    // Best-effort: a failed reload (e.g. no init system in a container) must not
    // fail the whole deploy — the files are already in place.
    match run_status(svc.reload) {
        CmdOutcome::Ok => format!("{} ok", svc.reload),
        CmdOutcome::Failed(msg) => format!("reload skipped ({msg})"),
        CmdOutcome::Missing => "reload skipped (service not running here)".to_string(),
    }
}

/// List server certificates LCM has deployed (unprivileged read).
pub fn list_deployments() -> Result<Vec<ServerDeployment>> {
    let mut out = Vec::new();
    for svc in SERVICES {
        let base = PathBuf::from(svc.dir);
        if !base.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&base)? {
            let entry = entry?;
            let fname = entry.file_name().to_string_lossy().into_owned();
            if !fname.starts_with(ANCHOR_PREFIX) || !entry.path().is_dir() {
                continue;
            }
            let name = fname.trim_start_matches(ANCHOR_PREFIX).to_string();
            let fullchain = entry.path().join("fullchain.crt");
            let cert = std::fs::read(&fullchain)
                .ok()
                .and_then(|b| cert::parse_one(&b).ok());
            out.push(ServerDeployment {
                name,
                service: svc.id.to_string(),
                cert,
                paths: vec![
                    fullchain.display().to_string(),
                    entry.path().join("privkey.key").display().to_string(),
                ],
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
