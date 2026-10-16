import type { PgToolsInstall, PgToolsProblem } from '@/api/types/backup.types';
import { isLinux, isMac } from './platform';

/** Como instalar as ferramentas do Postgres neste sistema */
export function pgToolsInstallHint(): string {
  if (isMac) {
    return 'Instale com "brew install libpq" ou pelo Postgres.app.';
  }
  if (isLinux) {
    return 'Instale o pacote do cliente (no Ubuntu/Debian: "sudo apt install postgresql-client").';
  }
  return 'Instale pelo instalador do PostgreSQL e marque "Command Line Tools".';
}

/** Frase para o usuário sobre o que impede o backup/restore */
export function describePgToolsProblem(problem: PgToolsProblem): string {
  switch (problem.kind) {
    case 'notFound':
      return `O pg_dump não foi encontrado. ${pgToolsInstallHint()}`;
    case 'invalidConfigured':
      return `${problem.message} Escolha outra pasta nas Preferências ou volte para a detecção automática.`;
    case 'clientTooOld':
      return `O pg_dump ${problem.client} é mais antigo que o servidor (${problem.server}) e não consegue ler esse banco. Instale a versão ${problem.server.split('.')[0]} ou mais nova.`;
  }
}

/** De onde veio a instalação, para mostrar ao lado da versão */
export function pgToolsSourceLabel(source: PgToolsInstall['source']): string {
  switch (source) {
    case 'configured':
      return 'pasta escolhida';
    case 'path':
      return 'PATH';
    case 'knownLocation':
      return 'detectada';
  }
}

/** Ferramentas que faltam na pasta (o `pg_dump` sempre existe) */
export function missingPgTools(install: PgToolsInstall): string[] {
  return [
    install.pgRestore ? null : 'pg_restore',
    install.psql ? null : 'psql',
  ].filter((name): name is string => name !== null);
}
