import { describe, expect, it } from 'vitest';
import type { ImportCandidate } from '@/api/types/import.types';
import {
  candidateTarget,
  redactUri,
  selectedByDefault,
  sourceSummary,
  warningLabel,
} from './import-labels';

const candidate = (
  overrides: Partial<ImportCandidate> = {},
): ImportCandidate => ({
  key: 0,
  name: 'Loja',
  dbType: 'postgres',
  driver: 'PostgreSQL',
  host: 'db.loja.com',
  port: 5432,
  username: 'app',
  defaultDatabase: 'loja',
  connectionUri: null,
  sslMode: 'disable',
  group: null,
  hasPassword: true,
  passwordInKeychain: false,
  duplicateOf: null,
  warnings: [],
  ...overrides,
});

describe('import-labels', () => {
  it('resume a origem', () => {
    const base = { source: 'dbeaver' as const, fileExtensions: ['json'] };
    expect(
      sourceSummary({ ...base, path: null, count: 0, passwords: 'file' }),
    ).toBe('Não encontrado nesta máquina');
    expect(
      sourceSummary({ ...base, path: '/x', count: 0, passwords: 'file' }),
    ).toBe('Nenhuma conexão salva');
    expect(
      sourceSummary({ ...base, path: '/x', count: 1, passwords: 'keychain' }),
    ).toBe('1 conexão · senhas no Keychain');
    expect(
      sourceSummary({ ...base, path: '/x', count: 3, passwords: 'none' }),
    ).toBe('3 conexões · sem senhas');
  });

  it('o alvo nunca mostra a senha da URI', () => {
    expect(candidateTarget(candidate())).toBe('app@db.loja.com:5432/loja');
    expect(redactUri('mongodb+srv://app:s3cr3t@cluster0.x.net/db')).toBe(
      'mongodb+srv://app:•••@cluster0.x.net/db',
    );
    expect(
      candidateTarget(candidate({ connectionUri: 'redis://:pw@cache:6379' })),
    ).not.toContain('pw');
    expect(
      candidateTarget(
        candidate({ dbType: 'sqlite', connectionUri: '/x/app.db' }),
      ),
    ).toBe('/x/app.db');
  });

  it('SSH avisa sem bloquear; banco sem suporte e duplicado começam desmarcados', () => {
    const ssh = warningLabel({ kind: 'sshTunnel', host: 'bastion' });
    expect(ssh.label).toBe('SSH');
    expect(ssh.title).toContain('bastion');
    expect(
      selectedByDefault(
        candidate({ warnings: [{ kind: 'sshTunnel', host: 'b' }] }),
      ),
    ).toBe(true);
    expect(selectedByDefault(candidate({ dbType: null }))).toBe(false);
    expect(selectedByDefault(candidate({ duplicateOf: 3 }))).toBe(false);
    expect(
      warningLabel({ kind: 'unsupportedType', driver: 'MySQL' }).tone,
    ).toBe('danger');
  });
});
