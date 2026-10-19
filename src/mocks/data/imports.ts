import type {
  ImportCandidate,
  ImportSource,
  ImportSourceInfo,
} from '@/api/types/import.types';
import type { DatabaseType } from '@/api/types/server.types';

/**
 * Conexões "encontradas" em outros apps pelo modo mock. Cobrem os casos da
 * tela: túnel SSH, banco sem suporte, duplicado do "Loja (local)", Keychain,
 * pastas e SSL verificado.
 */
export const MOCK_IMPORT_SOURCES: ImportSourceInfo[] = [
  {
    source: 'dbeaver',
    path: '/Users/voce/Library/DBeaverData/workspace6/General/.dbeaver/data-sources.json',
    count: 5,
    passwords: 'file',
    fileExtensions: ['json'],
  },
  {
    source: 'tableplus',
    path: '/Users/fulano-de-tal/Library/Application Support/com.tinyapp.TablePlus/Data/Connections.plist',
    count: 4,
    passwords: 'keychain',
    fileExtensions: ['plist', 'json'],
  },
  {
    source: 'pgadmin',
    path: '/Users/voce/.pgadmin/pgadmin4.db',
    count: 2,
    passwords: 'none',
    fileExtensions: ['json', 'db'],
  },
  {
    source: 'beekeeper',
    path: null,
    count: 0,
    passwords: 'file',
    fileExtensions: ['db'],
  },
  {
    source: 'datagrip',
    path: null,
    count: 0,
    passwords: 'none',
    fileExtensions: ['xml'],
  },
];

type Fixture = Omit<ImportCandidate, 'key' | 'duplicateOf'>;

const base = (
  name: string,
  dbType: DatabaseType | null,
  driver: string,
  overrides: Partial<Fixture> = {},
): Fixture => ({
  name,
  dbType,
  driver,
  host: 'localhost',
  port: 5432,
  username: 'postgres',
  defaultDatabase: null,
  connectionUri: null,
  sslMode: 'disable',
  group: null,
  hasPassword: true,
  passwordInKeychain: false,
  warnings: dbType ? [] : [{ kind: 'unsupportedType', driver }],
  ...overrides,
});

const FIXTURES: Record<Exclude<ImportSource, 'urls'>, Fixture[]> = {
  dbeaver: [
    base('Loja (local)', 'postgres', 'PostgreSQL', { defaultDatabase: 'loja' }),
    base('ERP produção', 'postgres', 'PostgreSQL', {
      host: 'db.erp.interno',
      username: 'erp',
      defaultDatabase: 'erp',
      sslMode: 'verify-full',
      group: 'Clientes / ACME',
      warnings: [
        { kind: 'sshTunnel', host: 'bastion.acme.com' },
        { kind: 'sslSystemCa', mode: 'verify-full' },
      ],
    }),
    base('ERP homologação', 'postgres', 'PostgreSQL', {
      host: 'db-hml.erp.interno',
      username: 'erp',
      defaultDatabase: 'erp',
      group: 'Clientes / ACME',
    }),
    base('Notas', 'sqlite', 'SQLite', {
      host: '',
      port: 0,
      username: '',
      defaultDatabase: 'main',
      connectionUri: '/Users/voce/dados/notas.db',
      hasPassword: false,
    }),
    base('Legado', null, 'MySQL', { port: 3306, username: 'root' }),
  ],
  tableplus: [
    base('Analytics', 'postgres', 'PostgreSQL', {
      host: 'analytics.exemplo.com',
      username: 'leitura',
      defaultDatabase: 'warehouse',
      hasPassword: false,
      passwordInKeychain: true,
      group: 'Dados',
    }),
    base('Sessões', 'redis', 'Redis', {
      host: 'cache.exemplo.com',
      port: 6379,
      username: '',
      hasPassword: false,
      passwordInKeychain: true,
    }),
    base('Eventos', 'mongodb', 'MongoDB', {
      host: '',
      port: 27017,
      username: 'app',
      connectionUri:
        'mongodb+srv://app:•••@cluster0.exemplo.mongodb.net/eventos',
      hasPassword: false,
    }),
    // Banco sem suporte: nem a senha do Keychain é lida
    base('Engenharia de dados', null, 'MySQL', {
      port: 3306,
      username: 'lucas',
      defaultDatabase: 'exercicio1',
      hasPassword: false,
    }),
  ],
  pgadmin: [
    base('Faturamento', 'postgres', 'PostgreSQL', {
      host: '10.0.4.12',
      port: 6432,
      username: 'billing',
      defaultDatabase: 'billing',
      hasPassword: false,
      group: 'Financeiro',
      warnings: [{ kind: 'noPassword' }],
    }),
    base('Relatórios', 'postgres', 'PostgreSQL', {
      host: 'relatorios.interno',
      username: 'bi',
      defaultDatabase: 'bi',
      group: 'Financeiro',
      hasPassword: false,
      warnings: [
        { kind: 'sshTunnel', host: 'jump.interno' },
        { kind: 'noPassword' },
      ],
    }),
  ],
  beekeeper: [],
  datagrip: [],
};

/** As conexões de uma origem; URLs coladas viram candidatos pelo `URL` do navegador */
export function mockImportFixtures(
  source: ImportSource,
  text: string | null,
): Fixture[] {
  if (source !== 'urls') return FIXTURES[source];

  return (text ?? '')
    .split('\n')
    .map(line => line.trim())
    .filter(line => line && !line.startsWith('#'))
    .flatMap((line): Fixture[] => {
      if (line.startsWith('/') || line.startsWith('~/')) {
        const name =
          line
            .split('/')
            .at(-1)
            ?.replace(/\.[^.]+$/, '') ?? line;
        return [
          base(name, 'sqlite', 'SQLite', {
            host: '',
            port: 0,
            username: '',
            defaultDatabase: 'main',
            connectionUri: line,
            hasPassword: false,
          }),
        ];
      }
      let url: URL;
      try {
        url = new URL(line);
      } catch {
        return [];
      }
      const scheme = url.protocol.replace(':', '');
      const dbType: DatabaseType | null = scheme.startsWith('postgres')
        ? 'postgres'
        : scheme.startsWith('mongodb')
          ? 'mongodb'
          : scheme.startsWith('redis')
            ? 'redis'
            : null;
      const database = url.pathname.replace(/^\//, '') || null;
      return [
        base(
          database ? `${database} @ ${url.hostname}` : url.hostname,
          dbType,
          dbType ? dbType : scheme,
          {
            host: url.hostname,
            port: Number(url.port) || 0,
            username: decodeURIComponent(url.username),
            defaultDatabase: database,
            hasPassword: !!url.password,
            warnings: dbType
              ? []
              : [{ kind: 'unsupportedType', driver: scheme }],
          },
        ),
      ];
    });
}
