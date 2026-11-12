import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useOpenLogDir } from '@/queries/use-open-log-dir';

export const useDiagnosticsSection = () => {
  const { mutate, isPending } = useOpenLogDir();

  const openLogs = useCallback(
    () =>
      mutate(undefined, {
        onError: error =>
          toast.error(error instanceof Error ? error.message : String(error)),
      }),
    [mutate],
  );

  const reloadInterface = useCallback(() => window.location.reload(), []);

  return { openLogs, isOpening: isPending, reloadInterface };
};
