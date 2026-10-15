import { open } from '@tauri-apps/plugin-dialog';
import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useSettings } from '@/queries/use-settings';
import { usePgToolsDetection, useSetPgToolsDir } from '@/queries/use-pg-tools';

const toastError = (error: unknown) =>
  toast.error(error instanceof Error ? error.message : String(error));

export const usePgToolsSection = () => {
  const { data: settings } = useSettings();
  const detection = usePgToolsDetection();
  const { mutate, isPending: isSaving } = useSetPgToolsDir();

  const configuredDir = settings?.pgToolsDir ?? null;
  const selected = detection.data?.selected ?? null;
  // As outras instalações achadas, para trocar com um clique
  const others = (detection.data?.candidates ?? []).filter(
    install => install.dir !== selected?.dir,
  );

  const setDir = useCallback(
    (dir: string | null) => mutate(dir, { onError: toastError }),
    [mutate],
  );

  const chooseDir = useCallback(async () => {
    const dir = await open({
      directory: true,
      title: 'Pasta com o pg_dump',
      defaultPath: configuredDir ?? selected?.dir,
    });
    if (typeof dir === 'string') setDir(dir);
  }, [configuredDir, selected?.dir, setDir]);

  const resetToAutomatic = useCallback(() => setDir(null), [setDir]);

  const { refetch } = detection;
  const redetect = useCallback(
    () =>
      void refetch().then(result => {
        if (result.error) toastError(result.error);
      }),
    [refetch],
  );

  return {
    configuredDir,
    selected,
    others,
    configuredError: detection.data?.configuredError ?? null,
    isDetecting: detection.isFetching,
    detectError: detection.error,
    isSaving,
    chooseDir,
    resetToAutomatic,
    setDir,
    redetect,
  };
};
