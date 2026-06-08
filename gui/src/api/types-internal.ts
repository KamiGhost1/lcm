import type {
  CertInfo, ClientIdentity, InstalledAnchor, Material, NssDb,
  OpResult, ServerDeployment, ServiceTarget, SystemInfo,
} from '../types';

/** The surface the UI talks to. Backed by Tauri commands in the app, or by
 *  mock data when the page is opened in a plain browser for preview. */
export interface LcmApi {
  systemInfo(): Promise<SystemInfo>;

  // CA anchors (system trust store)
  listAnchors(): Promise<InstalledAnchor[]>;
  /** Parse a picked file into its certificate(s) for preview before install. */
  parseCert(bytesB64: string): Promise<CertInfo[]>;
  /** `nss` also imports the CA into browser NSS databases. */
  installCa(name: string, bytesB64: string, nss: boolean): Promise<OpResult[]>;
  removeCa(name: string): Promise<OpResult[]>;
  /** Read-only: every CA the system trusts (not just LCM-managed). */
  listSystemTrust(): Promise<CertInfo[]>;
  /** Discovered browser NSS databases (Firefox/Chrome/Zen/…). */
  listNss(): Promise<NssDb[]>;
  /** Re-import all managed CAs + identities into every discovered browser. */
  syncNss(): Promise<OpResult[]>;

  // Client identities (managed user store)
  listIdentities(): Promise<ClientIdentity[]>;
  /** Parse picked material (leaf + chain + optional key) for preview.
   *  `password` decrypts PKCS#12 (.p12/.pfx) input; ignored for PEM. */
  parseMaterial(bytesB64: string, password?: string): Promise<Material>;
  importIdentity(name: string, bytesB64: string, password?: string, nss?: boolean): Promise<OpResult[]>;
  removeIdentity(name: string): Promise<void>;

  // Server certificates (service deployment)
  listServices(): Promise<ServiceTarget[]>;
  listDeployments(): Promise<ServerDeployment[]>;
  deployServer(name: string, service: string, bytesB64: string, password?: string): Promise<OpResult[]>;
  removeDeployment(name: string, service: string): Promise<OpResult[]>;
}
