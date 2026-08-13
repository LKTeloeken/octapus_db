import { invoke } from '@tauri-apps/api/core';
import type { RustCommand } from './commands';

/** Backend rejections are plain strings — normalized here to Error */
export class ApiError extends Error {
  constructor(
    message: string,
    public readonly command: RustCommand,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

export async function call<T>(
  command: RustCommand,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new ApiError(
      typeof error === 'string'
        ? error
        : error instanceof Error
          ? error.message
          : 'Unknown backend error',
      command,
    );
  }
}
