import { check, Update } from '@tauri-apps/plugin-updater';
import { call } from './client';
import { RustCommand } from './commands';

/** O que o `check_beta_update` devolve — o mesmo do `check` do plugin */
type UpdateMetadata = ConstructorParameters<typeof Update>[0];

/**
 * Atualização disponível, ou `null`. O canal estável usa o endpoint fixo do
 * `tauri.conf.json` (`releases/latest`, que nunca é pre-release); o beta pede
 * ao backend a versão mais alta entre releases e pre-releases. O `Update`
 * devolvido é o do plugin nos dois casos — o download segue igual.
 */
export async function checkForUpdate(beta: boolean): Promise<Update | null> {
  if (!beta) return check();

  const metadata = await call<UpdateMetadata | null>(
    RustCommand.CheckBetaUpdate,
  );
  return metadata ? new Update(metadata) : null;
}
