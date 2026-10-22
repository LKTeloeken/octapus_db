import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import type { ErrorDetails } from '@/lib/error-details';

/** Quanto tempo o "Copiado" fica no botão */
const COPIED_MS = 1500;

/** Copia o erro (título e mensagem) e avisa no próprio botão */
export const useErrorDetails = (details: ErrorDetails) => {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), COPIED_MS);
    return () => clearTimeout(timer);
  }, [copied]);

  const copy = async () => {
    try {
      await writeText(`${details.title}\n${details.message}`);
      setCopied(true);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return { copied, copy };
};
