import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { useAssistantMe } from '@/queries/use-assistant-me';
import { useLogoutAssistant } from '@/queries/use-assistant-session';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useSettings, useUpdateSettings } from '@/queries/use-settings';

const showError = (error: unknown) =>
  toast.error(error instanceof Error ? error.message : String(error));

export const useAssistantSection = () => {
  const { data: settings, isPending } = useSettings();
  const { mutate } = useUpdateSettings();
  const { data: status } = useAssistantStatus();
  const loggedIn = status?.loggedIn === true;
  const { data: me } = useAssistantMe(loggedIn && status?.online === true);
  const logout = useLogoutAssistant();

  const setAutoExecute = useCallback(
    (assistantAutoExecute: boolean) =>
      mutate({ assistantAutoExecute }, { onError: showError }),
    [mutate],
  );

  const signOut = useCallback(
    () => logout.mutate(undefined, { onError: showError }),
    [logout],
  );

  return {
    autoExecute: settings?.assistantAutoExecute ?? false,
    isLoading: isPending,
    setAutoExecute,
    configured: status?.configured ?? false,
    loggedIn,
    participantLabel: me?.participant.label ?? null,
    signOut,
    isSigningOut: logout.isPending,
  };
};
