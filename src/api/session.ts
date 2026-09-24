import { call } from './client';
import { RustCommand } from './commands';

/**
 * Snapshot das abas salvo na última sessão, ou `null` se nunca houve. O
 * backend guarda o texto como veio — o formato é definido (e versionado) em
 * `stores/tabs-session.ts`.
 */
export function loadSession(): Promise<string | null> {
  return call<string | null>(RustCommand.LoadSession);
}

/** Substitui o snapshot salvo; resolve só depois de gravado em disco. */
export function saveSession(snapshot: string): Promise<void> {
  return call<void>(RustCommand.SaveSession, { snapshot });
}
