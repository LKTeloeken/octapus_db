import type { DatabaseType } from './server.types';

/** App (ou formato) de onde as conexões vêm */
export type ImportSource =
  | 'dbeaver'
  | 'tableplus'
  | 'pgadmin'
  | 'beekeeper'
  | 'datagrip'
  /** URLs coladas (`postgres://…`, `mongodb+srv://…`, `redis://…`) */
  | 'urls';

/** Como o app de origem guarda as senhas */
export type PasswordStorage =
  /** No próprio arquivo: vêm junto */
  | 'file'
  /** No Keychain do macOS: o sistema pede permissão para cada senha */
  | 'keychain'
  /** O app não as exporta; só o `~/.pgpass` completa (Postgres) */
  | 'none';

export interface ImportSourceInfo {
  source: Exclude<ImportSource, 'urls'>;
  /** Arquivo no lugar padrão; null = app não encontrado nesta máquina */
  path: string | null;
  count: number;
  passwords: PasswordStorage;
  /** Extensões aceitas ao escolher o arquivo à mão */
  fileExtensions: string[];
}

/** O que não sai igual do app de origem */
export type ImportWarning =
  | { kind: 'sshTunnel'; host: string }
  | { kind: 'unsupportedType'; driver: string }
  | { kind: 'noPassword' }
  | { kind: 'sslNotVerified'; mode: string }
  | { kind: 'clientCertificate' }
  | { kind: 'fileMissing'; path: string };

/** Uma conexão lida, já no formato do Octapus — a senha fica no backend */
export interface ImportCandidate {
  key: number;
  name: string;
  /** null = banco que o Octapus não abre (ver `driver`) */
  dbType: DatabaseType | null;
  /** Nome do banco no app de origem ("PostgreSQL", "MySQL"…) */
  driver: string;
  host: string;
  port: number;
  username: string;
  defaultDatabase: string | null;
  connectionUri: string | null;
  sslEnabled: boolean;
  /** Pasta/grupo no app de origem */
  group: string | null;
  hasPassword: boolean;
  /** A senha está no Keychain: só é lida ao importar */
  passwordInKeychain: boolean;
  /** Servidor do Octapus que parece ser o mesmo */
  duplicateOf: number | null;
  warnings: ImportWarning[];
}

export interface ImportPreview {
  /**
   * Identifica a prévia no commit (as senhas ficaram presas a ele). É um u32
   * no Rust: precisa voltar exato, e um Number só é exato até 2^53
   */
  token: number;
  source: ImportSource;
  path: string | null;
  candidates: ImportCandidate[];
  /** Pastas do app de origem */
  groups: string[];
}

export interface ImportPreviewRequest {
  source: ImportSource;
  /** Arquivo escolhido à mão; null = lugar padrão */
  path?: string | null;
  /** URLs coladas (origem `urls`) */
  text?: string | null;
}

export interface ImportCommitRequest {
  token: number;
  keys: number[];
  /** Ler as senhas (no TablePlus, do Keychain) */
  includePasswords: boolean;
  /** Recriar as pastas do app de origem como grupos */
  keepGroups: boolean;
  /** Grupo para quem não tem pasta (ou para todos, sem `keepGroups`) */
  targetGroupId: number | null;
}

export interface ImportResult {
  imported: number;
  withoutPassword: number;
  keychainDenied: boolean;
  groupsCreated: number;
}
