import { call } from './client';
import { RustCommand } from './commands';
import type {
  ImportCommitRequest,
  ImportPreview,
  ImportPreviewRequest,
  ImportResult,
  ImportSourceInfo,
} from './types/import.types';

/** Apps desta máquina com conexões para importar */
export function detectImportSources(): Promise<ImportSourceInfo[]> {
  return call<ImportSourceInfo[]>(RustCommand.ImportDetectSources);
}

/** Lê uma origem; as senhas ficam no backend, presas ao token da prévia */
export function previewImport({
  source,
  path = null,
  text = null,
}: ImportPreviewRequest): Promise<ImportPreview> {
  return call<ImportPreview>(RustCommand.ImportPreview, { source, path, text });
}

export function commitImport(
  request: ImportCommitRequest,
): Promise<ImportResult> {
  return call<ImportResult>(RustCommand.ImportCommit, { request });
}

/** Diálogo fechado sem importar: as senhas lidas saem da memória do backend */
export function discardImport(): Promise<void> {
  return call<void>(RustCommand.ImportDiscard);
}
