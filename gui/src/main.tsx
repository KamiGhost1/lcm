import React from 'react';
import ReactDOM from 'react-dom/client';
import { HashRouter, Routes, Route } from 'react-router-dom';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ToastProvider } from './components/Toast';
import Layout from './components/Layout';
import Overview from './pages/Overview';
import Trust from './pages/Trust';
import ClientIds from './pages/ClientIds';
import ServerCerts from './pages/ServerCerts';
import Audit from './pages/Audit';
import './styles.css';

// HashRouter: the app is loaded from a file:// origin inside Tauri, where the
// History API and path-based routing don't behave; hash routing just works.
ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <ToastProvider>
        <HashRouter>
          <Routes>
            <Route element={<Layout />}>
              <Route index element={<Overview />} />
              <Route path="trust" element={<Trust />} />
              <Route path="clients" element={<ClientIds />} />
              <Route path="servers" element={<ServerCerts />} />
              <Route path="audit" element={<Audit />} />
            </Route>
          </Routes>
        </HashRouter>
      </ToastProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
