import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useSettings, useUpdateSettings } from '@/queries/use-settings';
import { useUpdateStore } from '@/stores/update-store';

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

/** "hoje às 14:32" ou "12/10 às 09:05" */
const formatCheckedAt = (timestamp: number) => {
  const date = new Date(timestamp);
  const time = date.toLocaleTimeString('pt-BR', {
    hour: '2-digit',
    minute: '2-digit',
  });
  const isToday = date.toDateString() === new Date().toDateString();
  if (isToday) return `hoje às ${time}`;
  const day = date.toLocaleDateString('pt-BR', {
    day: '2-digit',
    month: '2-digit',
  });
  return `${day} às ${time}`;
};

export const useUpdatesSection = () => {
  const { data: settings, isPending } = useSettings();
  const { mutate } = useUpdateSettings();

  const status = useUpdateStore(state => state.status);
  const version = useUpdateStore(state => state.version);
  const progress = useUpdateStore(state => state.progress);
  const lastCheckedAt = useUpdateStore(state => state.lastCheckedAt);
  const checking = useUpdateStore(state => state.checking);
  const check = useUpdateStore(state => state.check);
  const install = useUpdateStore(state => state.install);
  const restart = useUpdateStore(state => state.restart);

  const betaUpdates = settings?.betaUpdates ?? false;

  const setBetaUpdates = useCallback(
    (betaUpdates: boolean) =>
      mutate(
        { betaUpdates },
        { onError: error => toast.error(errorMessage(error)) },
      ),
    [mutate],
  );

  // Pedida pela pessoa: o resultado aparece, inclusive a falha.
  const checkNow = useCallback(async () => {
    try {
      const found = await check(betaUpdates);
      if (!found) toast.success('O Octapus está na versão mais recente.');
    } catch (error) {
      toast.error(`Não foi possível verificar: ${errorMessage(error)}`);
    }
  }, [check, betaUpdates]);

  const statusText =
    status === 'available'
      ? `Versão ${version} disponível.`
      : status === 'downloading'
        ? `Baixando a versão ${version}… ${progress}%`
        : status === 'ready'
          ? `Versão ${version} instalada — reinicie para usar.`
          : lastCheckedAt !== null
            ? `Última verificação ${formatCheckedAt(lastCheckedAt)}. O app procura sozinho a cada poucas horas.`
            : 'O app procura sozinho ao abrir e a cada poucas horas.';

  return {
    betaUpdates,
    isLoading: isPending,
    setBetaUpdates,
    status,
    statusText,
    checking,
    checkNow,
    install,
    restart,
  };
};
