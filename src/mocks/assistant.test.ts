import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acceptAssistantConsent,
  getAssistantConsent,
  getAssistantMe,
  getAssistantModels,
  getAssistantStatus,
  logoutAssistant,
  redeemAssistantInvite,
} from '@/api/assistant';
import { ApiError } from '@/api/client';
import { consentPending, quotaRemaining } from '@/queries/use-assistant-me';
import { resetAssistantSession } from './assistant';
import { useMockStore } from './mock-store';
import { route } from './router';

// O `invoke` do app passa pelo router do mock, como no `pnpm dev:mock`
vi.mock('@tauri-apps/api/core', async original => ({
  ...(await original<typeof import('@tauri-apps/api/core')>()),
  invoke: (command: string, args?: Record<string, unknown>) =>
    route(command, args),
}));

async function codeOf(promise: Promise<unknown>): Promise<string> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ApiError) return error.code;
  }
  throw new Error('esperava um ApiError');
}

describe('assistente no mock', () => {
  beforeEach(() => {
    resetAssistantSession();
    useMockStore.setState({
      latencyMs: 0,
      failMode: 'off',
      assistantMode: 'normal',
    });
  });

  it('convite → consentimento → /me', async () => {
    expect(await getAssistantStatus()).toMatchObject({
      configured: true,
      online: true,
      loggedIn: false,
    });
    expect(await codeOf(getAssistantMe())).toBe('ASSISTANT_UNAUTHORIZED');

    const participant = await redeemAssistantInvite('CONVITE-TCC-07');
    expect(participant.label).toBe('P-07');
    expect((await getAssistantStatus()).loggedIn).toBe(true);

    // Logado, mas o termo ainda não foi aceito
    const before = await getAssistantMe();
    expect(consentPending(before)).toBe(true);

    const term = await getAssistantConsent();
    expect(await codeOf(acceptAssistantConsent('versao-velha'))).toBe(
      'ASSISTANT_CONFLICT',
    );
    await acceptAssistantConsent(term.version);

    const me = await getAssistantMe();
    expect(consentPending(me)).toBe(false);
    expect(me.consent.acceptedVersion).toBe(term.version);
    expect(me.modelSelection).toBe('free');
    expect(quotaRemaining(me)).toBe(
      me.quota.limitTokens - me.quota.usedTokens - me.quota.reservedTokens,
    );
    expect((await getAssistantModels()).length).toBeGreaterThan(0);
  });

  it('convite curto ou revogado é recusado', async () => {
    expect(await codeOf(redeemAssistantInvite('curto'))).toBe(
      'ASSISTANT_INVALID_REQUEST',
    );
    expect(await codeOf(redeemAssistantInvite('REVOGADO-01'))).toBe(
      'ASSISTANT_UNAUTHORIZED',
    );
    expect((await getAssistantStatus()).loggedIn).toBe(false);
  });

  it('sair esquece o login, mesmo sem a API', async () => {
    await redeemAssistantInvite('CONVITE-TCC-07');
    useMockStore.setState({ assistantMode: 'offline' });
    await logoutAssistant();
    useMockStore.setState({ assistantMode: 'normal' });
    expect((await getAssistantStatus()).loggedIn).toBe(false);
  });

  it('sem URL: status desligado e comandos ASSISTANT_NOT_CONFIGURED', async () => {
    useMockStore.setState({ assistantMode: 'not-configured' });
    expect(await getAssistantStatus()).toMatchObject({
      configured: false,
      online: false,
    });
    expect(await codeOf(redeemAssistantInvite('CONVITE-TCC-07'))).toBe(
      'ASSISTANT_NOT_CONFIGURED',
    );
  });

  it('API fora: status offline e comandos ASSISTANT_UNAVAILABLE', async () => {
    useMockStore.setState({ assistantMode: 'offline' });
    expect(await getAssistantStatus()).toMatchObject({
      configured: true,
      online: false,
    });
    expect(await codeOf(getAssistantMe())).toBe('ASSISTANT_UNAVAILABLE');
  });
});
