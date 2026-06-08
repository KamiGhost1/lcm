import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { api, IS_TAURI } from '../api/client';
import { useAsync } from '../hooks';
import { Badge } from './ui';
import { IconOverview, IconShield, IconLicenses, IconServers, IconChip, IconActivity, Mark } from './icons';

const nav = [
  { to: '/', label: 'Overview', Icon: IconOverview, end: true },
  { to: '/trust', label: 'Trust · CA', Icon: IconShield },
  { to: '/clients', label: 'Client IDs', Icon: IconLicenses },
  { to: '/servers', label: 'Server certs', Icon: IconServers },
  { to: '/audit', label: 'Audit', Icon: IconActivity },
];

const titles: Record<string, { t: string; s: string }> = {
  '/': { t: 'Overview', s: 'Certificate trust on this machine at a glance' },
  '/trust': { t: 'Trust · CA Anchors', s: 'Install and remove CA certificates in the system trust store' },
  '/clients': { t: 'Client Identities', s: 'Certificate + key pairs for mTLS and VPN authentication' },
  '/servers': { t: 'Server Certificates', s: 'Deploy certificates to web / proxy services and reload them' },
  '/audit': { t: 'System Trust Audit', s: 'Every CA this machine trusts (read-only)' },
};

export default function Layout() {
  const { pathname } = useLocation();
  const meta = titles[pathname] ?? { t: '', s: '' };
  const { data: sys } = useAsync(() => api.systemInfo());

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <Mark />
          <div>
            <div className="brand-name">LCM</div>
            <div className="brand-sub">linux cert manager</div>
          </div>
        </div>
        <div className="nav-label">Manage</div>
        {nav.map(({ to, label, Icon, end }) => (
          <NavLink key={to} to={to} end={end} className="nav-item">
            <Icon className="nav-ico" />
            {label}
          </NavLink>
        ))}
        <div className="sidebar-foot">v{__APP_VERSION__} · phase 1{IS_TAURI ? '' : ' · demo data'}</div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div>
            <div className="page-title">{meta.t}</div>
            <div className="page-sub">{meta.s}</div>
          </div>
          <div className="topbar-right">
            {!IS_TAURI && <Badge kind="amber">demo data</Badge>}
            {sys && (
              <span className="badge badge-blue" title={`${sys.anchor_dir} · ${sys.apply_command}`}>
                <IconChip size={13} />
                {sys.supported ? sys.distro_family : 'unsupported distro'}
              </span>
            )}
            {sys && (
              <Badge kind={sys.is_root ? 'green' : 'muted'}>{sys.is_root ? 'root' : 'polkit'}</Badge>
            )}
          </div>
        </header>
        <div className="content">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
