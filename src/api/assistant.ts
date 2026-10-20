import { call } from './client';
import { RustCommand } from './commands';
import type {
  AssistantConsentTerm,
  AssistantMe,
  AssistantModel,
  AssistantParticipant,
  AssistantStatus,
} from './types/assistant.types';

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
