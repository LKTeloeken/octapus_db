import React from 'react';
import { Toaster } from 'react-hot-toast';

/**
 * Toasts em vidro (DESIGN.md §7): mesmas variáveis do utilitário `glass`, em
 * `style` porque o react-hot-toast aplica o próprio estilo inline por cima de
 * classes. Com transparência reduzida, `--glass` continua legível (78–80%).
 */
export function CustomToaster(): React.JSX.Element {
  return (
    <Toaster
      position="bottom-right"
      toastOptions={{
        style: {
          background: 'var(--glass)',
          backdropFilter: 'blur(20px) saturate(1.4)',
          WebkitBackdropFilter: 'blur(20px) saturate(1.4)',
          border: '1px solid var(--glass-border)',
          borderRadius: '12px',
          boxShadow: 'var(--elev-3), inset 0 1px 0 var(--glass-highlight)',
          color: 'var(--fg)',
          padding: '10px 14px',
          fontFamily: "'Geist Variable', ui-sans-serif, system-ui, sans-serif",
          fontSize: '13px',
          lineHeight: '20px',
        },
        success: {
          iconTheme: {
            primary: 'var(--success)',
            secondary: 'var(--surface-3)',
          },
        },
        error: {
          iconTheme: {
            primary: 'var(--danger)',
            secondary: 'var(--surface-3)',
          },
        },
      }}
    />
  );
}
