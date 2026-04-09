import type { RustMethods } from '../rust-functions';
import { invoke as tauriInvoker } from '@tauri-apps/api/core';
import { convertToCamelCase } from './logic';

export async function invoke<T>(
  event: RustMethods,
  payload: Record<string, any>,
): Promise<T> {
  const formatedPayload: Record<string, any> = convertToCamelCase(payload);

  const response = await tauriInvoker<T>(event, { ...formatedPayload });
  return response as T;
}
