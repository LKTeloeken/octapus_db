import type {
  ImportCandidate,
  ImportSource,
  ImportSourceInfo,
  ImportWarning,
  PasswordStorage,
} from '@/api/types/import.types';

export const SOURCE_LABELS: Record<ImportSource, string> = {
  dbeaver: 'DBeaver',
  tableplus: 'TablePlus',
  pgadmin: 'pgAdmin 4',
  beekeeper: 'Beekeeper Studio',
  datagrip: 'DataGrip',
  urls: 'URLs',
};

/** Ordem das origens na tela: as mais comuns primeiro */
export const SOURCE_ORDER: ImportSourceInfo['source'][] = [
  'dbeaver',
  'tableplus',
  'pgadmin',
  'datagrip',
  'beekeeper',
];

const PASSWORD_LABELS: Record<PasswordStorage, string> = {
  file: 'com senhas',
  keychain: 'senhas no Keychain',
  none: 'sem senhas',
};

/** "5 conexões · com senhas" ou por que a origem não está disponível */
export function sourceSummary(info: ImportSourceInfo): string {
  if (!info.path) return 'Não encontrado nesta máquina';
  if (info.count === 0) return 'Nenhuma conexão salva';
  const count = info.count === 1 ? '1 conexão' : `${info.count} conexões`;
  return `${count} · ${PASSWORD_LABELS[info.passwords]}`;
}

/** Dica de onde achar o arquivo quando o app não está no lugar padrão */
export const SOURCE_FILE_HINTS: Record<ImportSourceInfo['source'], string> = {
  dbeaver: 'data-sources.json da pasta .dbeaver do projeto',
  tableplus: 'Connections.plist da pasta Data do TablePlus',
  pgadmin: 'JSON de Tools › Import/Export Servers, ou o pgadmin4.db',
  beekeeper: 'app.db da pasta do Beekeeper Studio',
  datagrip: 'dataSources.xml da pasta .idea do projeto',
};

export interface WarningLabel {
  label: string;
  /** Explicação (tooltip) */
  title: string;
  tone: 'warning' | 'danger' | 'neutral';
}

export function warningLabel(warning: ImportWarning): WarningLabel {
  switch (warning.kind) {
    case 'sshTunnel':
      return {
        label: 'SSH',
        title: `Passa por túnel SSH${warning.host ? ` (${warning.host})` : ''}, que o Octapus ainda não suporta. Será importada, mas só conecta com o túnel aberto por fora.`,
        tone: 'warning',
      };
    case 'unsupportedType':
      return {
        label: warning.driver,
        title: `O Octapus ainda não abre ${warning.driver}.`,
        tone: 'danger',
      };
    case 'noPassword':
      return {
        label: 'Sem senha',
        title:
          'A senha não veio do app de origem: informe ao editar o servidor.',
        tone: 'neutral',
      };
    case 'sslNotVerified':
      return {
        label: 'SSL',
        title: `Usava ${warning.mode}; o Octapus cifra a conexão sem verificar o certificado.`,
        tone: 'neutral',
      };
    case 'clientCertificate':
      return {
        label: 'Certificado',
        title: 'Usava certificado de cliente, que o Octapus ainda não suporta.',
        tone: 'warning',
      };
    case 'fileMissing':
      return {
        label: 'Arquivo ausente',
        title: `${warning.path} não existe nesta máquina.`,
        tone: 'warning',
      };
  }
}

/** Onde a conexão aponta, numa linha: `app@db.exemplo.com:5432/loja` */
export function candidateTarget(candidate: ImportCandidate): string {
  if (candidate.dbType === 'sqlite') return candidate.connectionUri ?? '';
  if (candidate.connectionUri) return redactUri(candidate.connectionUri);
  const user = candidate.username ? `${candidate.username}@` : '';
  const db = candidate.defaultDatabase ? `/${candidate.defaultDatabase}` : '';
  return `${user}${candidate.host}:${candidate.port}${db}`;
}

/** A URI pode trazer a senha: nunca na tela */
export function redactUri(uri: string): string {
  return uri.replace(/(\/\/[^:/@]*:)[^@/]*@/, '$1•••@');
}

/** Entra marcada: banco suportado e ainda não cadastrado */
export const selectedByDefault = (candidate: ImportCandidate) =>
  candidate.dbType !== null && candidate.duplicateOf === null;
