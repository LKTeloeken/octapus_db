import type { DatabaseType } from '@/api/types/server.types';

export const DEFAULT_PORTS: Record<DatabaseType, number> = {
  postgres: 5432,
  mongodb: 27017,
  redis: 6379,
  mysql: 3306,
  sqlite: 0,
};

/** Database used when none is specified (BACKEND.md §6.2) */
export const DEFAULT_DATABASES: Record<DatabaseType, string> = {
  postgres: 'postgres',
  mongodb: 'admin',
  redis: '0',
  mysql: 'mysql',
  sqlite: 'main',
};

export const DB_TYPE_LABELS: Record<DatabaseType, string> = {
  postgres: 'PostgreSQL',
  mongodb: 'MongoDB',
  redis: 'Redis',
  mysql: 'MySQL (em breve)',
  sqlite: 'SQLite (em breve)',
};

/** Types with a working adapter — mysql/sqlite return "coming soon" */
export const SUPPORTED_DB_TYPES: DatabaseType[] = [
  'postgres',
  'mongodb',
  'redis',
];
