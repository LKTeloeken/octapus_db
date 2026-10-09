import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useSettings, useUpdateSettings } from '@/queries/use-settings';

export const useUpdatesSection = () => {
  const { data: settings, isPending } = useSettings();
  const { mutate } = useUpdateSettings();

  const setBetaUpdates = useCallback(
    (betaUpdates: boolean) =>
      mutate(
        { betaUpdates },
        {
          onError: error =>
            toast.error(error instanceof Error ? error.message : String(error)),
        },
      ),
    [mutate],
  );

  return {
    betaUpdates: settings?.betaUpdates ?? false,
    isLoading: isPending,
    setBetaUpdates,
  };
};
