import type {
  CertInfo, ClientIdentity, InstalledAnchor, Material,
  OpResult, ServerDeployment, ServiceTarget, SystemInfo,
} from '../types';
import type { LcmApi } from './types-internal';

// Demo data so the UI can be previewed in a plain browser (no Tauri backend).
const now = Math.floor(Date.now() / 1000);
const DAY = 86400;

function cert(partial: Partial<CertInfo> & { subject: string; not_after_ts: number }): CertInfo {
  return {
    issuer: 'CN=ACME Corp Issuing CA, O=ACME Corp',
    serial: '4a:3f:e1:09:7c:22:bb:01:de:ad:be:ef:12:34:56:78',
    not_before: '—',
    not_after: '—',
    not_before_ts: now - 200 * DAY,
    fingerprint_sha256: '3C:1A:9F:0B:77:2E:44:8D:6A:11:EC:16:DE:07:BF:56:B7:4B:3C:10:1E:CB:31:9A:15:78:00:28:00:6A:94:EE',
    is_ca: false,
    ...partial,
  };
}

let anchors: InstalledAnchor[] = [
  { name: 'corp-root', path: '/usr/local/share/ca-certificates/lcm-corp-root.crt', cert: cert({ subject: 'CN=ACME Corp Root CA, O=ACME Corp, C=US', is_ca: true, not_after_ts: now + 900 * DAY }) },
  { name: 'dev-mkcert', path: '/usr/local/share/ca-certificates/lcm-dev-mkcert.crt', cert: cert({ subject: 'CN=mkcert dev CA, O=mkcert', is_ca: true, not_after_ts: now + 18 * DAY }) },
  { name: 'old-staging', path: '/usr/local/share/ca-certificates/lcm-old-staging.crt', cert: cert({ subject: 'CN=Staging Root (2023), O=ACME Corp', is_ca: true, not_after_ts: now - 5 * DAY }) },
];

let identities: ClientIdentity[] = [
  { name: 'alice-mtls', path: '~/.local/share/lcm/identities/alice-mtls', has_key: true, cert: cert({ subject: 'CN=alice@acme.corp, OU=Engineering', not_after_ts: now + 300 * DAY }) },
  { name: 'vpn-laptop', path: '~/.local/share/lcm/identities/vpn-laptop', has_key: true, cert: cert({ subject: 'CN=laptop-01.vpn.acme.corp', not_after_ts: now + 12 * DAY }) },
];

let deployments: ServerDeployment[] = [
  { name: 'example-com', service: 'nginx', paths: ['/etc/nginx/certs/lcm-example-com/fullchain.crt', '/etc/nginx/certs/lcm-example-com/privkey.key'], cert: cert({ subject: 'CN=example.com', not_after_ts: now + 75 * DAY }) },
];

const services: ServiceTarget[] = [
  { id: 'nginx', label: 'nginx', available: true, cert_dir: '/etc/nginx/certs', reload: 'systemctl reload nginx' },
  { id: 'apache', label: 'Apache', available: false, cert_dir: '/etc/ssl/lcm/apache', reload: 'systemctl reload apache2' },
  { id: 'haproxy', label: 'HAProxy', available: false, cert_dir: '/etc/haproxy/certs', reload: 'systemctl reload haproxy' },
];

const wait = (ms = 280) => new Promise((r) => setTimeout(r, ms));

export const mockApi: LcmApi = {
  async systemInfo(): Promise<SystemInfo> {
    await wait(120);
    return { distro_family: 'Debian', anchor_dir: '/usr/local/share/ca-certificates', apply_command: 'update-ca-certificates', is_root: false, supported: true };
  },

  async listAnchors() { await wait(); return [...anchors]; },
  async parseCert() { await wait(150); return [cert({ subject: 'CN=Imported Root CA, O=Example Ltd', is_ca: true, not_after_ts: now + 1200 * DAY })]; },
  async installCa(name): Promise<OpResult[]> {
    await wait(500);
    anchors = anchors.filter((a) => a.name !== name);
    anchors.push({ name, path: `/usr/local/share/ca-certificates/lcm-${name}.crt`, cert: cert({ subject: 'CN=Imported Root CA, O=Example Ltd', is_ca: true, not_after_ts: now + 1200 * DAY }) });
    return [
      { op: `install anchor "${name}"`, ok: true, message: `wrote /usr/local/share/ca-certificates/lcm-${name}.crt` },
      { op: 'apply trust store', ok: true, message: 'update-ca-certificates succeeded' },
    ];
  },
  async removeCa(name): Promise<OpResult[]> {
    await wait(450);
    anchors = anchors.filter((a) => a.name !== name);
    return [{ op: `remove anchor "${name}"`, ok: true, message: 'removed' }, { op: 'apply trust store', ok: true, message: 'update-ca-certificates succeeded' }];
  },
  async listSystemTrust() {
    await wait(200);
    const roots = [
      'CN=ISRG Root X1, O=Internet Security Research Group, C=US',
      'CN=DigiCert Global Root G2, OU=www.digicert.com, O=DigiCert Inc, C=US',
      'CN=GlobalSign Root R46, O=GlobalSign nv-sa, C=BE',
      'CN=USERTrust RSA Certification Authority, O=The USERTRUST Network, C=US',
      'CN=Amazon Root CA 1, O=Amazon, C=US',
      'CN=Sectigo Public Server Authentication Root R46, O=Sectigo Limited, C=GB',
      'CN=ACME Corp Root CA, O=ACME Corp, C=US',
    ];
    return roots.map((subject, i) => cert({ subject, is_ca: true, not_after_ts: now + (1000 + i * 400) * DAY }));
  },
  async listNss() {
    await wait(150);
    return [
      { label: 'Shared (~/.pki/nssdb)', dir: '/home/you/.pki/nssdb' },
      { label: 'Firefox (snap): abc.default', dir: '/home/you/snap/firefox/common/.mozilla/firefox/abc.default' },
      { label: 'Zen: p1.Default', dir: '/home/you/.zen/p1.Default' },
    ];
  },
  async syncNss(): Promise<OpResult[]> {
    await wait(700);
    return [
      { op: 'CA corp-root → Shared (~/.pki/nssdb)', ok: true, message: 'imported' },
      { op: 'CA corp-root → Firefox (snap): abc.default', ok: true, message: 'imported' },
      { op: 'CA corp-root → Zen: p1.Default', ok: true, message: 'imported' },
      { op: 'ID alice-mtls → Shared (~/.pki/nssdb)', ok: true, message: 'imported' },
    ];
  },

  async listIdentities() { await wait(); return [...identities]; },
  async parseMaterial(): Promise<Material> {
    await wait(150);
    return { leaf: cert({ subject: 'CN=bob@acme.corp, OU=Sales', not_after_ts: now + 365 * DAY }), chain: [cert({ subject: 'CN=ACME Corp Issuing CA', is_ca: true, not_after_ts: now + 1500 * DAY })], has_key: true };
  },
  async importIdentity(name): Promise<OpResult[]> {
    await wait(450);
    identities = identities.filter((i) => i.name !== name);
    identities.push({ name, path: `~/.local/share/lcm/identities/${name}`, has_key: true, cert: cert({ subject: 'CN=bob@acme.corp, OU=Sales', not_after_ts: now + 365 * DAY }) });
    return [
      { op: `store identity "${name}"`, ok: true, message: `~/.local/share/lcm/identities/${name}` },
      { op: 'NSS: Shared (~/.pki/nssdb)', ok: true, message: 'imported' },
      { op: 'NSS: Firefox (snap): abc.default', ok: true, message: 'imported' },
    ];
  },
  async removeIdentity(name) { await wait(300); identities = identities.filter((i) => i.name !== name); },

  async listServices() { await wait(100); return services; },
  async listDeployments() { await wait(); return [...deployments]; },
  async deployServer(name, service): Promise<OpResult[]> {
    await wait(550);
    deployments = deployments.filter((d) => !(d.name === name && d.service === service));
    deployments.push({ name, service, paths: [`/etc/${service}/certs/lcm-${name}/fullchain.crt`, `/etc/${service}/certs/lcm-${name}/privkey.key`], cert: cert({ subject: `CN=${name}`, not_after_ts: now + 90 * DAY }) });
    return [{ op: `deploy "${name}" to ${service}`, ok: true, message: `deployed to /etc/${service}/certs/lcm-${name} · ${service} reloaded` }];
  },
  async removeDeployment(name, service): Promise<OpResult[]> {
    await wait(400);
    deployments = deployments.filter((d) => !(d.name === name && d.service === service));
    return [{ op: `remove "${name}" from ${service}`, ok: true, message: 'removed and reloaded' }];
  },
};
