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
        <div className="flex min-w-72 flex-col gap-3 rounded-xl border border-white/10 bg-[rgba(18,18,18,0.5)] p-4 text-white shadow-[0_4px_20px_rgba(0,0,0,0.25)] backdrop-blur-md">
          {status === 'available' && (
            <>
              <div>
                <p className="text-sm font-medium">Atualização disponível</p>
                <p className="text-xs text-white/60">Versão {version}</p>
              </div>
              <div className="flex justify-end gap-2">
                <Button
                  variant="ghost"
                  className="text-white hover:bg-white/10"
                  onClick={() => toast.dismiss(TOAST_ID)}
                >
                  Depois
                </Button>
                <Button onClick={install}>Atualizar agora</Button>
              </div>
            </>
          )}

          {status === 'downloading' && (
            <>
              <p className="text-sm font-medium">Baixando atualização…</p>
              <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
                <div
                  className="h-full rounded-full bg-white transition-[width] duration-200"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <p className="text-xs text-white/60">{progress}%</p>
            </>
          )}

          {status === 'ready' && (
            <>
              <div>
                <p className="text-sm font-medium">Atualização instalada</p>
                <p className="text-xs text-white/60">
                  Reinicie para usar a versão {version}.
                </p>
              </div>
              <div className="flex justify-end gap-2">
                <Button
                  variant="ghost"
                  className="text-white hover:bg-white/10"
                  onClick={() => toast.dismiss(TOAST_ID)}
                >
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
