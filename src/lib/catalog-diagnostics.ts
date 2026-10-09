import type { CatalogDiagnostics } from '@/api/types/catalog.types';
import type { DatabaseType } from '@/api/types/server.types';
import { DB_TYPE_LABELS } from '@/lib/db-defaults';
import { formatBytes } from '@/lib/format-bytes';
import { formatCount } from '@/lib/format-count';

const ms = (value: number) =>
  value >= 1000
    ? `${(value / 1000).toLocaleString('pt-BR', { maximumFractionDigits: 2 })} s`
    : `${Math.round(value)} ms`;

const when = (epochMs: number) => new Date(epochMs).toISOString();

const plural = (count: number, one: string, many: string) =>
  `${formatCount(count)} ${count === 1 ? one : many}`;

/**
 * Texto do "Copiar diagnóstico": o que ajuda a entender um catálogo lento ou
 * estranho, pronto para colar numa issue. Só contagens, tempos e a versão do
 * banco — o backend já não manda nomes de schema, tabela, host nem usuário, e
 * o nome do database fica de fora aqui.
 */
export function formatCatalogDiagnostics(
  diagnostics: CatalogDiagnostics,
  dbType: DatabaseType,
): string {
  const { status, lastSync, drift } = diagnostics;
  const { stats } = status;

  const state = status.error
    ? `erro: ${status.error}`
    : status.syncing
      ? 'sincronizando'
      : 'pronto';

  const lines = [
    `Octapus ${diagnostics.appVersion} · diagnóstico do catálogo`,
    `Banco: ${DB_TYPE_LABELS[dbType]}${status.serverVersion ? ` (${status.serverVersion})` : ''}`,
    `Estado: ${state}${status.fromDisk ? ' · conteúdo do disco, ainda não revalidado' : ''}`,
    `Última revalidação: ${status.fetchedAt ? when(status.fetchedAt) : 'nunca'}`,
    `Schemas: ${formatCount(stats.schemas)} (carregados ${formatCount(stats.loaded)}, desatualizados ${formatCount(stats.stale)}, pendentes ${formatCount(stats.unloaded)})`,
    `Formatos: ${formatCount(stats.shapes)} · nomes distintos: ${formatCount(stats.distinctNames)} · relações: ${formatCount(stats.relations)} · memória: ~${formatBytes(stats.approxHeapBytes)}`,
  ];

  if (lastSync) {
    const strategy =
      lastSync.strategy === 'shapeFirst'
        ? `por formato (${formatCount(lastSync.shapes ?? 0)} no servidor)`
        : 'em massa';
    lines.push(
      `Última sincronização (${when(lastSync.at)}): ${strategy}; ${formatCount(lastSync.fetched)} schemas lidos, ${formatCount(lastSync.shared)} copiados do formato; camada 0 em ${ms(lastSync.layer0Ms)}, total ${ms(lastSync.totalMs)}; +${formatCount(lastSync.added)} −${formatCount(lastSync.removed)} ~${formatCount(lastSync.changed)}`,
    );
  } else {
    lines.push('Última sincronização: nenhuma nesta sessão');
  }

  lines.push(
    drift.dominantTables === null
      ? 'Drift: sem formato repetido'
      : `Drift: formato principal com ${formatCount(drift.dominantTables)} relações em ${formatCount(drift.dominantSchemas)} schemas; ${plural(drift.divergentGroups, 'variação', 'variações')} em ${formatCount(drift.divergentSchemas)} schemas`,
  );

  return lines.join('\n');
}
