import { Component, ErrorInfo, ReactNode } from 'react';

export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('LCM UI crashed:', error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="login-wrap">
          <div className="card" style={{ maxWidth: 460 }}>
            <strong style={{ color: 'var(--red)' }}>Something went wrong</strong>
            <p className="hint" style={{ marginTop: 8 }}>{String(this.state.error.message)}</p>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
