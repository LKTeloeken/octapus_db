import type { DatabaseType, SslMode } from '@/api/types/server.types';

export interface SslModeOption {
  value: SslMode;
  label: string;
  description: string;
}

const OPTIONS: Record<SslMode, SslModeOption> = {
  disable: {
    value: 'disable',
    label: 'Desativado',
    description: 'Conexão sem criptografia.',
  },
  prefer: {
    value: 'prefer',
    label: 'Preferir',
    description:
      'Usa TLS se o servidor aceitar; senão conecta sem. Não verifica o certificado.',
  },
  require: {
    value: 'require',
    label: 'Exigir, sem verificar',
    description:
      'Sempre cifrado, mas aceita qualquer certificado (servidor autoassinado).',
  },
  'verify-ca': {
    value: 'verify-ca',
    label: 'Verificar a CA',
    description:
      'O certificado precisa vir de uma CA confiável; o nome do host não é conferido.',
  },
  'verify-full': {
    value: 'verify-full',
    label: 'Verificar tudo',
    description: 'CA confiável e nome do host no certificado — o mais seguro.',
  },
};

/**
 * Os modos que cada banco aceita (mesma regra de `SslMode::for_db_type` no
 * backend): Mongo e Redis não checam só a CA nem caem para texto puro;
 * SQLite é arquivo local.
 */
export function sslModesFor(dbType: DatabaseType): SslModeOption[] {
  switch (dbType) {
    case 'postgres':
      return Object.values(OPTIONS);
    case 'mongodb':
    case 'redis':
      return [OPTIONS.disable, OPTIONS.require, OPTIONS['verify-full']];
    case 'sqlite':
      return [];
  }
}

/** O modo mais próximo que o banco aceita (espelha o backend) */
export function sslModeFor(mode: SslMode, dbType: DatabaseType): SslMode {
  if (dbType === 'sqlite') return 'disable';
  if (dbType === 'postgres') return mode;
  if (mode === 'prefer') return 'disable';
  if (mode === 'verify-ca') return 'verify-full';
  return mode;
}

/** Modo que confere o certificado (e aceita uma CA própria) */
export const sslModeVerifies = (mode: SslMode | null | undefined) =>
  mode === 'verify-ca' || mode === 'verify-full';

export const sslModeLabel = (mode: SslMode) => OPTIONS[mode].label;
