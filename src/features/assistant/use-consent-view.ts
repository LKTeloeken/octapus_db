import { openUrl } from '@tauri-apps/plugin-opener';
import { useCallback } from 'react';
import toast from 'react-hot-toast';
import { hasErrorCode } from '@/api/client';
import {
  useAcceptAssistantConsent,
  useAssistantConsent,
} from '@/queries/use-assistant-consent';
import { useLogoutAssistant } from '@/queries/use-assistant-session';

export const useConsentView = () => {
  const term = useAssistantConsent();
  const accept = useAcceptAssistantConsent();
  const logout = useLogoutAssistant();

  const acceptTerm = useCallback(() => {
    if (!term.data) return;
    accept.mutate(term.data.version, {
      onError: error => {
        toast.error(
          hasErrorCode(error, 'ASSISTANT_CONFLICT')
            ? 'O termo mudou enquanto você lia. Leia a versão nova.'
            : error instanceof Error
              ? error.message
              : String(error),
        );
      },
    });
  }, [accept, term.data]);

  return {
    term: term.data ?? null,
    isLoading: term.isPending,
    error: term.error
      ? term.error instanceof Error
        ? term.error.message
        : String(term.error)
      : null,
    retry: () => void term.refetch(),
    acceptTerm,
    isAccepting: accept.isPending,
    signOut: () => logout.mutate(),
    openLink: (href: string) => void openUrl(href),
  };
};
