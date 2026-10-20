import { hasErrorCode } from '@/api/client';
import type { AssistantMe, AssistantStatus } from '@/api/types/assistant.types';
import { consentPending } from '@/queries/use-assistant-me';

/**
 * Em que porta o painel está, na ordem em que elas se resolvem. Só a última
 * (`ready`) mostra as conversas; as outras explicam o que falta — sem afetar o
 * resto do app (C-19).
 */
export type AssistantGate =
  | { kind: 'loading' }
  /** Build sem a URL da API (ex.: pacote Nix) */
  | { kind: 'not-configured' }
  | { kind: 'update-required'; minAppVersion: string | null }
  /** API fora do ar ou inalcançável */
  | { kind: 'offline' }
  /** Sem token: entrar com o convite */
  | { kind: 'login' }
  /** A API ainda não publicou um termo: nada a aceitar, nada a usar */
  | { kind: 'no-term' }
  /** Termo vigente ainda não aceito (RN-11) */
  | { kind: 'consent' }
  | { kind: 'error'; message: string }
  | { kind: 'ready'; me: AssistantMe };

export interface GateInput {
  status: AssistantStatus | undefined;
  me: AssistantMe | undefined;
  meError: unknown;
}

export function assistantGate({
  status,
  me,
  meError,
}: GateInput): AssistantGate {
  if (!status) return { kind: 'loading' };
  if (!status.configured) return { kind: 'not-configured' };
  if (status.updateRequired) {
    return { kind: 'update-required', minAppVersion: status.minAppVersion };
  }
  if (!status.online) return { kind: 'offline' };
  if (!status.loggedIn) return { kind: 'login' };

  if (meError) {
    if (hasErrorCode(meError, 'ASSISTANT_UNAVAILABLE')) {
      return { kind: 'offline' };
    }
    // O 401 já apagou o token: o status refeito leva ao login
    if (hasErrorCode(meError, 'ASSISTANT_UNAUTHORIZED')) {
      return { kind: 'login' };
    }
    return {
      kind: 'error',
      message: meError instanceof Error ? meError.message : String(meError),
    };
  }
  if (!me) return { kind: 'loading' };
  if (me.consent.currentVersion === null) return { kind: 'no-term' };
  if (consentPending(me)) return { kind: 'consent' };
  return { kind: 'ready', me };
}

/** As chamadas autenticadas só fazem sentido depois destas portas */
export const canCallApi = (status: AssistantStatus | undefined) =>
  !!status?.configured &&
  !status.updateRequired &&
  status.online &&
  status.loggedIn;
