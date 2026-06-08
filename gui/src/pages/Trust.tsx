import { useState } from 'react';
import { api } from '../api/client';
import { useAsync } from '../hooks';
import {
  Loading, EmptyState, Modal, Field, DropZone, CertSheet, ResultList, SearchInput,
  ExpiryBadge, Badge, fmtDate, short,
} from '../components/ui';
import { useToast } from '../components/Toast';
import { IconShield, IconPlus, IconTrash } from '../components/icons';
import type { CertInfo, InstalledAnchor, OpResult, SystemInfo } from '../types';

export default function Trust() {
  const { data, loading, reload } = useAsync(() => api.listAnchors());
  const { data: sys } = useAsync(() => api.systemInfo());
  const toast = useToast();
  const [adding, setAdding] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<InstalledAnchor | null>(null);
  const [detail, setDetail] = useState<InstalledAnchor | null>(null);
  const [query, setQuery] = useState('');

  const items = (data ?? []).filter((a) => {
    const q = query.toLowerCase();
    return !q || a.name.toLowerCase().includes(q)
      || (a.cert?.subject.toLowerCase().includes(q) ?? false)
      || (a.cert?.fingerprint_sha256.toLowerCase().includes(q) ?? false);
  });

  return (
    <>
      <div className="row between" style={{ marginBottom: 18 }}>
        <div className="row" style={{ gap: 12 }}>
          <span className="dim">{data?.length ?? 0} CA anchor{(data?.length ?? 0) === 1 ? '' : 's'}</span>
          {(data?.length ?? 0) > 0 && <SearchInput value={query} onChange={setQuery} />}
        </div>
        <button className="btn btn-primary" onClick={() => setAdding(true)}>
          <IconPlus className="btn-ico" /> Add CA
        </button>
      </div>

      {loading ? (
        <div className="panel"><Loading /></div>
      ) : !data || data.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<IconShield size={34} />}
            title="No CA anchors installed"
            hint="Add a corporate or self-signed root CA so this machine trusts certificates it signs."
          />
        </div>
      ) : items.length === 0 ? (
        <div className="panel"><EmptyState icon={<IconShield size={34} />} title="No matches" hint={`Nothing matches “${query}”.`} /></div>
      ) : (
        <div className="grid grid-3">
          {items.map((a) => (
            <div key={a.name} className="card">
              <div className="row between">
                <span className="name mono">{a.name}</span>
                {a.cert ? <ExpiryBadge ts={a.cert.not_after_ts} /> : <Badge kind="muted">unreadable</Badge>}
              </div>
              {a.cert && (
                <div className="section-gap" style={{ marginTop: 14, display: 'grid', gap: 9 }}>
                  <div className="dim" style={{ fontSize: 12.5, lineHeight: 1.4 }}>{a.cert.subject}</div>
                  <Row k="Expires" v={fmtDate(a.cert.not_after_ts)} mono />
                  <Row k="SHA-256" v={short(a.cert.fingerprint_sha256, 23)} mono />
                </div>
              )}
              <div className="row" style={{ marginTop: 16, gap: 8 }}>
                {a.cert && <button className="btn btn-sm btn-ghost" onClick={() => setDetail(a)}>Details</button>}
                <span className="spacer" />
                <button className="btn btn-sm btn-danger" onClick={() => setRemoveTarget(a)}>
                  <IconTrash className="btn-ico" /> Remove
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {adding && sys && (
        <AddCaModal sys={sys} onClose={() => setAdding(false)} onDone={() => { setAdding(false); reload(); toast('CA installed in trust store', 'success'); }} />
      )}
      {removeTarget && sys && (
        <RemoveModal anchor={removeTarget} sys={sys} onClose={() => setRemoveTarget(null)} onDone={() => { setRemoveTarget(null); reload(); toast('CA removed', 'success'); }} />
      )}
      {detail && detail.cert && (
        <Modal wide title={<span>CA · <span className="mono">{detail.name}</span></span>} onClose={() => setDetail(null)}>
          <div className="modal-body">
            <CertSheet cert={detail.cert} />
            <p className="hint mono" style={{ fontSize: 11.5 }}>{detail.path}</p>
          </div>
        </Modal>
      )}
    </>
  );
}

function Row({ k, v, mono }: { k: string; v: string; mono?: boolean }) {
  return (
    <div className="row between">
      <span className="dim" style={{ fontSize: 12.5 }}>{k}</span>
      <span className={mono ? 'mono' : ''} style={{ fontSize: 12 }}>{v}</span>
    </div>
  );
}

function PlanPreview({ sys, ops }: { sys: SystemInfo; ops: string[] }) {
  return (
    <div className="plan">
      <div className="ctx">{sys.distro_family} · {sys.anchor_dir} · {sys.apply_command}</div>
      {ops.map((o, i) => <div key={i} className="op">{o}</div>)}
    </div>
  );
}

function defaultName(filename: string): string {
  return filename.replace(/\.(crt|pem|cer|der)$/i, '').replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '') || 'anchor';
}

function AddCaModal({ sys, onClose, onDone }: { sys: SystemInfo; onClose: () => void; onDone: () => void }) {
  const [bytesB64, setBytesB64] = useState<string | null>(null);
  const [preview, setPreview] = useState<CertInfo | null>(null);
  const [name, setName] = useState('');
  const [nss, setNss] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<OpResult[] | null>(null);

  async function onFile(filename: string, b64: string) {
    setError(null);
    setBytesB64(b64);
    setName(defaultName(filename));
    try {
      const certs = await api.parseCert(b64);
      if (!certs.length) { setError('No certificate found in that file.'); return; }
      setPreview(certs[0]);
    } catch (e) {
      setError(String(e));
    }
  }

  async function install() {
    if (!bytesB64 || !name) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api.installCa(name, bytesB64, nss);
      setResults(r);
      if (r.every((x) => x.ok)) setTimeout(onDone, 900);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const footer = results ? (
    <button className="btn btn-primary" onClick={onDone}>Done</button>
  ) : (
    <>
      <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
      <button className="btn btn-primary" onClick={install} disabled={!preview || !name || busy}>
        {busy ? 'Installing…' : sys.is_root ? 'Install' : 'Install · authorize'}
      </button>
    </>
  );

  return (
    <Modal wide title="Add CA to system trust store" onClose={onClose} footer={footer}>
      <div className="modal-body">
        {!preview && !results && <DropZone onFile={onFile} />}
        {error && <div className="login-error">{error}</div>}

        {preview && !results && (
          <>
            {!preview.is_ca && (
              <div className="login-error" style={{ color: 'var(--amber)', background: 'rgba(224,175,104,0.08)', borderColor: 'rgba(224,175,104,0.2)' }}>
                This certificate is not marked as a CA — it usually shouldn’t be installed as a trust anchor.
              </div>
            )}
            <CertSheet cert={preview} />
            <Field label="Anchor name">
              <input className="input mono" value={name} onChange={(e) => setName(e.target.value)} placeholder="corp-root" autoFocus />
            </Field>
            <label className="row" style={{ gap: 8, cursor: 'pointer', fontSize: 13 }}>
              <input type="checkbox" checked={nss} onChange={(e) => setNss(e.target.checked)} />
              Also trust in browsers (NSS — Chrome/Firefox)
            </label>
            <PlanPreview sys={sys} ops={[`install anchor "${name}"`, 'apply trust store']} />
            {!sys.is_root && <p className="hint">polkit will prompt for authorization when you install.</p>}
          </>
        )}

        {results && <ResultList results={results} />}
      </div>
    </Modal>
  );
}

function RemoveModal({ anchor, sys, onClose, onDone }: { anchor: InstalledAnchor; sys: SystemInfo; onClose: () => void; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<OpResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function remove() {
    setBusy(true);
    try {
      const r = await api.removeCa(anchor.name);
      setResults(r);
      if (r.every((x) => x.ok)) setTimeout(onDone, 800);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const footer = results ? (
    <button className="btn btn-primary" onClick={onDone}>Done</button>
  ) : (
    <>
      <button className="btn btn-ghost" onClick={onClose}>Cancel</button>
      <button className="btn btn-danger" onClick={remove} disabled={busy}>{busy ? 'Removing…' : 'Remove'}</button>
    </>
  );

  return (
    <Modal title={<span>Remove <span className="mono">{anchor.name}</span></span>} onClose={onClose} footer={footer}>
      <div className="modal-body">
        {!results && (
          <>
            <p className="hint">
              This removes the anchor from the system trust store. Certificates signed by it
              will no longer be trusted on this machine.
            </p>
            <PlanPreview sys={sys} ops={[`remove anchor "${anchor.name}"`, 'apply trust store']} />
            {!sys.is_root && <p className="hint">polkit will prompt for authorization.</p>}
          </>
        )}
        {error && <div className="login-error">{error}</div>}
        {results && <ResultList results={results} />}
      </div>
    </Modal>
  );
}
