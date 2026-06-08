import { ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import { IconClose, IconUpload } from './icons';
import type { CertInfo, OpResult } from '../types';

// ---- formatters ----

/** Format a Unix-second timestamp as YYYY-MM-DD. */
export function fmtDate(ts: number | null | undefined): string {
  if (!ts) return '—';
  return new Date(ts * 1000).toLocaleDateString('en-CA');
}

/** Whole days from now until `ts` (negative = already past). */
export function daysUntil(ts: number): number {
  return Math.floor((ts * 1000 - Date.now()) / 86400000);
}

export const short = (h: string, n = 16) => (h.length > n ? `${h.slice(0, n)}…` : h);

/** A search box that focuses on `/` (when not already typing in a field). */
export function SearchInput({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder?: string }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const tag = (document.activeElement as HTMLElement | null)?.tagName;
      if (e.key === '/' && tag !== 'INPUT' && tag !== 'TEXTAREA') {
        e.preventDefault();
        ref.current?.focus();
      }
    };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, []);
  return (
    <input
      ref={ref}
      className="input search"
      value={value}
      placeholder={placeholder ?? 'Search…  /'}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => e.key === 'Escape' && onChange('')}
    />
  );
}

/** Whether base64-encoded bytes look like a PEM bundle (vs binary PKCS#12). */
export function looksPem(bytesB64: string): boolean {
  try {
    return atob(bytesB64.slice(0, 88)).includes('-----BEGIN');
  } catch {
    return false;
  }
}

// ---- components ----

export function Badge({ kind, children }: { kind: string; children: ReactNode }) {
  return <span className={`badge badge-${kind}`}><span className="dot" />{children}</span>;
}

/** Expiry badge driven by the cert's not_after timestamp. */
export function ExpiryBadge({ ts }: { ts: number }) {
  const d = daysUntil(ts);
  if (d < 0) return <Badge kind="red">expired</Badge>;
  if (d <= 30) return <Badge kind="amber">{d}d left</Badge>;
  return <Badge kind="green">valid</Badge>;
}

export function Modal({ title, onClose, children, footer, wide }: { title: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" style={wide ? { width: 600 } : undefined} onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">{title}<span className="x" onClick={onClose}><IconClose size={18} /></span></div>
        {children}
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return <div className="field"><label>{label}</label>{children}</div>;
}

export function EmptyState({ icon, title, hint }: { icon: ReactNode; title: string; hint?: string }) {
  return <div className="empty"><div className="empty-ico">{icon}</div><strong>{title}</strong>{hint && <span>{hint}</span>}</div>;
}

export function Loading() {
  return <div className="loading"><div className="spinner" /></div>;
}

/** A file drop zone that hands back the chosen file's bytes (base64) + name. */
export function DropZone({ onFile, accept }: { onFile: (filename: string, bytesB64: string) => void; accept?: string }) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [drag, setDrag] = useState(false);

  const read = useCallback((file: File) => {
    const reader = new FileReader();
    reader.onload = () => {
      const buf = reader.result as ArrayBuffer;
      let bin = '';
      const bytes = new Uint8Array(buf);
      for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
      onFile(file.name, btoa(bin));
    };
    reader.readAsArrayBuffer(file);
  }, [onFile]);

  return (
    <div
      className={`dropzone${drag ? ' drag' : ''}`}
      onClick={() => inputRef.current?.click()}
      onDragOver={(e) => { e.preventDefault(); setDrag(true); }}
      onDragLeave={() => setDrag(false)}
      onDrop={(e) => { e.preventDefault(); setDrag(false); const f = e.dataTransfer.files[0]; if (f) read(f); }}
    >
      <div className="dropzone-ico"><IconUpload size={26} /></div>
      <strong>Drop a certificate here</strong>
      <span>or click to choose a .crt / .pem / .cer / .der file</span>
      <input
        ref={inputRef}
        type="file"
        accept={accept ?? '.crt,.pem,.cer,.der,.crt.pem'}
        style={{ display: 'none' }}
        onChange={(e) => { const f = e.target.files?.[0]; if (f) read(f); }}
      />
    </div>
  );
}

/** Key/value sheet describing a certificate. */
export function CertSheet({ cert }: { cert: CertInfo }) {
  return (
    <dl className="kv">
      <dt>Subject</dt><dd>{cert.subject}</dd>
      <dt>Issuer</dt><dd>{cert.issuer}</dd>
      <dt>Serial</dt><dd className="mono">{cert.serial}</dd>
      <dt>Valid from</dt><dd>{fmtDate(cert.not_before_ts)}</dd>
      <dt>Valid until</dt><dd>{fmtDate(cert.not_after_ts)} · <ExpiryBadge ts={cert.not_after_ts} /></dd>
      <dt>Type</dt><dd>{cert.is_ca ? 'Certificate Authority' : 'Leaf certificate'}</dd>
      <dt>SHA-256</dt><dd className="fp">{cert.fingerprint_sha256}</dd>
    </dl>
  );
}

/** Renders the ✓/✗ result lines returned by a privileged operation. */
export function ResultList({ results }: { results: OpResult[] }) {
  return (
    <div>
      {results.map((r, i) => (
        <div key={i} className={`result ${r.ok ? 'ok' : 'err'}`}>
          <span>{r.ok ? '✓' : '✗'}</span>
          <span>{r.op}</span>
          <span className="dim">— {r.message}</span>
        </div>
      ))}
    </div>
  );
}
