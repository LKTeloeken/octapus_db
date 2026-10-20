import { RustCommand } from '@/api/commands';
import type {
  AssistantConsentTerm,
  AssistantMe,
  AssistantModel,
  AssistantParticipant,
  AssistantStatus,
} from '@/api/types/assistant.types';
import type { MockHandler } from './handlers';
import { useMockStore } from './mock-store';

/**
 * API do assistente simulada. O modo do painel (`assistantMode`) decide se ela
 * existe e se está no ar; o login e o aceite seguem o fluxo real: entrar com
 * um convite deixa o termo pendente até `assistant_consent_accept`.
 *
 * Convites: qualquer código com 8+ caracteres entra; um que comece com
 * `REVOG` é recusado como revogado. Os erros saem no formato do Rust
 * ("Assistant unauthorized: …"), e o router tira o código deles.
 */

type Args = Record<string, unknown>;

interface MockAssistantSession {
  loggedIn: boolean;
  acceptedVersion: string | null;
}

const SESSION_STORAGE_KEY = 'octapus-mock-assistant';
const NO_SESSION: MockAssistantSession = {
  loggedIn: false,
  acceptedVersion: null,
};

const PARTICIPANT: AssistantParticipant = {
  id: '6f1c2b8e-4d3a-4e5f-9a7b-1c2d3e4f5a6b',
  label: 'P-07',
};

const TERM: AssistantConsentTerm = {
  version: '2026-10-v1',
  body: [
    '# Termo de consentimento (simulado)',
    '',
    'Este é o termo do **modo mock**. No app real ele vem da API do assistente.',
    '',
    '- A estrutura do banco (nomes de tabelas e colunas) é enviada ao modelo.',
    '- Os valores do seu banco nunca saem desta máquina.',
    '- Não cole dados do seu banco nos comentários das avaliações.',
  ].join('\n'),
  publishedAt: '2026-10-01T12:00:00Z',
};

export const ASSISTANT_MODELS: AssistantModel[] = [
  {
    id: '0b6f8f0e-1a2b-4c3d-8e9f-a1b2c3d4e5f6',
    alias: 'Modelo A',
    contextWindow: 200_000,
  },
  {
    id: '1c7a9a1f-2b3c-4d4e-9fa0-b2c3d4e5f6a7',
    alias: 'Modelo B',
    contextWindow: 128_000,
  },
];

const QUOTA = { limitTokens: 500_000, usedTokens: 42_310, reservedTokens: 0 };

/**
 * O token do app real fica cifrado no SQLite; aqui, o `localStorage` da origin
 * do mock (isolada do app). Fora do navegador (testes) fica só em memória.
 */
let memorySession: MockAssistantSession = NO_SESSION;

function readSession(): MockAssistantSession {
  try {
    const saved = globalThis.localStorage?.getItem(SESSION_STORAGE_KEY);
    return saved ? { ...NO_SESSION, ...JSON.parse(saved) } : memorySession;
  } catch {
    return memorySession;
  }
}

function writeSession(session: MockAssistantSession) {
  memorySession = session;
  try {
    globalThis.localStorage?.setItem(
      SESSION_STORAGE_KEY,
      JSON.stringify(session),
    );
  } catch {
    // sem localStorage: fica em memória
  }
}

/** Volta ao "nunca entrou" — o reset do painel */
export function resetAssistantSession() {
  memorySession = NO_SESSION;
  try {
    globalThis.localStorage?.removeItem(SESSION_STORAGE_KEY);
  } catch {
    // sem localStorage: nada a apagar
  }
}

/** Sem URL ou API fora: o que o `endpoint()` e o cliente do Rust rejeitariam */
function requireApi() {
  const { assistantMode } = useMockStore.getState();
  if (assistantMode === 'not-configured') throw 'Assistant not configured';
  if (assistantMode === 'offline') {
    throw 'Assistant unavailable: error sending request: connection refused (mock)';
  }
}

/** Chamada autenticada: sem login é `ASSISTANT_UNAUTHORIZED`, como no Rust */
export function requireAssistantLogin(): MockAssistantSession {
  requireApi();
  const session = readSession();
  if (!session.loggedIn) throw 'Assistant unauthorized: not signed in';
  return session;
}

export const assistantHandlers: Record<string, MockHandler> = {
  [RustCommand.AssistantStatus]: (): AssistantStatus => {
    const { assistantMode } = useMockStore.getState();
    if (assistantMode === 'not-configured') {
      return {
        configured: false,
        online: false,
        loggedIn: false,
        minAppVersion: null,
        updateRequired: false,
      };
    }
    return {
      configured: true,
      online: assistantMode === 'normal',
      loggedIn: readSession().loggedIn,
      minAppVersion: null,
      updateRequired: false,
    };
  },

  [RustCommand.AssistantRedeem]: ({
    inviteCode,
  }: Args): AssistantParticipant => {
    requireApi();
    const code = String(inviteCode ?? '').trim();
    if (code.length < 8) {
      throw 'Assistant invalid_request: inviteCode must have at least 8 characters';
    }
    if (code.toUpperCase().startsWith('REVOG')) {
      throw 'Assistant unauthorized: invite revoked';
    }
    // Novo token; o aceite é do participante e fica como estava
    writeSession({ ...readSession(), loggedIn: true });
    return PARTICIPANT;
  },

  // Só apaga o token local: funciona até sem API, como no Rust
  [RustCommand.AssistantLogout]: (): void => {
    writeSession({ ...readSession(), loggedIn: false });
  },

  [RustCommand.AssistantMe]: (): AssistantMe => {
    const session = requireAssistantLogin();
    return {
      participant: PARTICIPANT,
      modelSelection: 'free',
      consent: {
        currentVersion: TERM.version,
        acceptedVersion: session.acceptedVersion,
      },
      quota: QUOTA,
    };
  },

  [RustCommand.AssistantConsentCurrent]: (): AssistantConsentTerm => {
    requireAssistantLogin();
    return TERM;
  },

  [RustCommand.AssistantConsentAccept]: ({ version }: Args): void => {
    const session = requireAssistantLogin();
    if (version !== TERM.version) {
      throw 'Assistant conflict: consent version is not the current one';
    }
    writeSession({ ...session, acceptedVersion: TERM.version });
  },

  // A API não exige o termo aqui; só o modo `free` (o do participante mock)
  [RustCommand.AssistantModels]: (): AssistantModel[] => {
    requireAssistantLogin();
    return ASSISTANT_MODELS;
  },
};
