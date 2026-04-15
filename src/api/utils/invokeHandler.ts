import type { RustMethods } from '../rust-functions';
import { invoke as tauriInvoker } from '@tauri-apps/api/core';
import { convertToCamelCase } from './logic';
import { invokeMock } from '../mocks/invoke-mock';

export async function invoke<T>(
  event: RustMethods,
  payload: Record<string, any>,
): Promise<T> {
  const formatedPayload: Record<string, any> = convertToCamelCase(payload);
  const useMockApi =
    (import.meta as { env?: Record<string, string> }).env?.VITE_USE_MOCK_API ===
    'true';
  // Set VITE_USE_MOCK_API=true to run the frontend without Tauri/Rust.
  if (useMockApi) {
    return invokeMock<T>(event, formatedPayload);
  }

  const response = await tauriInvoker<T>(event, { ...formatedPayload });
  return response as T;
}
