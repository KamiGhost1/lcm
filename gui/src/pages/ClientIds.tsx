import { useState } from 'react';
import { api } from '../api/client';
import { useAsync } from '../hooks';
import {
  Loading, EmptyState, Modal, Field, DropZone, CertSheet, ResultList, SearchInput,
  ExpiryBadge, Badge, fmtDate, short, looksPem,
} from '../components/ui';
import { useToast } from '../components/Toast';
import { IconLicenses, IconPlus, IconTrash, IconKey } from '../components/icons';
import type { ClientIdentity, Material, OpResult } from '../types';

export default function ClientIds() {
  const { data, loading, reload } = useAsync(() => api.listIdentities());
  const toast = useToast();
  const [adding, setAdding] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ClientIdentity | null>(null);
  const [detail, setDetail] = useState<ClientIdentity | null>(null);
  const [query, setQuery] = useState('');

  const items = (data ?? []).filter((id) => {
    const q = query.toLowerCase();
    return !q || id.name.toLowerCase().includes(q) || id.cert.subject.toLowerCase().includes(q);
  });

  async function doRemove() {
    if (!removeTarget) return;
    await api.removeIdentity(removeTarget.name);
    setRemoveTarget(null);
    reload();
    toast('Identity removed', 'success');
  }

  return (
    <>
      <div className="row between" style={{ marginBottom: 18 }}>
        <div className="row" style={{ gap: 12 }}>
          <span className="dim">{data?.length ?? 0} client identit{(data?.length ?? 0) === 1 ? 'y' : 'ies'}</span>
          {(data?.length ?? 0) > 0 && <SearchInput value={query} onChange={setQuery} />}
        </div>
        <button className="btn btn-primary" onClick={() => setAdding(true)}>
          <IconPlus className="btn-ico" /> Import identity
        </button>
      </div>

      {loading ? (
        <div className="panel"><Loading /></div>
      ) : !data || data.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<IconLicenses size={34} />}
            title="No client identities"
            hint="Import a certificate + private key (PEM or .p12) to use for mTLS or VPN authentication."
          />
        </div>
      ) : items.length === 0 ? (
        <div className="panel"><EmptyState icon={<IconLicenses size={34} />} title="No matches" hint={`Nothing matches “${query}”.`} /></div>
      ) : (
        <div className="grid grid-3">
          {items.map((id) => (
            <div key={id.name} className="card">
              <div className="row between">
                <span className="name mono">{id.name}</span>
                <ExpiryBadge ts={id.cert.not_after_ts} />
              </div>
              <div className="section-gap" style={{ marginTop: 14, display: 'grid', gap: 9 }}>
                <div className="dim" style={{ fontSize: 12.5, lineHeight: 1.4 }}>{id.cert.subject}</div>
                <div className="row between">
                  <span className="dim" style={{ fontSize: 12.5 }}>Private key</span>
                  {id.has_key ? <Badge kind="green">present</Badge> : <Badge kind="amber">missing</Badge>}
                </div>
                <div className="row between">
                  <span className="dim" style={{ fontSize: 12.5 }}>Expires</span>
                  <span className="mono" style={{ fontSize: 12 }}>{fmtDate(id.cert.not_after_ts)}</span>
                </div>
                <div className="row between">
                  <span className="dim" style={{ fontSize: 12.5 }}>SHA-256</span>
                  <span className="mono" style={{ fontSize: 12 }}>{short(id.cert.fingerprint_sha256, 23)}</span>
                </div>
              </div>
              <div className="row" style={{ marginTop: 16, gap: 8 }}>
                <button className="btn btn-sm btn-ghost" onClick={() => setDetail(id)}>Details</button>
                <span className="spacer" />
                <button className="btn btn-sm btn-danger" onClick={() => setRemoveTarget(id)}>
                  <IconTrash className="btn-ico" /> Remove
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {adding && <ImportModal onClose={() => setAdding(false)} onDone={() => { setAdding(false); reload(); toast('Identity imported', 'success'); }} />}
      {detail && (
        <Modal wide title={<span>Identity · <span className="mono">{detail.name}</span></span>} onClose={() => setDetail(null)}>
          <div className="modal-body">
            <div className="row wrap">
              {detail.has_key ? <Badge kind="green"><IconKey size={12} /> private key present</Badge> : <Badge kind="amber"><IconKey size={12} /> no private key</Badge>}
            </div>
            <CertSheet cert={detail.cert} />
            <p className="hint mono" style={{ fontSize: 11.5 }}>{detail.path}</p>
          </div>
        </Modal>
      )}
      {removeTarget && (
        <Modal
          title={<span>Remove <span className="mono">{removeTarget.name}</span></span>}
          onClose={() => setRemoveTarget(null)}
          footer={<>
            <button className="btn btn-ghost" onClick={() => setRemoveTarget(null)}>Cancel</button>
            <button className="btn btn-danger" onClick={doRemove}>Remove</button>
          </>}
        >
          <div className="modal-body">
            <p className="hint">Deletes the identity (certificate and private key) from the managed store. This is a user-level action — no root needed.</p>
          </div>
        </Modal>
      )}
    </>
  );
}

function defaultName(subject: string): string {
  const cn = /CN=([^,]+)/.exec(subject)?.[1] ?? 'identity';
  return cn.replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || 'identity';
}

function ImportModal({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [bytesB64, setBytesB64] = useState<string | null>(null);
  const [material, setMaterial] = useState<Material | null>(null);
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [nss, setNss] = useState(true);
  const [needsPassword, setNeedsPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<OpResult[] | null>(null);

  async function parse(b64: string, pw: string) {
    try {
      const m = await api.parseMaterial(b64, pw);
      setMaterial(m);
      setName(defaultName(m.leaf.subject));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function onFile(_filename: string, b64: string) {
    setError(null);
    setBytesB64(b64);
    if (looksPem(b64)) {
      parse(b64, '');
    } else {
      setNeedsPassword(true); // PKCS#12 — ask for the password first
    }
  }

  async function importIt() {
    if (!bytesB64 || !name) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api.importIdentity(name, bytesB64, password, nss);
      setResults(r);
      if (r.every((x) => x.ok)) setTimeout(onDone, 1000);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      wide
      title="Import client identity"
      onClose={onClose}
      footer={results ? (
        <button className="btn btn-primary" onClick={onDone}>Done</button>
      ) : (
        <>
          <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" onClick={importIt} disabled={!material || !name || busy}>
            {busy ? 'Importing…' : 'Import'}
          </button>
        </>
      )}
    >
      <div className="modal-body">
        {!material && !needsPassword && !results && <DropZone onFile={onFile} accept=".pem,.crt,.cer,.key,.p12,.pfx,.skb" />}
        {needsPassword && !material && (
          <>
            <p className="hint">Bundle detected (.p12 / Secutor .skb) — enter its password (leave blank if none) and unlock.</p>
            <Field label="PKCS#12 password">
              <input
                className="input"
                type="password"
                value={password}
                autoFocus
                onChange={(e) => setPassword(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && bytesB64 && parse(bytesB64, password)}
              />
            </Field>
            <button className="btn btn-primary" style={{ alignSelf: 'flex-start' }} onClick={() => bytesB64 && parse(bytesB64, password)}>Unlock</button>
          </>
        )}
        {error && <div className="login-error">{error}</div>}

        {results && (
          <>
            <ResultList results={results} />
            {results.some((r) => /not found|libnss3-tools|nss-tools/.test(r.message)) && (
              <p className="hint">NSS tools missing — <span className="mono">sudo apt install libnss3-tools</span>, then re-import or use Sync.</p>
            )}
          </>
        )}

        {material && !results && (
          <>
            <div className="row wrap">
              {material.has_key
                ? <Badge kind="green"><IconKey size={12} /> private key present</Badge>
                : <Badge kind="amber"><IconKey size={12} /> no private key — needed for mTLS</Badge>}
              {material.chain.length > 0 && <Badge kind="blue">+{material.chain.length} chain cert{material.chain.length === 1 ? '' : 's'}</Badge>}
            </div>
            <CertSheet cert={material.leaf} />
            <Field label="Identity name">
              <input className="input mono" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
            </Field>
            <label className="row" style={{ gap: 8, cursor: 'pointer', fontSize: 13 }}>
              <input type="checkbox" checked={nss} onChange={(e) => setNss(e.target.checked)} />
              Also import into browsers (NSS) for mTLS
            </label>
            <p className="hint">Stored in the managed user store (the key with <span className="mono">0600</span> permissions). No root required.</p>
          </>
        )}
      </div>
    </Modal>
  );
}
