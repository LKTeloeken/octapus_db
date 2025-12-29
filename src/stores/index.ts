import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import {
  createSchemaCacheSlice,
  type SchemaCacheState,
} from "./slices/schemaCache";

export const useStore = create<SchemaCacheState>()(
  persist(
    (...a) => ({
      ...createSchemaCacheSlice(...a),
    }),
    {
      name: "octapus-db-store",
      partialize: (state) => ({ cache: state.cache }),
      storage: createJSONStorage(() => localStorage),
    }
  )
);
