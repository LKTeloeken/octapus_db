import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import toast from 'react-hot-toast';
import { create } from 'zustand';
import { checkForUpdate } from '@/api/updater';
import { compareVersions, type UpdateStatus } from '@/lib/update-schedule';
import { flushTabsSession } from './tabs-session';

interface UpdateState {
  status: UpdateStatus;
  /** Versão oferecida (ou já instalada, em `ready`) */
  version: string | null;
  /** Download em %, enquanto `downloading` */
  progress: number;
  /** Quando terminou a última checagem (com ou sem sucesso) */
  lastCheckedAt: number | null;
  checking: boolean;
  /**
   * A pessoa respondeu "Depois" para a versão atual: o toast some e o polling
   * não o traz de volta — só uma versão mais nova traz.
   */
  dismissed: boolean;
  update: Update | null;

  /**
   * Procura versão nova no canal pedido. Devolve a versão encontrada (ou
   * `null`) e rejeita se a checagem falhar — o polling engole o erro, a
   * checagem manual o mostra.
   */
  check: (beta: boolean) => Promise<string | null>;
  install: () => Promise<void>;
  restart: () => Promise<void>;
  dismiss: () => void;
}

/**
 * O auto-update do app: o mesmo estado para o toast (`UpdateNotifier`, que
 * também agenda as checagens) e para a seção Atualizações das configurações.
 */
export const useUpdateStore = create<UpdateState>((set, get) => ({
  status: 'idle',
  version: null,
  progress: 0,
  lastCheckedAt: null,
  checking: false,
  dismissed: false,
  update: null,

  check: async beta => {
    if (get().checking) return get().version;
    set({ checking: true });

    try {
      const update = await checkForUpdate(beta).catch(error => {
        // A API do GitHub fora (ou sem cota): o canal estável ainda serve.
        if (!beta) throw error;
        console.error('Falha no canal beta, usando o estável:', error);
        return checkForUpdate(false);
      });

      const { status, version } = get();
      // Baixando ou instalado enquanto checava: esse já é o update da vez.
      if (!update || status === 'downloading' || status === 'ready') {
        return status === 'idle' ? null : version;
      }

      // Só troca por uma versão mais nova: voltar do beta para o estável não
      // descarta a pre-release já oferecida.
      if (version && compareVersions(update.version, version) <= 0) {
        return version;
      }

      set({
        update,
        version: update.version,
        status: 'available',
        dismissed: false,
      });
      return update.version;
    } finally {
      set({ checking: false, lastCheckedAt: Date.now() });
    }
  },

  install: async () => {
    const { update } = get();
    if (!update) return;

    set({ status: 'downloading', progress: 0, dismissed: false });

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
              set({ progress: Math.round((downloaded / total) * 100) });
            }
            break;
          case 'Finished':
            set({ progress: 100 });
            break;
        }
      });

      set({ status: 'ready' });
    } catch (error) {
      // Aqui o usuário pediu a ação explicitamente, então merece o aviso.
      toast.error(error instanceof Error ? error.message : String(error));
      set({ status: 'available' });
    }
  },

  restart: async () => {
    try {
      // O usuário pode ter seguido editando enquanto o update baixava.
      await flushTabsSession();
      await relaunch();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  },

  dismiss: () => set({ dismissed: true }),
}));
