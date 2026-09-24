import toast from 'react-hot-toast';
import { loadSession, saveSession } from '@/api/session';
import type { SortSpec } from '@/api/types/browse.types';
import { encodeNodeId } from '@/lib/node-ref';
import { useTabsStore, type WorkspaceTab } from './tabs-store';

/**
 * Sessão do workspace: as abas abertas sobrevivem ao fechamento do app — em
 * especial ao restart do update, quando o usuário não escolheu fechar nada.
 *
 * O snapshot guarda só a intenção de cada aba (query escrita, ordenação,
 * filtro, colunas ocultas). Resultados e edições pendentes do grid não entram:
 * são dados do banco e voltam ao reabrir.
 *
 * Mora no SQLite do backend, não no `localStorage`: o comando só responde depois
 * de gravar em disco, então dá para salvar e reiniciar logo em seguida sem
 * depender do flush assíncrono do WebView (nem do teto de quota dele).
 */

const SNAPSHOT_VERSION = 1;

/**
 * Throttle das gravações: digitando sem parar, grava no máximo uma vez por
 * janela — e nunca mais que uma janela depois da última mudança. Fechar o app
 * no meio dela perde no máximo esse intervalo; o update força a gravação antes.
 */
const SAVE_INTERVAL_MS = 500;

interface SessionSnapshot {
  version: number;
  activeTabId: string | null;
  tabs: WorkspaceTab[];
}

let lastSaved: string | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
/** Fila das gravações: uma resposta atrasada nunca sobrescreve uma mais nova. */
let queue: Promise<void> = Promise.resolve();
let syncing = false;

function serialize(
  tabs: Map<string, WorkspaceTab>,
  activeTabId: string | null,
): string {
  const snapshot: SessionSnapshot = {
    version: SNAPSHOT_VERSION,
    activeTabId,
    tabs: Array.from(tabs.values()),
  };
  return JSON.stringify(snapshot);
}

// ── Leitura defensiva ───────────────────────────────────────────────────────
// O snapshot vem do disco: pode ser de uma versão antiga do app ou estar
// corrompido. Campo inválido cai no default; aba sem identidade é descartada.

type RawObject = Record<string, unknown>;

const isObject = (value: unknown): value is RawObject =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const asString = (value: unknown, fallback: string) =>
  typeof value === 'string' ? value : fallback;

const asNullableString = (value: unknown) =>
  typeof value === 'string' ? value : null;

const asStringArray = (value: unknown) =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];

function parseSort(value: unknown): SortSpec[] {
  if (!Array.isArray(value)) return [];

  return value.flatMap(item =>
    isObject(item) &&
    typeof item.column === 'string' &&
    (item.direction === 'asc' || item.direction === 'desc')
      ? [{ column: item.column, direction: item.direction }]
      : [],
  );
}

function parseTab(raw: unknown): WorkspaceTab | null {
  if (!isObject(raw)) return null;

  const { serverId, database } = raw;
  if (!Number.isInteger(serverId) || typeof database !== 'string') return null;
  const base = { serverId: serverId as number, database };

  if (raw.kind === 'query' && typeof raw.id === 'string') {
    return {
      ...base,
      id: raw.id,
      kind: 'query',
      title: asString(raw.title, database),
      schema: asNullableString(raw.schema),
      content: asString(raw.content, ''),
      hiddenColumns: asStringArray(raw.hiddenColumns),
    };
  }

  if (raw.kind === 'browse' && typeof raw.table === 'string') {
    const schema = asNullableString(raw.schema);
    return {
      ...base,
      // O id do browse é derivado da tabela (uma aba por tabela): recalcular
      // mantém o `openBrowseTab` reencontrando a aba restaurada.
      id: encodeNodeId({
        ...base,
        schema: schema ?? undefined,
        table: raw.table,
      }),
      kind: 'browse',
      title: asString(raw.title, raw.table),
      schema,
      table: raw.table,
      whereExpr: asString(raw.whereExpr, ''),
      sort: parseSort(raw.sort),
      hiddenColumns: asStringArray(raw.hiddenColumns),
    };
  }

  return null;
}

function parseSnapshot(json: string): {
  tabs: Map<string, WorkspaceTab>;
  activeTabId: string | null;
} | null {
  const raw: unknown = JSON.parse(json);
  if (!isObject(raw) || raw.version !== SNAPSHOT_VERSION) return null;
  if (!Array.isArray(raw.tabs)) return null;

  const tabs = new Map<string, WorkspaceTab>();
  let activeTabId: string | null = null;

  for (const rawTab of raw.tabs) {
    const tab = parseTab(rawTab);
    if (!tab || tabs.has(tab.id)) continue;

    tabs.set(tab.id, tab);
    if (isObject(rawTab) && rawTab.id === raw.activeTabId) activeTabId = tab.id;
  }

  return {
    tabs,
    activeTabId: activeTabId ?? tabs.keys().next().value ?? null,
  };
}

// ── Gravação ────────────────────────────────────────────────────────────────

function persist(): Promise<void> {
  // Antes da restauração o store ainda está vazio: gravar agora apagaria a
  // sessão salva em disco.
  if (!syncing) return queue;

  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }

  const { tabs, activeTabId } = useTabsStore.getState();
  const snapshot = serialize(tabs, activeTabId);
  if (snapshot === lastSaved) return queue;
  lastSaved = snapshot;

  queue = queue
    .then(() => saveSession(snapshot))
    .catch(error => {
      // Permite regravar na próxima mudança, mesmo que o conteúdo seja igual.
      lastSaved = null;
      console.error('Falha ao salvar a sessão das abas:', error);
      // Id fixo: uma falha repetida (disco cheio) não empilha um toast por tecla.
      toast.error('Não foi possível salvar as abas abertas', {
        id: 'tabs-session-save',
      });
    });

  return queue;
}

function startSync(): void {
  if (syncing) return;
  syncing = true;

  useTabsStore.subscribe((state, prev) => {
    if (state.tabs === prev.tabs && state.activeTabId === prev.activeTabId) {
      return;
    }
    timer ??= setTimeout(() => void persist(), SAVE_INTERVAL_MS);
  });
}

/**
 * Reabre as abas da sessão anterior e passa a gravar cada mudança. Chamado uma
 * vez no bootstrap, antes do primeiro render — assim a tela não pisca vazia e
 * nenhuma aba aberta cedo demais disputa com a restauração.
 *
 * Nunca rejeita: uma sessão ilegível não pode impedir o app de abrir.
 */
export async function restoreTabsSession(): Promise<void> {
  try {
    const json = await loadSession();
    const restored = json ? parseSnapshot(json) : null;
    if (restored && restored.tabs.size > 0) useTabsStore.setState(restored);
  } catch (error) {
    console.error('Falha ao restaurar a sessão das abas:', error);
  }

  const { tabs, activeTabId } = useTabsStore.getState();
  lastSaved = serialize(tabs, activeTabId);
  startSync();
}

/**
 * Grava agora o que estiver pendente no throttle e espera o disco confirmar.
 * Use antes de o app sair por conta própria (instalar/reiniciar o update).
 * Nunca rejeita — a falha já é avisada pelo toast.
 */
export function flushTabsSession(): Promise<void> {
  return persist();
}
