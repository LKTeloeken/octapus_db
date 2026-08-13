import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { encodeNodeId } from '@/lib/node-ref';

export interface TableRef {
  serverId: number;
  database: string;
  /** null for schema-less databases (Mongo/Redis) */
  schema: string | null;
  table: string;
}

const MAX_RECENTS = 20;

interface RecentTablesState {
  recents: TableRef[];
  addRecent: (ref: TableRef) => void;
}

/** Last tables opened from the command palette, most-recent first. */
export const useRecentTablesStore = create<RecentTablesState>()(
  persist(
    set => ({
      recents: [],

      addRecent: ref => {
        set(state => {
          const id = encodeNodeId({
            serverId: ref.serverId,
            database: ref.database,
            schema: ref.schema ?? undefined,
            table: ref.table,
          });

          const deduped = state.recents.filter(
            existing =>
              encodeNodeId({
                serverId: existing.serverId,
                database: existing.database,
                schema: existing.schema ?? undefined,
                table: existing.table,
              }) !== id,
          );

          return { recents: [ref, ...deduped].slice(0, MAX_RECENTS) };
        });
      },
    }),
    { name: 'octapus-recent-tables' },
  ),
);
