import { useState } from 'react';
import { api } from '../api/client';
import { useAsync } from '../hooks';
import { Loading, EmptyState, Modal, CertSheet, SearchInput, ExpiryBadge, fmtDate, short } from '../components/ui';
import { IconActivity } from '../components/icons';
import type { CertInfo } from '../types';

export default function Audit() {
  const { data, loading } = useAsync(() => api.listSystemTrust());
  const [query, setQuery] = useState('');
  const [detail, setDetail] = useState<CertInfo | null>(null);

  const items = (data ?? []).filter((c) => {
    const q = query.toLowerCase();
    return !q || c.subject.toLowerCase().includes(q) || c.fingerprint_sha256.toLowerCase().includes(q);
  });

  if (loading) return <Loading />;

  return (
    <>
      <div className="row between" style={{ marginBottom: 18 }}>
        <div className="row" style={{ gap: 12 }}>
          <span className="dim">{data?.length ?? 0} trusted CA{(data?.length ?? 0) === 1 ? '' : 's'} on this machine</span>
          {(data?.length ?? 0) > 0 && <SearchInput value={query} onChange={setQuery} />}
        </div>
      </div>

      {!data || data.length === 0 ? (
        <div className="panel">
          <EmptyState icon={<IconActivity size={34} />} title="No system trust bundle found" hint="LCM reads the consolidated CA bundle (e.g. /etc/ssl/certs/ca-certificates.crt)." />
        </div>
      ) : items.length === 0 ? (
        <div className="panel"><EmptyState icon={<IconActivity size={34} />} title="No matches" hint={`Nothing matches “${query}”.`} /></div>
      ) : (
        <div className="panel">
          <table>
            <thead>
              <tr><th>Subject</th><th>Expires</th><th>Status</th><th></th></tr>
            </thead>
            <tbody>
              {items.map((c, i) => (
                <tr key={c.fingerprint_sha256 + i}>
                  <td><span className="name">{short(c.subject, 70)}</span></td>
                  <td className="dim mono" style={{ fontSize: 12.5 }}>{fmtDate(c.not_after_ts)}</td>
                  <td><ExpiryBadge ts={c.not_after_ts} /></td>
                  <td style={{ textAlign: 'right' }}>
                    <button className="btn btn-sm btn-ghost" onClick={() => setDetail(c)}>Details</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {detail && (
        <Modal wide title="Trusted CA" onClose={() => setDetail(null)}>
          <div className="modal-body"><CertSheet cert={detail} /></div>
        </Modal>
      )}
    </>
  );
}
