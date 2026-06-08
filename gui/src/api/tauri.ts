import { invoke } from '@tauri-apps/api/core';
import type {
  CertInfo, ClientIdentity, InstalledAnchor, Material, NssDb,
  OpResult, ServerDeployment, ServiceTarget, SystemInfo,
} from '../types';
import type { LcmApi } from './types-internal';

// Thin wrappers over the Rust #[tauri::command]s exposed by lcm-gui (src-tauri).
export const tauriApi: LcmApi = {
  systemInfo: () => invoke<SystemInfo>('system_info'),

  listAnchors: () => invoke<InstalledAnchor[]>('list_anchors'),
  parseCert: (bytesB64) => invoke<CertInfo[]>('parse_cert', { bytesB64 }),
  installCa: (name, bytesB64, nss) => invoke<OpResult[]>('install_ca', { name, bytesB64, nss }),
  removeCa: (name) => invoke<OpResult[]>('remove_ca', { name }),
  listSystemTrust: () => invoke<CertInfo[]>('list_system_trust'),
  listNss: () => invoke<NssDb[]>('nss_databases'),
  syncNss: () => invoke<OpResult[]>('nss_sync'),

  listIdentities: () => invoke<ClientIdentity[]>('list_identities'),
  parseMaterial: (bytesB64, password) => invoke<Material>('parse_material', { bytesB64, password }),
  importIdentity: (name, bytesB64, password, nss) => invoke<OpResult[]>('import_identity', { name, bytesB64, password, nss }),
  removeIdentity: (name) => invoke<void>('remove_identity', { name }),

  listServices: () => invoke<ServiceTarget[]>('list_services'),
  listDeployments: () => invoke<ServerDeployment[]>('list_deployments'),
  deployServer: (name, service, bytesB64, password) => invoke<OpResult[]>('deploy_server', { name, service, bytesB64, password }),
  removeDeployment: (name, service) => invoke<OpResult[]>('remove_deployment', { name, service }),
};
