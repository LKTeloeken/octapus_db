import { useMutation } from '@tanstack/react-query';
import { openLogDir } from '@/api/logs';

/** Abre a pasta de logs; nada em cache muda, então não há invalidação */
export function useOpenLogDir() {
  return useMutation({ mutationFn: openLogDir });
}
