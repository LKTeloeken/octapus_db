import { useCallback, useEffect, useRef, useState } from 'react';
import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';
import toast from 'react-hot-toast';

/** Atraso antes de checar, para não competir com a carga inicial do app. */
const CHECK_DELAY_MS = 5_000;

export type UpdateStatus = 'idle' | 'available' | 'downloading' | 'ready';

export const useUpdateNotifier = () => {
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [version, setVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);

  const updateRef = useRef<Update | null>(null);
  const hasChecked = useRef(false);

  useEffect(() => {
    // O StrictMode monta o efeito duas vezes em dev; só a primeira checa.
    if (hasChecked.current) return;
    hasChecked.current = true;

    const timer = setTimeout(async () => {
      try {
        const update = await check();
        if (!update) return;

        updateRef.current = update;
        setVersion(update.version);
        setStatus('available');
      } catch (error) {
        // Checar atualização é best-effort: falha de rede não incomoda o usuário.
        console.error('Falha ao verificar atualizações:', error);
      }
    }, CHECK_DELAY_MS);

    return () => clearTimeout(timer);
  }, []);

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;

    setStatus('downloading');
    setProgress(0);

    try {
      let downloaded = 0;
      let total = 0;

      await update.downloadAndInstall(event => {
        switch (event.event) {
          case 'Started':
            total = event.data.contentLength ?? 0;
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            if (total > 0) {
              setProgress(Math.round((downloaded / total) * 100));
            }
            break;
          case 'Finished':
            setProgress(100);
            break;
        }
      });

      setStatus('ready');
    } catch (error) {
      // Aqui o usuário pediu a ação explicitamente, então merece o aviso.
      toast.error(error instanceof Error ? error.message : String(error));
      setStatus('available');
    }
  }, []);

  const restart = useCallback(async () => {
    try {
      await relaunch();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  }, []);

  return { status, version, progress, install, restart };
};
