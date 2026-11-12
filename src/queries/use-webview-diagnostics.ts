import { useEffect } from 'react';
import { reportWebviewState } from '@/api/logs';

/** A partir de quanto tempo escondida a volta da janela entra no log */
const LONG_HIDDEN_MS = 30 * 60 * 1000;

/**
 * Leva ao log do app o que só a webview sabe: quando ela se diz sem rede (o
 * WebKitGTK erra isso) e quando a janela volta depois de muito tempo
 * escondida. Nada disso muda o comportamento — as queries não dependem do
 * `navigator.onLine` (`networkMode: 'always'` no `query-provider`) —, mas
 * explica um relato de app parado depois de dias aberto.
 */
export function useWebviewDiagnostics() {
  useEffect(() => {
    const report = (hiddenMinutes?: number) => {
      // Diagnóstico nunca atrapalha: falha de log é ignorada
      reportWebviewState(navigator.onLine, hiddenMinutes).catch(() => {});
    };

    const onNetwork = () => report();

    let hiddenAt: number | null = null;
    const onVisibility = () => {
      if (document.hidden) {
        hiddenAt = Date.now();
        return;
      }
      if (hiddenAt === null) return;
      const elapsed = Date.now() - hiddenAt;
      hiddenAt = null;
      if (elapsed >= LONG_HIDDEN_MS) report(Math.round(elapsed / 60_000));
    };

    window.addEventListener('online', onNetwork);
    window.addEventListener('offline', onNetwork);
    document.addEventListener('visibilitychange', onVisibility);
    return () => {
      window.removeEventListener('online', onNetwork);
      window.removeEventListener('offline', onNetwork);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  }, []);
}
