import { createContext, useCallback, useContext, useState, ReactNode } from 'react';

type ToastKind = 'success' | 'error' | 'info';
interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

const ToastCtx = createContext<(message: string, kind?: ToastKind) => void>(() => {});

/** `const toast = useToast(); toast('Done', 'success');` */
export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);

  const push = useCallback(
    (message: string, kind: ToastKind = 'info') => {
      const id = Date.now() + Math.random();
      setToasts((t) => [...t, { id, kind, message }]);
      setTimeout(() => dismiss(id), 3800);
    },
    [dismiss],
  );

  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="toasts">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast-${t.kind}`} onClick={() => dismiss(t.id)}>
            <span className="toast-ico">{t.kind === 'success' ? '✓' : t.kind === 'error' ? '✗' : '•'}</span>
            <span>{t.message}</span>
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
