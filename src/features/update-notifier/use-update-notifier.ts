import { useCallback, useEffect, useRef, useState } from 'react';
import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import toast from 'react-hot-toast';
import { checkForUpdate } from '@/api/updater';
import { useSettings } from '@/queries/use-settings';
import { flushTabsSession } from '@/stores/tabs-session';

/** Atraso antes de checar, para não competir com a carga inicial do app. */
const CHECK_DELAY_MS = 5_000;

export type UpdateStatus = 'idle' | 'available' | 'downloading' | 'ready';

export const useUpdateNotifier = () => {
  const [status, setStatus] = useState<UpdateStatus>('idle');
  const [version, setVersion] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);

  const updateRef = useRef<Update | null>(null);
  /** Canal da última checagem — `null` antes da primeira */
  const checkedChannel = useRef<boolean | null>(null);
  const statusRef = useRef(status);
  statusRef.current = status;

  // O canal vem das preferências (SQLite): sem elas carregadas, não checa.
  const beta = useSettings().data?.betaUpdates;

  useEffect(() => {
    if (beta === undefined) return;
    // O StrictMode monta o efeito duas vezes em dev; e mudar outra preferência
    // não pode checar de novo — só a troca de canal.
    if (checkedChannel.current === beta) return;
    // Já há um update na mão (ou baixando): trocar de canal não o descarta.
    if (statusRef.current !== 'idle') return;
    const isFirstCheck = checkedChannel.current === null;
    checkedChannel.current = beta;

    // Na partida, espera a carga inicial; ao ligar o beta, checa na hora.
    let fired = false;
    const timer = setTimeout(
      async () => {
        fired = true;
        try {
          const update = await checkForUpdate(beta).catch(error => {
            // A API do GitHub fora (ou sem cota): o canal estável ainda serve.
            if (!beta) throw error;
            console.error('Falha no canal beta, usando o estável:', error);
            return checkForUpdate(false);
          });
          if (!update) return;

          updateRef.current = update;
          setVersion(update.version);
          setStatus('available');
        } catch (error) {
          // Checar atualização é best-effort: falha de rede não incomoda o usuário.
          console.error('Falha ao verificar atualizações:', error);
        }
      },
      isFirstCheck ? CHECK_DELAY_MS : 0,
    );

    return () => {
      clearTimeout(timer);
      // Saiu antes de checar (StrictMode, troca de canal no meio da espera):
      // o próximo efeito é que faz a checagem.
      if (!fired) checkedChannel.current = isFirstCheck ? null : !beta;
    };
  }, [beta]);

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;

    setStatus('downloading');
    setProgress(0);

    try {
      // No Windows o instalador fecha o app de dentro do downloadAndInstall —
      // o `restart` nem chega a rodar. A sessão precisa estar em disco antes.
      await flushTabsSession();

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
      // O usuário pode ter seguido editando enquanto o update baixava.
      await flushTabsSession();
      await relaunch();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  }, []);

  return { status, version, progress, install, restart };
};
