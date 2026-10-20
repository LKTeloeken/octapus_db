import { useCallback, useState, type FormEvent } from 'react';
import { hasErrorCode } from '@/api/client';
import { useRedeemAssistantInvite } from '@/queries/use-assistant-session';

/** O convite tem pelo menos 8 caracteres (a API recusa menos) */
const MIN_INVITE_LENGTH = 8;

function loginErrorMessage(error: unknown): string {
  if (hasErrorCode(error, 'ASSISTANT_UNAUTHORIZED')) {
    return 'Convite inválido ou revogado.';
  }
  if (hasErrorCode(error, 'ASSISTANT_INVALID_REQUEST')) {
    return 'Confira o código do convite.';
  }
  if (hasErrorCode(error, 'ASSISTANT_UNAVAILABLE')) {
    return 'Sem conexão com o assistente. Tente de novo em instantes.';
  }
  return error instanceof Error ? error.message : String(error);
}

export const useLoginForm = () => {
  const [inviteCode, setInviteCode] = useState('');
  const redeem = useRedeemAssistantInvite();

  const trimmed = inviteCode.trim();
  const canSubmit = trimmed.length >= MIN_INVITE_LENGTH && !redeem.isPending;

  const submit = useCallback(
    (event?: FormEvent) => {
      event?.preventDefault();
      if (!canSubmit) return;
      redeem.mutate({ inviteCode: trimmed });
    },
    [canSubmit, redeem, trimmed],
  );

  return {
    inviteCode,
    setInviteCode: (value: string) => {
      setInviteCode(value);
      if (redeem.error) redeem.reset();
    },
    canSubmit,
    submit,
    isSubmitting: redeem.isPending,
    error: redeem.error ? loginErrorMessage(redeem.error) : null,
  };
};
