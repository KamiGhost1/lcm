import { useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import { useAsync } from '../hooks';
import { Loading, Badge, ExpiryBadge, Modal, ResultList, fmtDate, daysUntil } from '../components/ui';
import { useToast } from '../components/Toast';
import { IconShield, IconClock, IconChip, IconActivity, IconRefresh } from '../components/icons';
import type { OpResult } from '../types';

type Soon = { name: string; kind: string; ts: number };

export default function Overview() {
  const { data: anchors, loading } = useAsync(() => api.listAnchors());
  const { data: ids } = useAsync(() => api.listIdentities());
  const { data: deps } = useAsync(() => api.listDeployments());
  const { data: sys } = useAsync(() => api.systemInfo());
  const { data: nss, reload: reloadNss } = useAsync(() => api.listNss());
  const toast = useToast();
  const [syncing, setSyncing] = useState(false);
  const [syncResults, setSyncResults] = useState<OpResult[] | null>(null);

  async function syncBrowsers() {
    setSyncing(true);
    try {
      const r = await api.syncNss();
      const ok = r.filter((x) => x.ok).length;
      const fail = r.length - ok;
      if (r.length === 0) toast('Nothing to sync (no managed certs or browsers)', 'info');
      else if (fail === 0) toast(`Synced ${ok} import(s) into browsers`, 'success');
      else {
        toast(`${ok} ok, ${fail} failed — see details`, 'error');
        setSyncResults(r); // surface the per-item errors
      }
      reloadNss();
    } finally {
      setSyncing(false);
    }
  }

  if (loading || !anchors) return <Loading />;

  const withCert = anchors.filter((a) => a.cert);
  const expiring = withCert.filter((a) => { const d = daysUntil(a.cert!.not_after_ts); return d >= 0 && d <= 30; });
  const expired = withCert.filter((a) => daysUntil(a.cert!.not_after_ts) < 0);

  // Aggregate expiry across every object type for the alert banner.
  const all: Soon[] = [
    ...withCert.map((a) => ({ name: a.name, kind: 'CA', ts: a.cert!.not_after_ts })),
    ...(ids ?? []).map((i) => ({ name: i.name, kind: 'identity', ts: i.cert.not_after_ts })),
    ...(deps ?? []).filter((d) => d.cert).map((d) => ({ name: d.name, kind: 'server', ts: d.cert!.not_after_ts })),
  ];
  const allExpired = all.filter((s) => daysUntil(s.ts) < 0);
  const allSoon = all.filter((s) => { const d = daysUntil(s.ts); return d >= 0 && d <= 30; });
  const flagged = [...allExpired, ...allSoon];

  const stats = [
    { label: 'Installed CAs', value: anchors.length, foot: 'in the system trust store', Icon: IconShield, cls: '' },
    { label: 'Expiring soon', value: expiring.length, foot: 'within 30 days', Icon: IconClock, cls: 'stat-amber' },
    { label: 'Expired', value: expired.length, foot: 'should be removed', Icon: IconActivity, cls: expired.length ? '' : 'stat-green' },
  ];

  return (
    <>
      {flagged.length > 0 && (
        <div className={`banner ${allExpired.length ? 'danger' : ''}`}>
          <span className="banner-ico"><IconClock size={18} /></span>
          <span>
            {allExpired.length > 0 && <><b>{allExpired.length}</b> expired</>}
            {allExpired.length > 0 && allSoon.length > 0 && ' · '}
            {allSoon.length > 0 && <><b>{allSoon.length}</b> expiring within 30 days</>}
            {' — '}
            {flagged.slice(0, 4).map((s) => `${s.name} (${s.kind})`).join(', ')}
            {flagged.length > 4 ? '…' : ''}
          </span>
        </div>
      )}

      <div className="grid grid-3">
        {stats.map((s) => (
          <div key={s.label} className={`card stat ${s.cls}`}>
            <div className="stat-label"><s.Icon className="stat-ico" />{s.label}</div>
            <div className="stat-value">{s.value}</div>
            <div className="stat-foot">{s.foot}</div>
          </div>
        ))}
      </div>

      <div className="grid grid-2 section-gap">
        <div className="card">
          <div className="stat-label"><IconChip className="stat-ico" />Detected system</div>
          {sys ? (
            <div className="kv" style={{ marginTop: 14 }}>
              <dt>Distro family</dt><dd>{sys.supported ? sys.distro_family : <Badge kind="red">unsupported</Badge>}</dd>
              <dt>Anchor dir</dt><dd className="mono" style={{ fontSize: 12 }}>{sys.anchor_dir}</dd>
              <dt>Apply with</dt><dd className="mono" style={{ fontSize: 12 }}>{sys.apply_command}</dd>
              <dt>Privilege</dt><dd>{sys.is_root ? <Badge kind="green">running as root</Badge> : <Badge kind="muted">polkit on write</Badge>}</dd>
            </div>
          ) : <div style={{ height: 80 }} />}
        </div>

        <div className="panel">
          <div className="panel-head">
            <span className="panel-title">Installed anchors</span>
            <Link to="/trust" className="btn btn-sm btn-ghost">Manage →</Link>
          </div>
          {withCert.length === 0 ? (
            <div className="empty" style={{ padding: '32px 20px' }}><span>No CA anchors installed yet.</span></div>
          ) : (
            <div style={{ padding: '4px 4px' }}>
              <table>
                <tbody>
                  {withCert.slice(0, 6).map((a) => (
                    <tr key={a.name}>
                      <td><span className="name">{a.name}</span></td>
                      <td className="dim" style={{ fontSize: 12.5 }}>{fmtDate(a.cert!.not_after_ts)}</td>
                      <td style={{ textAlign: 'right' }}><ExpiryBadge ts={a.cert!.not_after_ts} /></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      <div className="panel section-gap">
        <div className="panel-head">
          <span className="panel-title">Browser stores (NSS)</span>
          <button className="btn btn-sm btn-primary" onClick={syncBrowsers} disabled={syncing}>
            <IconRefresh className="btn-ico" /> {syncing ? 'Syncing…' : 'Sync certificates'}
          </button>
        </div>
        <div style={{ padding: '6px 18px 16px' }}>
          {nss && nss.length > 0 ? (
            <>
              <table>
                <tbody>
                  {nss.map((d) => (
                    <tr key={d.dir}>
                      <td style={{ width: 1, whiteSpace: 'nowrap' }}><span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--green)', display: 'inline-block' }} /></td>
                      <td><span className="name">{d.label}</span></td>
                      <td className="dim mono" style={{ fontSize: 11.5 }}>{d.dir}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="hint" style={{ marginTop: 12 }}>
                Sync re-imports every managed CA and identity into these browsers — run it after installing a new browser. Close the browser first.
              </p>
            </>
          ) : (
            <p className="hint">
              No browser NSS databases detected yet. Install/launch a browser (Firefox, Chrome, Zen…), then Sync. A shared <span className="mono">~/.pki/nssdb</span> is created automatically on first import.
            </p>
          )}
        </div>
      </div>

      {syncResults && (
        <Modal wide title="Browser sync results" onClose={() => setSyncResults(null)}>
          <div className="modal-body">
            <ResultList results={syncResults} />
            {syncResults.some((r) => /not found|libnss3-tools|nss-tools/.test(r.message)) && (
              <p className="hint">
                The NSS tools are missing. Install them: <span className="mono">sudo apt install libnss3-tools</span> (Debian/Ubuntu) or <span className="mono">sudo dnf install nss-tools</span> (Fedora), then Sync again.
              </p>
            )}
          </div>
        </Modal>
      )}
    </>
  );
}
