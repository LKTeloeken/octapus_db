import { invoke } from '@tauri-apps/api/core';
import type { RustCommand } from './commands';
import type { BackendError, BackendErrorCode } from './types/error.types';

/**
 * `UNKNOWN`: rejeição que não veio do `Error` do app — argumentos que o Tauri
 * não conseguiu desserializar, comando inexistente, falha de plugin.
 */
export type ApiErrorCode = BackendErrorCode | 'UNKNOWN';

/** Rejeição do backend (`{ code, message }`) normalizada em Error */
export class ApiError extends Error {
  constructor(
    message: string,
    public readonly command: RustCommand,
    public readonly code: ApiErrorCode = 'UNKNOWN',
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

function isBackendError(error: unknown): error is BackendError {
  return (
    typeof error === 'object' &&
    error !== null &&
    typeof (error as BackendError).code === 'string' &&
    typeof (error as BackendError).message === 'string'
  );
}

function toApiError(error: unknown, command: RustCommand): ApiError {
  if (isBackendError(error)) {
    return new ApiError(error.message, command, error.code);
  }
  const message =
    typeof error === 'string'
      ? error
      : error instanceof Error
        ? error.message
        : 'Unknown backend error';
  return new ApiError(message, command);
}

/** O erro é um `ApiError` com este código */
export function hasErrorCode(error: unknown, code: ApiErrorCode): boolean {
  return error instanceof ApiError && error.code === code;
}

export async function call<T>(
  command: RustCommand,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toApiError(error, command);
  }
}
