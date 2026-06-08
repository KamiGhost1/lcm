//! LCM desktop GUI — Tauri shell.
//!
//! The window content is the React frontend in `../`. All real work is done by
//! `lcm-core`; this file just exposes a handful of `#[tauri::command]`s and
//! routes privileged operations to the root helper.
//!
//! Privilege model mirrors the CLI: when the app runs as root the plan is
//! executed inline; otherwise it is handed to `pkexec lcm helper --json`, which
//! triggers the polkit authorization dialog. The path to the `lcm` helper binary
//! can be overridden with the `LCM_HELPER` environment variable.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;
use std::process::{Command, Stdio};

use base64::Engine;
use serde::Serialize;

use lcm_core::bundle::{self, Material};
use lcm_core::cert::CertInfo;
use lcm_core::exec::{InstalledAnchor, OpResult};
use lcm_core::identity::{self, ClientIdentity};
use lcm_core::plan::{Plan, PrivilegedOp};
use lcm_core::service::{self, ServerDeployment, ServiceTarget};
use lcm_core::{backend, cert, distro, exec, plan};

#[derive(Serialize)]
struct SystemInfo {
    distro_family: String,
    anchor_dir: String,
    apply_command: String,
    is_root: bool,
    supported: bool,
}

fn is_root() -> bool {
    lcm_core::util::is_elevated()
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("base64: {e}"))
}

#[tauri::command]
fn system_info() -> SystemInfo {
    let family = distro::detect();
    let supported = family != distro::DistroFamily::Unsupported;
    let (anchor_dir, apply_command) = match backend::for_family(family) {
        Ok(b) => (b.anchor_dir().display().to_string(), b.apply_command_str()),
        Err(_) => (String::new(), String::new()),
    };
    SystemInfo {
        distro_family: format!("{family:?}"),
        anchor_dir,
        apply_command,
        is_root: is_root(),
        supported,
    }
}

#[tauri::command]
fn list_anchors() -> Result<Vec<InstalledAnchor>, String> {
    exec::list_installed().map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_cert(bytes_b64: String) -> Result<Vec<CertInfo>, String> {
    let bytes = b64_decode(&bytes_b64)?;
    cert::parse(&bytes).map_err(|e| e.to_string())
}

/// Read-only audit of every CA the system trusts.
#[tauri::command]
fn list_system_trust() -> Result<Vec<CertInfo>, String> {
    lcm_core::trust::list_system_trust().map_err(|e| e.to_string())
}

/// Browser NSS databases LCM can import into (Firefox/Chrome/Zen/…).
#[tauri::command]
fn nss_databases() -> Vec<lcm_core::nss::NssDb> {
    lcm_core::nss::databases()
}

/// Re-import all managed CAs + identities into every discovered browser store.
#[tauri::command]
fn nss_sync() -> Vec<OpResult> {
    lcm_core::nss::sync()
}

#[tauri::command]
fn install_ca(name: String, bytes_b64: String, nss: bool) -> Result<Vec<OpResult>, String> {
    let bytes = b64_decode(&bytes_b64)?;
    let info = cert::parse_one(&bytes).map_err(|e| e.to_string())?;
    let stem = plan::sanitize_name(&name).map_err(|e| e.to_string())?;

    let mut p = Plan::new();
    p.push(PrivilegedOp::InstallAnchor {
        name: stem.clone(),
        cert_der_b64: base64::engine::general_purpose::STANDARD.encode(&info.der),
    });
    p.push(PrivilegedOp::ApplyTrust);
    let mut results = run_plan(&p)?;

    // NSS import runs here (as the user), never in the root helper.
    if nss {
        let pem = cert::to_pem(&info);
        for (label, r) in lcm_core::nss::import_ca_all(pem.as_bytes(), &format!("LCM {stem}")) {
            results.push(nss_opresult(label, r));
        }
    }
    Ok(results)
}

fn nss_opresult(label: String, r: lcm_core::Result<String>) -> OpResult {
    match r {
        Ok(_) => OpResult { op: format!("NSS: {label}"), ok: true, message: "imported".to_string() },
        Err(e) => OpResult { op: format!("NSS: {label}"), ok: false, message: e.to_string() },
    }
}

#[tauri::command]
fn remove_ca(name: String) -> Result<Vec<OpResult>, String> {
    let stem = plan::sanitize_name(&name).map_err(|e| e.to_string())?;
    let mut p = Plan::new();
    p.push(PrivilegedOp::RemoveAnchor { name: stem });
    p.push(PrivilegedOp::ApplyTrust);
    run_plan(&p)
}

// ---- client identities (user-level) ----

#[tauri::command]
fn parse_material(bytes_b64: String, password: Option<String>) -> Result<Material, String> {
    let bytes = b64_decode(&bytes_b64)?;
    bundle::parse_material(&bytes, password.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_identities() -> Result<Vec<ClientIdentity>, String> {
    identity::list().map_err(|e| e.to_string())
}

#[tauri::command]
fn import_identity(
    name: String,
    bytes_b64: String,
    password: Option<String>,
    nss: bool,
) -> Result<Vec<OpResult>, String> {
    let bytes = b64_decode(&bytes_b64)?;
    let material = bundle::parse_material(&bytes, password.as_deref()).map_err(|e| e.to_string())?;
    let id = identity::import(&name, &material).map_err(|e| e.to_string())?;

    let mut results = vec![OpResult {
        op: format!("store identity {:?}", id.name),
        ok: true,
        message: id.path.clone(),
    }];
    if nss {
        // Surface NSS results so a failed browser import isn't silent.
        for (label, r) in lcm_core::nss::import_identity_all(&material) {
            results.push(nss_opresult(label, r));
        }
    }
    Ok(results)
}

#[tauri::command]
fn remove_identity(name: String) -> Result<(), String> {
    identity::remove(&name).map_err(|e| e.to_string())
}

// ---- server certificates ----

#[tauri::command]
fn list_services() -> Vec<ServiceTarget> {
    service::targets()
}

#[tauri::command]
fn list_deployments() -> Result<Vec<ServerDeployment>, String> {
    service::list_deployments().map_err(|e| e.to_string())
}

#[tauri::command]
fn deploy_server(
    name: String,
    service: String,
    bytes_b64: String,
    password: Option<String>,
) -> Result<Vec<OpResult>, String> {
    let bytes = b64_decode(&bytes_b64)?;
    let material = bundle::parse_material(&bytes, password.as_deref()).map_err(|e| e.to_string())?;
    let key_pem = material.key_pem.clone().ok_or("a private key is required")?;

    let mut p = Plan::new();
    p.push(PrivilegedOp::DeployServer {
        name,
        service,
        cert_pem: material.leaf_pem.clone(),
        key_pem,
        chain_pem: material.chain_pem.clone(),
    });
    run_plan(&p)
}

#[tauri::command]
fn remove_deployment(name: String, service: String) -> Result<Vec<OpResult>, String> {
    let mut p = Plan::new();
    p.push(PrivilegedOp::RemoveDeployment { name, service });
    run_plan(&p)
}

/// Execute a privileged plan: inline when root, else via `pkexec lcm helper --json`.
fn run_plan(p: &Plan) -> Result<Vec<OpResult>, String> {
    if is_root() {
        return exec::execute_plan(p).map_err(|e| e.to_string());
    }

    let json = p.to_json().map_err(|e| e.to_string())?;
    let helper = std::env::var("LCM_HELPER").unwrap_or_else(|_| "/usr/bin/lcm".to_string());

    let mut child = Command::new("pkexec")
        .arg("--disable-internal-agent")
        .arg(&helper)
        .arg("helper")
        .arg("--json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launching pkexec (is polkit installed?): {e}"))?;

    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(json.as_bytes())
        .map_err(|e| e.to_string())?;

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout);

    // The helper prints a JSON array of OpResult on its last line.
    let line = stdout.lines().rev().find(|l| l.trim_start().starts_with('['));
    match line {
        Some(l) => serde_json::from_str::<Vec<OpResult>>(l).map_err(|e| e.to_string()),
        None => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if out.status.success() {
                Err("helper produced no result".to_string())
            } else {
                // Most commonly: the user dismissed the polkit dialog.
                Err(format!("authorization failed or was cancelled: {}", stderr.trim()))
            }
        }
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            system_info,
            list_anchors,
            parse_cert,
            list_system_trust,
            nss_databases,
            nss_sync,
            install_ca,
            remove_ca,
            parse_material,
            list_identities,
            import_identity,
            remove_identity,
            list_services,
            list_deployments,
            deploy_server,
            remove_deployment
        ])
        .run(tauri::generate_context!())
        .expect("error while running LCM");
}
