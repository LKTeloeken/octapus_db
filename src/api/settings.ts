import { call } from './client';
import { RustCommand } from './commands';
import type { AppSettings } from './types/settings.types';

/** Preferências salvas (padrões para o que nunca foi gravado) */
export function getSettings(): Promise<AppSettings> {
  return call<AppSettings>(RustCommand.GetSettings);
}

/** Substitui as preferências e devolve o que ficou gravado */
export function updateSettings(settings: AppSettings): Promise<AppSettings> {
  return call<AppSettings>(RustCommand.UpdateSettings, { settings });
}
