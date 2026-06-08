// Mirrors the JSON shapes emitted by lcm-core (serde, snake_case fields).

export interface CertInfo {
  subject: string;
  issuer: string;
  serial: string;
  not_before: string;
  not_after: string;
  not_before_ts: number;
  not_after_ts: number;
  fingerprint_sha256: string;
  is_ca: boolean;
}

export interface InstalledAnchor {
  name: string;
  path: string;
  cert: CertInfo | null;
}

export interface OpResult {
  op: string;
  ok: boolean;
  message: string;
}

export interface SystemInfo {
  distro_family: string; // "Debian" | "Unsupported"
  anchor_dir: string;
  apply_command: string;
  is_root: boolean;
  supported: boolean;
}

/** A certificate the user picked for import, before it is installed. */
export interface PickedCert {
  filename: string;
  bytes_b64: string;
  certs: CertInfo[];
}

/** Parsed certificate material: a leaf, optional chain, optional private key. */
export interface Material {
  leaf: CertInfo;
  chain: CertInfo[];
  has_key: boolean;
}

/** A client identity in the managed store. */
export interface ClientIdentity {
  name: string;
  cert: CertInfo;
  has_key: boolean;
  path: string;
}

/** A discovered browser NSS database. */
export interface NssDb {
  label: string;
  dir: string;
}

/** A server-certificate deployment target (web / proxy service). */
export interface ServiceTarget {
  id: string;
  label: string;
  available: boolean;
  cert_dir: string;
  reload: string;
}

/** A server certificate LCM has deployed to a service. */
export interface ServerDeployment {
  name: string;
  service: string;
  cert: CertInfo | null;
  paths: string[];
}
