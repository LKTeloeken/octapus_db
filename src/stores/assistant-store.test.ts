import { beforeEach, describe, expect, it } from 'vitest';
import { startTurn } from '@/lib/assistant/turn-reducer';
import { useAssistantStore } from './assistant-store';

const target = { serverId: 1, database: 'loja', schema: 'public' };
const state = () => useAssistantStore.getState();

describe('assistant-store', () => {
  beforeEach(() => {
    useAssistantStore.setState(useAssistantStore.getInitialState(), true);
  });

  it('abrir o painel sem abas cria uma conversa com o alvo', () => {
    state().openPanel(target);
    expect(state().panelOpen).toBe(true);
    expect(state().tabs).toHaveLength(1);
    expect(state().tabs[0].target).toEqual(target);
    expect(state().activeTabId).toBe(state().tabs[0].id);
  });

  it('aba em branco troca de alvo; aba que já perguntou ganha irmã', () => {
    state().openPanel(target);
    const first = state().tabs[0].id;
    const other = { serverId: 2, database: 'app', schema: null };

    state().openPanel(other);
    expect(state().tabs).toHaveLength(1);
    expect(state().tabs[0].target).toEqual(other);

    state().setLive(first, startTurn('c1', 'quantos?', false));
    state().openPanel(target);
    expect(state().tabs).toHaveLength(2);
    expect(state().activeTabId).not.toBe(first);
  });

  it('nova aba herda o alvo; fechar ativa a vizinha e esquece o turno', () => {
    state().openPanel(target);
    const first = state().tabs[0].id;
    const second = state().newTab();
    expect(state().tabs[1].target).toEqual(target);
    state().setLive(second, startTurn('c2', 'x', false));

    state().closeTab(second);
    expect(state().activeTabId).toBe(first);
    expect(state().live[second]).toBeUndefined();

    state().closeTab(first);
    expect(state().tabs).toEqual([]);
    expect(state().activeTabId).toBeNull();
  });

  it('reabrir uma conversa já aberta só a ativa', () => {
    state().openConversation('conv-1', target);
    const id = state().activeTabId;
    state().newTab();
    state().openConversation('conv-1', target);
    expect(state().activeTabId).toBe(id);
    expect(state().tabs).toHaveLength(2);
  });

  it('modo agente abre o painel com o alvo; fechar o painel sai dele', () => {
    state().setAgentMode(true, target);
    expect(state().panelOpen).toBe(true);
    expect(state().agentMode).toBe(true);
    expect(state().tabs[0].target).toEqual(target);

    state().setAgentMode(false);
    expect(state().panelOpen).toBe(true);
    expect(state().agentMode).toBe(false);

    state().setAgentMode(true);
    state().closePanel();
    expect(state().agentMode).toBe(false);
  });

  it('resultados por tentativa somam os patches', () => {
    state().patchRun('a1', { executing: true });
    state().patchRun('a1', { executing: false, query: 'SELECT 1' });
    expect(state().runs.a1).toMatchObject({
      executing: false,
      query: 'SELECT 1',
      result: null,
    });
  });
});
