import { RustMethods } from '../rust-functions';
import type { Server } from '@/shared/models/servers.types';
import type {
  ExecuteQueryResponse,
  GetColumnsResponse,
  GetSchemasWithTablesResponse,
  GetDatabasesResponse,
  ApplyRowEditsResponse,
} from '../database/database-responses.types';
import { TableTypes } from '../database/database-responses.types';

let serverIdSeed = 2;

let servers: Server[] = [
  {
    id: 1,
    name: 'Local PostgreSQL',
    host: 'localhost',
    port: 5432,
    username: 'postgres',
    password: '',
    default_database: 'postgres',
    isConnected: true,
  },
];

const databasesByServer: Record<number, GetDatabasesResponse[]> = {
  1: [
    { name: 'postgres', sizeBytes: 1_024_000 },
    { name: 'analytics', sizeBytes: 2_048_000 },
  ],
};

const structuresByDatabase: Record<string, GetSchemasWithTablesResponse> = {
  '1:postgres': {
    fetchedAt: Date.now(),
    schemas: [
      {
        name: 'public',
        tables: [
          { name: 'users', tableType: TableTypes.Table },
          { name: 'orders', tableType: TableTypes.Table },
        ],
      },
    ],
  },
  '1:analytics': {
    fetchedAt: Date.now(),
    schemas: [
      {
        name: 'public',
        tables: [{ name: 'events', tableType: TableTypes.Table }],
      },
    ],
  },
};

const columnsByTable: Record<string, GetColumnsResponse> = {
  '1:postgres:public:users': [
    {
      name: 'id',
      dataType: 'uuid',
      isNullable: false,
      ordinal: 1,
      defaultValue: null,
      isPrimaryKey: true,
      isForeignKey: false,
      foreignKeyTarget: null,
    },
    {
      name: 'email',
      dataType: 'text',
      isNullable: false,
      ordinal: 2,
      defaultValue: null,
      isPrimaryKey: false,
      isForeignKey: false,
      foreignKeyTarget: null,
    },
  ],
  '1:postgres:public:orders': [
    {
      name: 'id',
      dataType: 'uuid',
      isNullable: false,
      ordinal: 1,
      defaultValue: null,
      isPrimaryKey: true,
      isForeignKey: false,
      foreignKeyTarget: null,
    },
    {
      name: 'user_id',
      dataType: 'uuid',
      isNullable: false,
      ordinal: 2,
      defaultValue: null,
      isPrimaryKey: false,
      isForeignKey: true,
      foreignKeyTarget: { schema: 'public', table: 'users', column: 'id' },
    },
  ],
  '1:analytics:public:events': [
    {
      name: 'id',
      dataType: 'bigint',
      isNullable: false,
      ordinal: 1,
      defaultValue: null,
      isPrimaryKey: true,
      isForeignKey: false,
      foreignKeyTarget: null,
    },
    {
      name: 'event_name',
      dataType: 'text',
      isNullable: false,
      ordinal: 2,
      defaultValue: null,
      isPrimaryKey: false,
      isForeignKey: false,
      foreignKeyTarget: null,
    },
  ],
};

const buildQueryResult = (query: string): ExecuteQueryResponse => {
  const lowerQuery = query.toLowerCase();
  if (lowerQuery.includes('users')) {
    return {
      columns: [
        { name: 'id', typeName: 'uuid', typeOid: null },
        { name: 'email', typeName: 'text', typeOid: null },
      ],
      rows: [
        ['11111111-1111-1111-1111-111111111111', 'alice@example.com'],
        ['22222222-2222-2222-2222-222222222222', 'bob@example.com'],
      ],
      hasMore: false,
      rowCount: 2,
      totalCount: 2,
      executionTimeMs: 12,
      editableInfo: {
        schema: 'public',
        table: 'users',
        primaryKeyColumns: ['id'],
        primaryKeyColumnIndices: [0],
      },
      queryId: `mock-${Date.now()}`,
    };
  }

  return {
    columns: [{ name: 'result', typeName: 'text', typeOid: null }],
    rows: [[`Executed in mock mode: ${query || 'empty query'}`]],
    hasMore: false,
    rowCount: 1,
    totalCount: 1,
    executionTimeMs: 8,
    editableInfo: null,
    queryId: `mock-${Date.now()}`,
  };
};

export async function invokeMock<T>(
  event: RustMethods,
  payload: Record<string, any>,
): Promise<T> {
  switch (event) {
    case RustMethods.GET_ALL_SERVERS:
      return servers.map(({ password: _password, ...server }) => server) as T;
    case RustMethods.CREATE_SERVER: {
      const id = ++serverIdSeed;
      const nextServer: Server = {
        ...payload.input,
        id,
        isConnected: false,
      };
      servers = [...servers, nextServer];
      return { ...nextServer, password: undefined } as T;
    }
    case RustMethods.UPDATE_SERVER: {
      const targetId = Number(payload.id);
      servers = servers.map(server =>
        server.id === targetId
          ? {
              ...server,
              ...payload,
              password:
                payload.password === '' ||
                payload.password === null ||
                payload.password === undefined
                  ? server.password
                  : payload.password,
            }
          : server,
      );
      const updated = servers.find(server => server.id === targetId);
      if (!updated) throw new Error('Server not found');
      return { ...updated, password: undefined } as T;
    }
    case RustMethods.DELETE_SERVER:
      servers = servers.filter(server => server.id !== Number(payload.id));
      return undefined as T;
    case RustMethods.CONNECT_TO_SERVER:
      return true as T;
    case RustMethods.GET_DATABASES:
      return (databasesByServer[payload.serverId] ?? []) as T;
    case RustMethods.GET_SCHEMAS_WITH_TABLES: {
      const key = `${payload.serverId}:${payload.database}`;
      return (
        structuresByDatabase[key] ?? {
          fetchedAt: Date.now(),
          schemas: [],
        }
      ) as T;
    }
    case RustMethods.GET_COLUMNS: {
      const key = `${payload.serverId}:${payload.database}:${payload.schema}:${payload.table}`;
      return (columnsByTable[key] ?? []) as T;
    }
    case RustMethods.EXECUTE_QUERY:
      return buildQueryResult(payload.query ?? '') as T;
    case RustMethods.APPLY_ROW_EDITS:
      return {
        affectedRows: payload.edits?.length ?? 0,
        executionTimeMs: 4,
      } as ApplyRowEditsResponse as T;
    case RustMethods.INSERT_TABLE_ROWS:
      return {
        affectedRows: payload.rows?.length ?? 0,
        executionTimeMs: 5,
      } as ApplyRowEditsResponse as T;
    case RustMethods.DELETE_TABLE_ROWS:
      return {
        affectedRows: payload.pkValuesList?.length ?? 0,
        executionTimeMs: 4,
      } as ApplyRowEditsResponse as T;
    case RustMethods.CANCEL_QUERY:
      return undefined as T;
    default:
      throw new Error(`Mock not implemented for method: ${event}`);
  }
}
