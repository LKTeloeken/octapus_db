import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useCliStatus, useInstallCli } from '@/queries/use-cli';

export const useTerminalSection = () => {
  const { data: status } = useCliStatus();
  const { mutate, isPending } = useInstallCli();

  const install = useCallback(
    () =>
      mutate(undefined, {
        onSuccess: installed =>
          toast.success(`octapus instalado em ${installed.path}`),
        onError: error =>
          toast.error(error instanceof Error ? error.message : String(error)),
      }),
    [mutate],
  );

  // A pasta onde o binário fica, para a instrução do PATH
  const directory = status?.path.replace(/[/\\][^/\\]+$/, '') ?? '';

  return { status, directory, install, isInstalling: isPending };
};
