import { Channel } from '@tauri-apps/api/core';
import { call } from './client';
import { RustCommand } from './commands';
import type {
  AssistantConsentTerm,
  AssistantConversation,
  AssistantConversationPage,
  AssistantConversationSummary,
  AssistantFeedbackInput,
  AssistantMe,
  AssistantMention,
  AssistantModel,
  AssistantParticipant,
  AssistantStatus,
  AssistantTurnEvent,
  AssistantTurnFinish,
} from './types/assistant.types';
import type { QueryOptions, QueryResult } from './types/query.types';

/**
 * Configurado? API no ar? Há login? Não rejeita por causa da API: sem URL ou
 * com a API fora, o status diz isso (C-19).
 */
export function getAssistantStatus(): Promise<AssistantStatus> {
  return call<AssistantStatus>(RustCommand.AssistantStatus);
}

/** Troca o convite pelo token do participante (guardado cifrado no backend) */
export function redeemAssistantInvite(
  inviteCode: string,
  deviceLabel: string | null = null,
): Promise<AssistantParticipant> {
  return call<AssistantParticipant>(RustCommand.AssistantRedeem, {
    inviteCode,
    deviceLabel,
  });
}

/** Esquece o token nesta máquina */
export function logoutAssistant(): Promise<void> {
  return call<void>(RustCommand.AssistantLogout);
}

/** Participante, consentimento, cota e modo de seleção de modelo */
export function getAssistantMe(): Promise<AssistantMe> {
  return call<AssistantMe>(RustCommand.AssistantMe);
}

/** Termo de consentimento vigente (RN-11) */
export function getAssistantConsent(): Promise<AssistantConsentTerm> {
  return call<AssistantConsentTerm>(RustCommand.AssistantConsentCurrent);
}

/** Aceita o termo `version`; outro que não o vigente → `ASSISTANT_CONFLICT` */
export function acceptAssistantConsent(version: string): Promise<void> {
  return call<void>(RustCommand.AssistantConsentAccept, { version });
}

/** Modelos para escolher — só no modo `free` */
export function getAssistantModels(): Promise<AssistantModel[]> {
  return call<AssistantModel[]>(RustCommand.AssistantModels);
}

/** Conversas do participante, mais recentes primeiro (`cursor` = próxima página) */
export function listAssistantConversations(
  cursor: string | null = null,
): Promise<AssistantConversationPage> {
  return call<AssistantConversationPage>(RustCommand.AssistantConversations, {
    cursor,
  });
}

/** A conversa inteira, com turnos, tentativas e avaliações */
export function getAssistantConversation(
  conversationId: string,
): Promise<AssistantConversation> {
  return call<AssistantConversation>(RustCommand.AssistantConversation, {
    conversationId,
  });
}

/**
 * Abre uma conversa para o tipo de banco do servidor. `modelId` só no modo
 * `free`; nos outros a API decide o modelo.
 */
export function createAssistantConversation(
  serverId: number,
  modelId: string | null = null,
  title: string | null = null,
): Promise<AssistantConversationSummary> {
  return call<AssistantConversationSummary>(
    RustCommand.AssistantConversationCreate,
    { serverId, modelId, title },
  );
}

export interface AskAssistantInput {
  serverId: number;
  database: string;
  conversationId: string;
  question: string;
  /** UUID gerado na tela: retentativa segura e alvo do cancelamento */
  clientTurnId: string;
  /** Schemas da conversa (Postgres); vazio = `public` */
  schemas?: string[];
  /** Citadas com `@` na pergunta: entram obrigatoriamente no contexto */
  mentions?: AssistantMention[];
  /** Citadas nas perguntas anteriores: só ganham prioridade */
  pastMentions?: AssistantMention[];
}

/**
 * Faz a pergunta: o turno inteiro roda no backend e o andamento chega por
 * `onEvent` (tentativa, resposta, plano, resultado, falhas).
 */
export function askAssistant(
  input: AskAssistantInput,
  onEvent: (event: AssistantTurnEvent) => void,
): Promise<AssistantTurnFinish> {
  const channel = new Channel<AssistantTurnEvent>();
  channel.onmessage = onEvent;
  return call<AssistantTurnFinish>(RustCommand.AssistantTurnAsk, {
    ...input,
    schemas: input.schemas ?? null,
    mentions: input.mentions ?? null,
    pastMentions: input.pastMentions ?? null,
    onEvent: channel,
  });
}

/** Cancela o turno em andamento; `false` se ele já terminou */
export function cancelAssistantTurn(clientTurnId: string): Promise<boolean> {
  return call<boolean>(RustCommand.AssistantTurnCancel, { clientTurnId });
}

export interface ExecuteAssistantQueryInput {
  serverId: number;
  database: string;
  query: string;
  page: QueryOptions;
  /** Com a tentativa, a primeira página completa o outcome dela na API */
  attemptId?: string | null;
  userEdited?: boolean;
}

/** Execução manual (revisão e páginas seguintes), sempre só-leitura */
export function executeAssistantQuery(
  input: ExecuteAssistantQueryInput,
): Promise<QueryResult> {
  return call<QueryResult>(RustCommand.AssistantExecute, {
    ...input,
    attemptId: input.attemptId ?? null,
    userEdited: input.userEdited ?? false,
  });
}

/**
 * Avalia a resposta do turno (👍/👎, motivos só no 👎, comentário até 2000
 * caracteres); repetir sobrescreve
 */
export function setAssistantFeedback(
  turnId: string,
  feedback: AssistantFeedbackInput,
): Promise<void> {
  return call<void>(RustCommand.AssistantFeedbackSet, { turnId, feedback });
}

/** Remove a avaliação do turno (sem avaliação não é erro) */
export function clearAssistantFeedback(turnId: string): Promise<void> {
  return call<void>(RustCommand.AssistantFeedbackClear, { turnId });
}
