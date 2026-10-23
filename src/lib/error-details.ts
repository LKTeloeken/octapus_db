import { ApiError, type ApiErrorCode } from '@/api/client';

/** O que a tela mostra de uma falha: um título curto e a mensagem inteira */
export interface ErrorDetails {
  code: ApiErrorCode;
  title: string;
  message: string;
}

/**
 * Título pelo código do erro (nunca pelo texto, que vem do driver e muda de
 * banco para banco). O motivo concreto — recusado, senha, TLS — está na
 * mensagem, mostrada inteira logo abaixo.
 */
const TITLES: Partial<Record<ApiErrorCode, string>> = {
  CONNECTION_ERROR: 'Não foi possível conectar',
  CONNECTION_TIMEOUT: 'O servidor não respondeu a tempo',
  POOL_EXHAUSTED: 'Sem conexões livres no momento',
  QUERY_ERROR: 'O banco recusou a operação',
  INVALID_QUERY: 'Comando inválido',
  NOT_FOUND: 'Não encontrado',
  STORAGE_ERROR: 'Falha nos dados locais do app',
  UNSUPPORTED_DATABASE: 'Banco não suportado',
  UNSUPPORTED_TYPE: 'Operação não suportada neste banco',
};

/** O código de qualquer coisa que veio de um `catch` */
export const errorCode = (error: unknown): ApiErrorCode =>
  error instanceof ApiError ? error.code : 'UNKNOWN';

export function errorDetails(error: unknown): ErrorDetails {
  const code = errorCode(error);
  const message =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : String(error);
  return {
    code,
    title: TITLES[code] ?? 'Algo deu errado',
    message,
  };
}
