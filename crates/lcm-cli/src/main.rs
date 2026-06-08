//! `lcm` — the Linux Cert Manager command-line front end.
//!
//! Unprivileged work (parsing, building a plan, auditing) happens in-process.
//! Privileged work is expressed as a [`Plan`] and executed by the root helper:
//! directly when already root, otherwise via `pkexec <self> helper` with the
//! plan piped over stdin. The privilege boundary lives in `lcm-core::exec`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use clap::{Parser, Subcommand};

use lcm_core::cert::{self, CertInfo};
use lcm_core::exec::{self, OpResult};
use lcm_core::plan::{sanitize_name, Plan, PrivilegedOp};
use lcm_core::{backend, bundle, distro, identity, service, trust};

#[derive(Parser)]
#[command(
    name = "lcm",
    version,
    about = "Linux Cert Manager — integrate ready-made certificates into the system"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show metadata of a certificate file (PEM or DER). No privileges needed.
    Info {
        /// Path to a `.crt`/`.pem`/`.cer`/`.der` file.
        file: PathBuf,
    },
    /// Manage CA trust anchors in the system trust store.
    Ca {
        #[command(subcommand)]
        cmd: CaCmd,
    },
    /// Manage client identities (cert + key) in the managed user store.
    Client {
        #[command(subcommand)]
        cmd: ClientCmd,
    },
    /// Deploy server certificates to web / proxy services.
    Server {
        #[command(subcommand)]
        cmd: ServerCmd,
    },
    /// Inspect browser NSS databases LCM can import into.
    Nss {
        #[command(subcommand)]
        cmd: NssCmd,
    },
    /// Internal: privileged helper. Reads a JSON plan from stdin; run as root.
    #[command(hide = true)]
    Helper {
        /// Emit results as JSON (used by the GUI); otherwise human-readable.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CaCmd {
    /// Install a CA certificate into the system trust store.
    Install {
        /// Certificate file (PEM or DER).
        file: PathBuf,
        /// Logical name for the anchor (default: derived from the file name).
        #[arg(long)]
        name: Option<String>,
        /// Print the plan and exit without changing anything.
        #[arg(long)]
        dry_run: bool,
        /// Proceed even if the certificate is not marked as a CA.
        #[arg(long)]
        force: bool,
        /// Also import the CA into browser NSS databases (Chrome/Firefox).
        #[arg(long)]
        nss: bool,
    },
    /// List LCM-managed anchors installed in the system trust store.
    List {
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Audit every CA the system trusts (read-only), not just LCM-managed ones.
    Audit {
        #[arg(long)]
        json: bool,
        /// Only show certificates expiring within this many days.
        #[arg(long)]
        expiring: Option<i64>,
    },
    /// Remove a previously installed anchor by name.
    Remove {
        /// The logical name shown by `lcm ca list`.
        #[arg(long)]
        name: String,
        /// Print the plan and exit without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum ClientCmd {
    /// Import a client identity (PEM cert+key, or a .p12/.pfx) into the store.
    Import {
        /// Certificate material (PEM bundle or PKCS#12).
        file: PathBuf,
        /// Logical name (default: derived from the certificate CN).
        #[arg(long)]
        name: Option<String>,
        /// PKCS#12 password (prefer --password-stdin to keep it out of `ps`).
        #[arg(long)]
        password: Option<String>,
        /// Read the PKCS#12 password from the first line of stdin.
        #[arg(long)]
        password_stdin: bool,
        /// Also import the identity into browser NSS databases.
        #[arg(long)]
        nss: bool,
    },
    /// List client identities in the managed store.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Remove a client identity by name.
    Remove {
        #[arg(long)]
        name: String,
    },
}

#[derive(Subcommand)]
enum ServerCmd {
    /// Deploy a server certificate (+ key, + chain) to a service and reload it.
    Deploy {
        /// Certificate material (PEM bundle or PKCS#12) — must contain a key.
        file: PathBuf,
        /// Target service: nginx | apache | haproxy.
        #[arg(long)]
        service: String,
        /// Logical name (default: derived from the certificate CN).
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        password_stdin: bool,
        /// Print the plan and exit without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// List server certificates LCM has deployed.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Remove a deployed server certificate.
    Remove {
        #[arg(long)]
        name: String,
        #[arg(long)]
        service: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum NssCmd {
    /// List discovered browser NSS databases (incl. Snap/Flatpak Firefox).
    List {
        #[arg(long)]
        json: bool,
    },
    /// Re-import all managed CAs + identities into every discovered browser.
    Sync,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Info { file } => cmd_info(&file),
        Commands::Ca { cmd } => match cmd {
            CaCmd::Install {
                file,
                name,
                dry_run,
                force,
                nss,
            } => cmd_ca_install(&file, name, dry_run, force, nss),
            CaCmd::List { json } => cmd_ca_list(json),
            CaCmd::Audit { json, expiring } => cmd_ca_audit(json, expiring),
            CaCmd::Remove { name, dry_run } => cmd_ca_remove(&name, dry_run),
        },
        Commands::Client { cmd } => match cmd {
            ClientCmd::Import { file, name, password, password_stdin, nss } => {
                cmd_client_import(&file, name, resolve_password(password, password_stdin)?, nss)
            }
            ClientCmd::List { json } => cmd_client_list(json),
            ClientCmd::Remove { name } => cmd_client_remove(&name),
        },
        Commands::Server { cmd } => match cmd {
            ServerCmd::Deploy { file, service, name, password, password_stdin, dry_run } => {
                cmd_server_deploy(&file, &service, name, resolve_password(password, password_stdin)?, dry_run)
            }
            ServerCmd::List { json } => cmd_server_list(json),
            ServerCmd::Remove { name, service, dry_run } => cmd_server_remove(&name, &service, dry_run),
        },
        Commands::Nss { cmd } => match cmd {
            NssCmd::List { json } => cmd_nss_list(json),
            NssCmd::Sync => cmd_nss_sync(),
        },
        Commands::Helper { json } => cmd_helper(json),
    }
}

fn resolve_password(password: Option<String>, from_stdin: bool) -> Result<Option<String>> {
    if from_stdin {
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).context("reading password from stdin")?;
        Ok(Some(s.trim_end_matches(['\n', '\r']).to_string()))
    } else {
        Ok(password)
    }
}

fn name_from_cn(subject: &str, fallback: &str) -> String {
    let cn = subject
        .split(',')
        .find_map(|p| p.trim().strip_prefix("CN="))
        .unwrap_or(fallback);
    sanitize_name(cn).unwrap_or_else(|_| fallback.to_string())
}

fn cmd_client_import(
    file: &PathBuf,
    name: Option<String>,
    password: Option<String>,
    nss: bool,
) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let material = bundle::parse_material(&bytes, password.as_deref())
        .with_context(|| format!("parsing {}", file.display()))?;
    let name = name.unwrap_or_else(|| name_from_cn(&material.leaf.subject, "identity"));
    let id = identity::import(&name, &material).map_err(|e| anyhow!("{e}"))?;
    println!(
        "✓ imported identity {:?} ({}, key: {})",
        id.name,
        id.cert.subject,
        if id.has_key { "yes" } else { "no" }
    );
    println!("  {}", id.path);

    if nss {
        nss_report(lcm_core::nss::import_identity_all(&material));
    }
    Ok(())
}

fn cmd_client_list(json: bool) -> Result<()> {
    let ids = identity::list().map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&ids)?);
        return Ok(());
    }
    if ids.is_empty() {
        println!("No client identities.");
        return Ok(());
    }
    for id in &ids {
        println!(
            "{:<20} {}  (key: {}, expires {})",
            id.name,
            id.cert.subject,
            if id.has_key { "yes" } else { "no" },
            id.cert.not_after
        );
    }
    Ok(())
}

fn cmd_client_remove(name: &str) -> Result<()> {
    identity::remove(name).map_err(|e| anyhow!("{e}"))?;
    println!("✓ removed identity {name:?}");
    Ok(())
}

fn cmd_server_deploy(
    file: &PathBuf,
    service: &str,
    name: Option<String>,
    password: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let material = bundle::parse_material(&bytes, password.as_deref())
        .with_context(|| format!("parsing {}", file.display()))?;
    let key_pem = material
        .key_pem
        .clone()
        .ok_or_else(|| anyhow!("the bundle has no private key — a server certificate needs one"))?;
    let name = name.unwrap_or_else(|| name_from_cn(&material.leaf.subject, "server"));

    println!("Server certificate:");
    print_cert(&material.leaf);
    println!();

    let mut plan = Plan::new();
    plan.push(PrivilegedOp::DeployServer {
        name: name.clone(),
        service: service.to_string(),
        cert_pem: material.leaf_pem.clone(),
        key_pem,
        chain_pem: material.chain_pem.clone(),
    });

    for op in &plan.ops {
        println!("  - {}", op.label());
    }
    if dry_run {
        println!("\n(dry run — nothing changed)");
        return Ok(());
    }
    apply_privileged(&plan)
}

fn cmd_nss_list(json: bool) -> Result<()> {
    let dbs = lcm_core::nss::databases();
    if json {
        println!("{}", serde_json::to_string_pretty(&dbs)?);
        return Ok(());
    }
    if dbs.is_empty() {
        println!("No browser NSS databases found.");
        println!("Looked in: ~/.pki/nssdb, Firefox/Chromium profiles (incl. Snap & Flatpak).");
        println!("Tip: launch the browser once so it creates its profile, then re-run with --nss.");
        return Ok(());
    }
    for d in &dbs {
        println!("{:<24} {}", d.label, d.dir);
    }
    Ok(())
}

fn cmd_nss_sync() -> Result<()> {
    let results = lcm_core::nss::sync();
    if results.is_empty() {
        println!("Nothing to sync (no managed certs, or no browser NSS databases found).");
        return Ok(());
    }
    print_results(&results);
    Ok(())
}

fn cmd_server_list(json: bool) -> Result<()> {
    let deps = service::list_deployments().map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&deps)?);
        return Ok(());
    }
    if deps.is_empty() {
        println!("No server certificates deployed.");
        return Ok(());
    }
    for d in &deps {
        let subject = d.cert.as_ref().map(|c| c.subject.as_str()).unwrap_or("<unreadable>");
        println!("{:<20} {:<8} {}", d.name, d.service, subject);
    }
    Ok(())
}

fn cmd_server_remove(name: &str, service: &str, dry_run: bool) -> Result<()> {
    let mut plan = Plan::new();
    plan.push(PrivilegedOp::RemoveDeployment {
        name: name.to_string(),
        service: service.to_string(),
    });
    for op in &plan.ops {
        println!("  - {}", op.label());
    }
    if dry_run {
        println!("\n(dry run — nothing changed)");
        return Ok(());
    }
    apply_privileged(&plan)
}

fn cmd_info(file: &PathBuf) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let certs = cert::parse(&bytes).with_context(|| format!("parsing {}", file.display()))?;
    for (i, c) in certs.iter().enumerate() {
        if certs.len() > 1 {
            println!("# certificate {} of {}", i + 1, certs.len());
        }
        print_cert(c);
        println!();
    }
    Ok(())
}

fn cmd_ca_install(
    file: &PathBuf,
    name: Option<String>,
    dry_run: bool,
    force: bool,
    nss: bool,
) -> Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let info = cert::parse_one(&bytes).with_context(|| format!("parsing {}", file.display()))?;

    if !info.is_ca && !force {
        bail!(
            "{} is not marked as a CA certificate; pass --force to install anyway",
            file.display()
        );
    }

    // Derive a default name from the file stem when one isn't given.
    let raw_name = name.unwrap_or_else(|| {
        file.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "anchor".to_string())
    });
    let stem = sanitize_name(&raw_name).context("invalid --name")?;

    println!("Certificate to install:");
    print_cert(&info);
    println!();

    let mut plan = Plan::new();
    plan.push(PrivilegedOp::InstallAnchor {
        name: stem.clone(),
        cert_der_b64: base64::engine::general_purpose::STANDARD.encode(&info.der),
    });
    plan.push(PrivilegedOp::ApplyTrust);

    preview_plan(&plan)?;

    if dry_run {
        println!("\n(dry run — nothing changed)");
        if nss {
            println!("would also import into NSS: {}", nss_db_summary());
        }
        return Ok(());
    }
    apply_privileged(&plan)?;

    if nss {
        let pem = cert::to_pem(&info);
        nss_report(lcm_core::nss::import_ca_all(pem.as_bytes(), &format!("LCM {stem}")));
    }
    Ok(())
}

/// Print a short list of the NSS databases that would be targeted.
fn nss_db_summary() -> String {
    let dbs = lcm_core::nss::databases();
    if dbs.is_empty() {
        "none found".to_string()
    } else {
        dbs.iter().map(|d| d.label.clone()).collect::<Vec<_>>().join(", ")
    }
}

/// Print per-database NSS import results.
fn nss_report(results: Vec<(String, lcm_core::Result<String>)>) {
    if results.is_empty() {
        println!("NSS: no browser databases found (~/.pki/nssdb, Firefox profiles).");
        return;
    }
    for (label, r) in results {
        match r {
            Ok(_) => println!("✓ NSS {label} — imported"),
            Err(e) => println!("✗ NSS {label} — {e}"),
        }
    }
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>())
    } else {
        s.to_string()
    }
}

fn cmd_ca_audit(json: bool, expiring: Option<i64>) -> Result<()> {
    let mut certs = trust::list_system_trust().context("reading system trust bundle")?;
    if let Some(days) = expiring {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        certs.retain(|c| (c.not_after_ts - now) / 86_400 <= days);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&certs)?);
        return Ok(());
    }
    match trust::bundle_path() {
        Some(p) => println!("System trust bundle: {} — {} CA(s)\n", p.display(), certs.len()),
        None => {
            println!("No system trust bundle found.");
            return Ok(());
        }
    }
    for c in &certs {
        println!("{:<52} expires {}", short(&c.subject, 52), c.not_after);
    }
    Ok(())
}

fn cmd_ca_list(json: bool) -> Result<()> {
    let anchors = exec::list_installed().context("listing installed anchors")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&anchors)?);
        return Ok(());
    }
    if anchors.is_empty() {
        println!("No LCM-managed anchors installed.");
        return Ok(());
    }
    for a in &anchors {
        match &a.cert {
            Some(c) => println!("{:<24} {}  (expires {})", a.name, c.subject, c.not_after),
            None => println!("{:<24} <unparseable: {}>", a.name, a.path),
        }
    }
    Ok(())
}

fn cmd_ca_remove(name: &str, dry_run: bool) -> Result<()> {
    let stem = sanitize_name(name).context("invalid --name")?;
    let mut plan = Plan::new();
    plan.push(PrivilegedOp::RemoveAnchor { name: stem });
    plan.push(PrivilegedOp::ApplyTrust);

    preview_plan(&plan)?;

    if dry_run {
        println!("\n(dry run — nothing changed)");
        return Ok(());
    }
    apply_privileged(&plan)
}

/// The privileged helper: read a plan from stdin, execute it, report results.
/// With `--json` it prints a JSON array of results (consumed by the GUI);
/// otherwise it prints human-readable lines (for the CLI).
fn cmd_helper(json: bool) -> Result<()> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("reading plan from stdin")?;
    let plan = Plan::from_json(&buf).context("parsing plan JSON")?;
    let results = exec::execute_plan(&plan).context("executing plan")?;
    if json {
        println!("{}", serde_json::to_string(&results)?);
    } else {
        print_results(&results);
    }
    if results.iter().any(|r| !r.ok) {
        std::process::exit(1);
    }
    Ok(())
}

/// Show the distro/target context and the operations that will run.
fn preview_plan(plan: &Plan) -> Result<()> {
    let family = distro::detect();
    let backend = backend::for_family(family)
        .map_err(|e| anyhow!("{e}"))
        .context("no trust-store backend for this system")?;
    println!(
        "Plan (distro family: {:?}, anchor dir: {}, apply: {}):",
        family,
        backend.anchor_dir().display(),
        backend.apply_command_str()
    );
    for op in &plan.ops {
        println!("  - {}", op.label());
    }
    Ok(())
}

/// Execute a privileged plan: inline when root, else via pkexec.
fn apply_privileged(plan: &Plan) -> Result<()> {
    if plan.is_empty() {
        println!("Nothing to do.");
        return Ok(());
    }
    if is_root() {
        let results = exec::execute_plan(plan).context("executing plan")?;
        print_results(&results);
        if results.iter().any(|r| !r.ok) {
            bail!("one or more operations failed");
        }
        Ok(())
    } else {
        run_via_pkexec(plan)
    }
}

fn run_via_pkexec(plan: &Plan) -> Result<()> {
    let exe = std::env::current_exe().context("locating own executable")?;
    let json = plan.to_json()?;

    println!("\nRequesting privileges via polkit…");
    let mut child = Command::new("pkexec")
        .arg("--disable-internal-agent")
        .arg(&exe)
        .arg("helper")
        .stdin(Stdio::piped())
        .spawn()
        .context("launching pkexec (is polkit installed?)")?;

    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(json.as_bytes())
        .context("sending plan to helper")?;

    let status = child.wait().context("waiting for helper")?;
    if !status.success() {
        bail!("privileged helper reported failure");
    }
    Ok(())
}

fn is_root() -> bool {
    lcm_core::util::is_elevated()
}

fn print_cert(c: &CertInfo) {
    println!("  Subject:     {}", c.subject);
    println!("  Issuer:      {}", c.issuer);
    println!("  Serial:      {}", c.serial);
    println!("  Valid from:  {}", c.not_before);
    println!("  Valid until: {}", c.not_after);
    println!("  SHA-256:     {}", c.fingerprint_sha256);
    println!("  Is CA:       {}", if c.is_ca { "yes" } else { "no" });
}

fn print_results(results: &[OpResult]) {
    for r in results {
        let mark = if r.ok { "✓" } else { "✗" };
        println!("{mark} {} — {}", r.op, r.message);
    }
}
