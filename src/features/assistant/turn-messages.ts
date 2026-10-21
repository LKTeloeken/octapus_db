import type { TurnPhase } from '@/lib/assistant/turn-reducer';
import type { FailureStage } from './turn-view';

/** Rótulo ao lado do polvo, por fase do turno */
export const PHASE_LABELS: Record<TurnPhase, string> = {
  opening: 'Lendo a estrutura do banco…',
  generating: 'Pensando…',
  checking: 'Conferindo a consulta…',
  executing: 'Executando a consulta…',
  waiting: 'Aguardando a vez…',
  finished: '',
};

/** Durante o stream, "pensando" vira "escrevendo"; numa correção, "corrigindo" */
export function phaseLabel(
  phase: TurnPhase,
  streaming: boolean,
  correcting: boolean,
): string {
  if (phase === 'generating') {
    if (correcting) return 'Corrigindo a consulta…';
    if (streaming) return 'Escrevendo…';
  }
  return PHASE_LABELS[phase];
}

export const STAGE_LABELS: Record<FailureStage, string> = {
  guard: 'Recusada pela verificação de leitura',
  preflight: 'Falhou na conferência (EXPLAIN)',
  execution: 'Falhou ao executar',
  output: 'Sem resposta válida',
};

/**
 * Por que o turno parou sem resposta, pelo `errorCode` do fim do turno ou da
 * rejeição do comando. Explícito sempre (RN-21): nunca uma tela vazia.
 */
export function turnErrorMessage(
  code: string | null,
  message: string | null,
): string {
  switch (code) {
    case 'ASSISTANT_QUOTA_EXCEEDED':
      return 'Sua cota de uso do assistente acabou. O resto do Octapus continua funcionando.';
    case 'ASSISTANT_RATE_LIMITED':
      return 'Muitas perguntas seguidas. Espere um pouco e tente de novo.';
    case 'ASSISTANT_MODEL_UNAVAILABLE':
      return 'O modelo desta conversa está indisponível agora. Tente de novo mais tarde ou abra uma conversa nova.';
    case 'ASSISTANT_PROVIDER_ERROR':
      return 'O provedor do modelo falhou ao responder. Tente de novo.';
    case 'ASSISTANT_MODEL_REFUSED':
      return 'O modelo se recusou a responder esta pergunta.';
    case 'ASSISTANT_TURN_LIMIT_EXCEEDED':
      return 'A pergunta passou do limite de tempo ou de consumo de um turno.';
    case 'ASSISTANT_UNAVAILABLE':
      return 'Sem conexão com o assistente. O resto do Octapus continua funcionando.';
    case 'ASSISTANT_CONSENT_REQUIRED':
      return 'O termo de consentimento mudou. Aceite a versão nova para continuar.';
    case 'ASSISTANT_PROMPT_NOT_REGISTERED':
      return 'Esta versão do app usa um prompt que o servidor não conhece. Atualize o Octapus.';
    case 'ASSISTANT_UNAUTHORIZED':
      return 'Seu acesso ao assistente expirou. Entre de novo com o convite.';
    case 'CONNECTION_ERROR':
    case 'CONNECTION_TIMEOUT':
    case 'POOL_EXHAUSTED':
      return 'Não consegui falar com o banco de dados para conferir a consulta.';
    case 'ASSISTANT_INVALID_OUTPUT':
      return 'O modelo respondeu fora do formato esperado.';
    case 'CANCELLED':
      return 'Cancelado.';
    default:
      return message ?? 'O assistente não conseguiu responder.';
  }
}

/** Por que a avaliação não foi gravada (a tela já voltou ao estado anterior) */
export function feedbackErrorMessage(code: string | null): string {
  switch (code) {
    case 'ASSISTANT_UNAVAILABLE':
    case 'ASSISTANT_CONSENT_REQUIRED':
    case 'ASSISTANT_UNAUTHORIZED':
      return `A avaliação não foi salva. ${turnErrorMessage(code, null)}`;
    case 'ASSISTANT_CONFLICT':
      return 'A avaliação não foi salva: esta resposta ainda não pode ser avaliada.';
    default:
      return 'A avaliação não foi salva: o servidor recusou. Tente de novo.';
  }
}
