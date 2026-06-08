import type { LcmApi } from './types-internal';
import { mockApi } from './mock';

// Tauri v2 injects __TAURI_INTERNALS__ into the webview. When it's absent we're
// running in a plain browser (vite preview / screenshot) and serve mock data.
export const IS_TAURI =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

// Resolve the concrete implementation once, before any call runs, so the real
// app never briefly shows mock data while the Tauri SDK is importing.
const ready: Promise<LcmApi> = IS_TAURI
  ? import('./tauri').then((m) => m.tauriApi)
  : Promise.resolve(mockApi);

export const api: LcmApi = {
  systemInfo: () => ready.then((i) => i.systemInfo()),

  listAnchors: () => ready.then((i) => i.listAnchors()),
  parseCert: (b) => ready.then((i) => i.parseCert(b)),
  installCa: (n, b, nss) => ready.then((i) => i.installCa(n, b, nss)),
  removeCa: (n) => ready.then((i) => i.removeCa(n)),
  listSystemTrust: () => ready.then((i) => i.listSystemTrust()),
  listNss: () => ready.then((i) => i.listNss()),
  syncNss: () => ready.then((i) => i.syncNss()),

  listIdentities: () => ready.then((i) => i.listIdentities()),
  parseMaterial: (b, p) => ready.then((i) => i.parseMaterial(b, p)),
  importIdentity: (n, b, p, nss) => ready.then((i) => i.importIdentity(n, b, p, nss)),
  removeIdentity: (n) => ready.then((i) => i.removeIdentity(n)),

  listServices: () => ready.then((i) => i.listServices()),
  listDeployments: () => ready.then((i) => i.listDeployments()),
  deployServer: (n, s, b, p) => ready.then((i) => i.deployServer(n, s, b, p)),
  removeDeployment: (n, s) => ready.then((i) => i.removeDeployment(n, s)),
};
