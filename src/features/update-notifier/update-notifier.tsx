import { useEffect } from 'react';
import toast from 'react-hot-toast';
import { Button } from '@/components/ui/button';
import { useUpdateNotifier } from './use-update-notifier';

/** Id fixo: o mesmo toast é reaproveitado ao longo de todo o fluxo. */
const TOAST_ID = 'app-update';

export function UpdateNotifier() {
  const { status, version, progress, install, restart } = useUpdateNotifier();

  useEffect(() => {
    if (status === 'idle') return;

    toast.custom(
      () => (
        <div className="glass flex min-w-72 flex-col gap-3 rounded-lg p-4 text-fg">
          {status === 'available' && (
            <>
              <div>
                <p className="text-body font-medium">Atualização disponível</p>
                <p className="text-small text-fg-muted">Versão {version}</p>
              </div>
              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => toast.dismiss(TOAST_ID)}>
                  Depois
                </Button>
                <Button onClick={install}>Atualizar agora</Button>
              </div>
            </>
          )}

          {status === 'downloading' && (
            <>
              <p className="text-body font-medium">Baixando atualização…</p>
              <div className="h-1 overflow-hidden rounded-full bg-hover">
                <div
                  className="h-full rounded-full bg-info transition-[width] duration-180"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <p className="text-small text-fg-muted">{progress}%</p>
            </>
          )}

          {status === 'ready' && (
            <>
              <div>
                <p className="text-body font-medium">Atualização instalada</p>
                <p className="text-small text-fg-muted">
                  Reinicie para usar a versão {version}.
                </p>
              </div>
              <div className="flex justify-end gap-2">
                <Button variant="ghost" onClick={() => toast.dismiss(TOAST_ID)}>
                  Depois
                </Button>
                <Button onClick={restart}>Reiniciar</Button>
              </div>
            </>
          )}
        </div>
      ),
      { id: TOAST_ID, duration: Infinity, position: 'bottom-right' },
    );
  }, [status, version, progress, install, restart]);

  return null;
}
