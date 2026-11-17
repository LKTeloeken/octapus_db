import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useCallback } from 'react';
import toast from 'react-hot-toast';

const TOAST_ID = 'query-messages-copy';

/** Copia um trecho do log (já formatado) e avisa pelo toast */
export const useCopyLogText = () =>
  useCallback(async (text: string, message: string) => {
    if (!text) return;
    try {
      await writeText(text);
      toast.success(message, { id: TOAST_ID });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error), {
        id: TOAST_ID,
      });
    }
  }, []);
