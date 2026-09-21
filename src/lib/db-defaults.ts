import mongodbIcon from '@/assets/mongodb-icon.svg';
import postgresIcon from '@/assets/postgres-icon.svg';
import redisIcon from '@/assets/redis-icon.svg';
import sqliteIcon from '@/assets/sqlite-icon.svg';
import type { DatabaseType } from '@/api/types/server.types';

export const DEFAULT_PORTS: Record<DatabaseType, number> = {
  postgres: 5432,
  mongodb: 27017,
  redis: 6379,
  // Banco de arquivo: não tem porta
  sqlite: 0,
};

/** Database used when none is specified (BACKEND.md §6.2) */
export const DEFAULT_DATABASES: Record<DatabaseType, string> = {
  postgres: 'postgres',
  mongodb: 'admin',
  redis: '0',
  sqlite: 'main',
};

export const DB_TYPE_LABELS: Record<DatabaseType, string> = {
  postgres: 'PostgreSQL',
  mongodb: 'MongoDB',
  redis: 'Redis',
  sqlite: 'SQLite',
};

/** URLs resolvidas pelo Vite — caminho literal não é emitido no build */
export const DB_TYPE_ICONS: Record<DatabaseType, string> = {
  postgres: postgresIcon,
  mongodb: mongodbIcon,
  redis: redisIcon,
  sqlite: sqliteIcon,
};

/** Types with a working adapter — mysql returns "coming soon" */
export const SUPPORTED_DB_TYPES: DatabaseType[] = [
  'postgres',
  'mongodb',
  'redis',
  'sqlite',
];
