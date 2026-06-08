import { useState } from 'react';
import { api } from '../api/client';
import { useAsync } from '../hooks';
import {
  Loading, EmptyState, Modal, Field, DropZone, CertSheet, ResultList, SearchInput,
  ExpiryBadge, Badge, fmtDate, looksPem,
} from '../components/ui';
import { useToast } from '../components/Toast';
import { IconServers, IconPlus, IconTrash, IconKey } from '../components/icons';
import type { Material, OpResult, ServerDeployment, ServiceTarget, SystemInfo } from '../types';

export default function ServerCerts() {
  const { data, loading, reload } = useAsync(() => api.listDeployments());
  const { data: sys } = useAsync(() => api.systemInfo());
  const { data: services } = useAsync(() => api.listServices());
  const toast = useToast();
  const [adding, setAdding] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<ServerDeployment | null>(null);
  const [detail, setDetail] = useState<ServerDeployment | null>(null);
  const [query, setQuery] = useState('');

  const items = (data ?? []).filter((d) => {
    const q = query.toLowerCase();
    return !q || d.name.toLowerCase().includes(q) || d.service.toLowerCase().includes(q)
      || (d.cert?.subject.toLowerCase().includes(q) ?? false);
  });

  return (
    <>
      <div className="row between" style={{ marginBottom: 18 }}>
        <div className="row" style={{ gap: 12 }}>
          <span className="dim">{data?.length ?? 0} deployment{(data?.length ?? 0) === 1 ? '' : 's'}</span>
          {(data?.length ?? 0) > 0 && <SearchInput value={query} onChange={setQuery} />}
        </div>
        <button className="btn btn-primary" onClick={() => setAdding(true)}>
          <IconPlus className="btn-ico" /> Deploy certificate
        </button>
      </div>

      {loading ? (
        <div className="panel"><Loading /></div>
      ) : !data || data.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<IconServers size={34} />}
            title="No server certificates deployed"
            hint="Deploy a certificate + key to a service (nginx, Apache, HAProxy) and reload it."
          />
        </div>
      ) : items.length === 0 ? (
        <div className="panel"><EmptyState icon={<IconServers size={34} />} title="No matches" hint={`Nothing matches “${query}”.`} /></div>
      ) : (
        <div className="grid grid-2">
          {items.map((d) => (
            <div key={`${d.service}/${d.name}`} className="card">
              <div className="row between">
                <span className="name mono">{d.name}</span>
                <div className="row" style={{ gap: 8 }}>
                  <Badge kind="magenta">{d.service}</Badge>
                  {d.cert && <ExpiryBadge ts={d.cert.not_after_ts} />}
                </div>
              </div>
              {d.cert && <div className="dim" style={{ fontSize: 12.5, marginTop: 12 }}>{d.cert.subject} · expires {fmtDate(d.cert.not_after_ts)}</div>}
              <div className="plan" style={{ marginTop: 12 }}>
                {d.paths.map((p, i) => <div key={i} className="ctx">{p}</div>)}
              </div>
              <div className="row" style={{ marginTop: 14, gap: 8 }}>
                {d.cert && <button className="btn btn-sm btn-ghost" onClick={() => setDetail(d)}>Details</button>}
                <span className="spacer" />
                <button className="btn btn-sm btn-danger" onClick={() => setRemoveTarget(d)}>
                  <IconTrash className="btn-ico" /> Remove
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {adding && sys && services && (
        <DeployModal sys={sys} services={services} onClose={() => setAdding(false)} onDone={() => { setAdding(false); reload(); toast('Certificate deployed', 'success'); }} />
      )}
      {removeTarget && sys && (
        <RemoveModal deployment={removeTarget} sys={sys} onClose={() => setRemoveTarget(null)} onDone={() => { setRemoveTarget(null); reload(); toast('Deployment removed', 'success'); }} />
      )}
      {detail && detail.cert && (
        <Modal wide title={<span>Deployment · <span className="mono">{detail.name}</span> → {detail.service}</span>} onClose={() => setDetail(null)}>
          <div className="modal-body">
            <CertSheet cert={detail.cert} />
            <div className="plan">{detail.paths.map((p, i) => <div key={i} className="ctx">{p}</div>)}</div>
          </div>
        </Modal>
      )}
    </>
  );
}

function defaultName(subject: string): string {
  const cn = /CN=([^,]+)/.exec(subject)?.[1] ?? 'site';
  return cn.replace(/\*/g, 'wildcard').replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || 'site';
}

function DeployModal({ sys, services, onClose, onDone }: { sys: SystemInfo; services: ServiceTarget[]; onClose: () => void; onDone: () => void }) {
  const [bytesB64, setBytesB64] = useState<string | null>(null);
  const [material, setMaterial] = useState<Material | null>(null);
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [needsPassword, setNeedsPassword] = useState(false);
  const [service, setService] = useState(services.find((s) => s.available)?.id ?? services[0]?.id ?? 'nginx');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<OpResult[] | null>(null);

  const svc = services.find((s) => s.id === service);

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
      setNeedsPassword(true);
    }
  }

  async function deploy() {
    if (!bytesB64 || !name) return;
    if (material && !material.has_key) { setError('This bundle has no private key — a server certificate needs one.'); return; }
    setBusy(true);
    setError(null);
    try {
      const r = await api.deployServer(name, service, bytesB64, password);
      setResults(r);
      if (r.every((x) => x.ok)) setTimeout(onDone, 900);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      wide
      title="Deploy server certificate"
      onClose={onClose}
      footer={results ? (
        <button className="btn btn-primary" onClick={onDone}>Done</button>
      ) : (
        <>
          <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn btn-primary" onClick={deploy} disabled={!material || !name || busy}>
            {busy ? 'Deploying…' : sys.is_root ? 'Deploy' : 'Deploy · authorize'}
          </button>
        </>
      )}
    >
      <div className="modal-body">
        {!material && !results && !needsPassword && <DropZone onFile={onFile} accept=".pem,.crt,.cer,.key,.p12,.pfx,.skb" />}
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

        {material && !results && (
          <>
            <div className="row wrap">
              {material.has_key
                ? <Badge kind="green"><IconKey size={12} /> private key present</Badge>
                : <Badge kind="red"><IconKey size={12} /> no private key</Badge>}
              {material.chain.length > 0 && <Badge kind="blue">+{material.chain.length} chain cert{material.chain.length === 1 ? '' : 's'}</Badge>}
            </div>
            <CertSheet cert={material.leaf} />
            <div className="row" style={{ gap: 12 }}>
              <Field label="Name">
                <input className="input mono" value={name} onChange={(e) => setName(e.target.value)} />
              </Field>
              <Field label="Service">
                <select className="input" value={service} onChange={(e) => setService(e.target.value)}>
                  {services.map((s) => (
                    <option key={s.id} value={s.id}>{s.label}{s.available ? '' : ' (not detected)'}</option>
                  ))}
                </select>
              </Field>
            </div>
            {svc && (
              <div className="plan">
                <div className="ctx">target: {svc.cert_dir}/lcm-{name || '<name>'}/</div>
                <div className="op">write fullchain.crt + privkey.key</div>
                <div className="op">reload: {svc.reload}</div>
              </div>
            )}
            {!sys.is_root && <p className="hint">polkit will prompt for authorization when you deploy.</p>}
          </>
        )}

        {results && <ResultList results={results} />}
      </div>
    </Modal>
  );
}

function RemoveModal({ deployment, sys, onClose, onDone }: { deployment: ServerDeployment; sys: SystemInfo; onClose: () => void; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<OpResult[] | null>(null);

  async function remove() {
    setBusy(true);
    const r = await api.removeDeployment(deployment.name, deployment.service);
    setResults(r);
    if (r.every((x) => x.ok)) setTimeout(onDone, 800);
    setBusy(false);
  }

  return (
    <Modal
      title={<span>Remove <span className="mono">{deployment.name}</span> from {deployment.service}</span>}
      onClose={onClose}
      footer={results ? (
        <button className="btn btn-primary" onClick={onDone}>Done</button>
      ) : (
        <>
          <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
          <button className="btn btn-danger" onClick={remove} disabled={busy}>{busy ? 'Removing…' : 'Remove'}</button>
        </>
      )}
    >
      <div className="modal-body">
        {!results && (
          <>
            <p className="hint">Removes the deployed certificate and key, then reloads {deployment.service}.</p>
            {!sys.is_root && <p className="hint">polkit will prompt for authorization.</p>}
          </>
        )}
        {results && <ResultList results={results} />}
      </div>
    </Modal>
  );
}
