// Minimal stroke icons (currentColor). Laconic, no icon library dependency.
// Ported from the kamienclave control-plane web UI.
type P = { className?: string; size?: number };
const base = (size = 18) => ({
  width: size, height: size, viewBox: '0 0 24 24', fill: 'none',
  stroke: 'currentColor', strokeWidth: 1.7, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const,
});

export const IconOverview = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/></svg>
);
export const IconShield = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="m9 12 2 2 4-4"/></svg>
);
export const IconLicenses = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><circle cx="7.5" cy="15.5" r="4.5"/><path d="m10.8 12.2 8.4-8.4"/><path d="m18 5 2 2"/><path d="m15 8 2 2"/></svg>
);
export const IconServers = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><rect x="3" y="4" width="18" height="7" rx="1.5"/><rect x="3" y="13" width="18" height="7" rx="1.5"/><path d="M7 7.5h.01M7 16.5h.01"/></svg>
);
export const IconUpload = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m17 8-5-5-5 5"/><path d="M12 3v12"/></svg>
);
export const IconPlus = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M12 5v14M5 12h14"/></svg>
);
export const IconClose = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M18 6 6 18M6 6l12 12"/></svg>
);
export const IconTrash = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>
);
export const IconActivity = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>
);
export const IconClock = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>
);
export const IconCheck = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M20 6 9 17l-5-5"/></svg>
);
export const IconRefresh = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/></svg>
);
export const IconChip = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v2M15 2v2M9 20v2M15 20v2M2 9h2M2 15h2M20 9h2M20 15h2"/></svg>
);
export const IconKey = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><circle cx="7.5" cy="15.5" r="4.5"/><path d="m10.8 12.2 8.4-8.4 2 2M14 9l2 2"/></svg>
);
export const IconGlobe = ({ className, size }: P) => (
  <svg {...base(size)} className={className}><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.5 2.5 2.5 15 0 18M12 3c-2.5 2.5-2.5 15 0 18"/></svg>
);

export const Mark = ({ size = 26 }: { size?: number }) => (
  <svg className="brand-mark" width={size} height={size} viewBox="0 0 32 32" fill="none">
    <defs>
      <linearGradient id="lcmg" x1="0" y1="0" x2="32" y2="32">
        <stop stopColor="#7aa2f7"/><stop offset="1" stopColor="#bb9af7"/>
      </linearGradient>
    </defs>
    <path d="M16 2 4 9v9.5C4 25 16 30 16 30s12-5 12-11.5V9L16 2z" stroke="url(#lcmg)" strokeWidth="1.8" fill="rgba(122,162,247,0.08)"/>
    <path d="m11 16 3.4 3.4L21 12.5" stroke="url(#lcmg)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" fill="none"/>
  </svg>
);
