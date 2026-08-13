import { create } from 'zustand';

const key = (serverId: number, database: string) => `${serverId}:${database}`;

/**
 * Best-effort record of which (server, database) pairs were connected in this
 * session. Used by the command palette to skip the connect() round-trip when a
 * server is already reachable. Not persisted — if the backend drops a pool, the
 * tab's data query reconnects lazily, so a stale entry only costs one extra
 * query, never a broken state.
 */
interface ConnectionState {
  connected: Set<string>;
  isConnected: (serverId: number, database: string) => boolean;
  markConnected: (serverId: number, database: string) => void;
  clearServer: (serverId: number) => void;
}

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  connected: new Set(),

  isConnected: (serverId, database) => get().connected.has(key(serverId, database)),

  markConnected: (serverId, database) => {
    set(state => {
      const next = new Set(state.connected);
      next.add(key(serverId, database));
      return { connected: next };
    });
  },

  clearServer: serverId => {
    set(state => {
      const prefix = `${serverId}:`;
      const next = new Set(
        Array.from(state.connected).filter(k => !k.startsWith(prefix)),
      );
      return { connected: next };
    });
  },
}));
