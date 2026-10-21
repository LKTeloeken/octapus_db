import { describe, expect, it, vi } from 'vitest';
import { startTurn } from '@/lib/assistant/turn-reducer';
import { useAssistantStore } from './assistant-store';
import { flushTabsSession, restoreTabsSession } from './tabs-session';
import { useTabsStore } from './tabs-store';

const session = vi.hoisted(() => ({
  loadSession: vi.fn(async (): Promise<string | null> => null),
  saveSession: vi.fn(async (_snapshot: string) => {}),
}));
vi.mock('@/api/session', () => session);

/** Pergunta, rascunho e linha do resultado do assistente */
const CANARY = 'cnry-b10-assistente';

describe('sessão das abas em disco (RN-01, critério 8)', () => {
  it('o snapshot leva as abas do editor e nada do assistente', async () => {
    await restoreTabsSession();

    const assistant = useAssistantStore.getState();
    assistant.openPanel({ serverId: 1, database: 'loja', schema: 'public' });
    const tabId = useAssistantStore.getState().activeTabId!;
    assistant.setConversation(tabId, 'c-1');
    assistant.setDraft(tabId, `rascunho ${CANARY}`);
    assistant.setLive(tabId, startTurn('ct-1', `quanto ${CANARY}?`, true));
    assistant.patchRun('a-1', {
      query: `SELECT '${CANARY}'`,
      result: {
        columns: [],
        rows: [[CANARY]],
        rowCount: 1,
        totalCount: null,
        hasMore: false,
        executionTimeMs: 1,
        editableInfo: null,
      },
    });

    useTabsStore.getState().openQueryTab({
      serverId: 1,
      database: 'loja',
      content: 'SELECT 1',
    });
    await flushTabsSession();

    expect(session.saveSession).toHaveBeenCalled();
    for (const [snapshot] of session.saveSession.mock.calls) {
      expect(snapshot).toContain('SELECT 1');
      expect(snapshot).not.toContain(CANARY);
      expect(snapshot).not.toContain('c-1');
    }
  });
});
