import { Dispatch, SetStateAction } from "react";
import { toast } from "react-hot-toast";
import { ITreeNode } from "@/shared/models/tree";

export type NodeKey =
  | `server::${number}`
  | `database::${number}::${string}`
  | `schema::${number}::${string}::${string}`
  | `schema_table::${number}::${string}::${string}`
  | `table::${number}::${string}::${string}::${string}`
  | `table_column::${number}::${string}::${string}::${string}`
  | `table_index::${number}::${string}::${string}::${string}`
  | `table_trigger::${number}::${string}::${string}::${string}`
  | `column::${number}::${string}::${string}::${string}::${string}`
  | `index::${number}::${string}::${string}::${string}::${string}`
  | `trigger::${number}::${string}::${string}::${string}::${string}`;

export const key = (...parts: (string | number)[]) =>
  parts.join("::") as NodeKey;

type TreeSetter = Dispatch<SetStateAction<Record<string, ITreeNode>>>;

export function createTreeActions(setServers: TreeSetter) {
  function upsertNode(k: NodeKey, name: string, data: any = {}) {
    setServers((prev) => {
      const next = { ...prev } as Record<string, ITreeNode>;
      const existing = next[k];
      next[k] = {
        name: name ?? existing?.name,
        children: existing?.children ?? [],
        data: { ...(existing?.data ?? {}), ...data },
      };
      return next;
    });
  }

  function setChildren(parent: NodeKey, children: NodeKey[]) {
    setServers((prev) => {
      const next = { ...prev } as Record<string, ITreeNode>;
      if (next[parent]) next[parent] = { ...next[parent], children };
      return next;
    });
  }

  function removeSubtree(root: NodeKey) {
    setServers((prev) => {
      const next = { ...prev } as Record<string, ITreeNode>;
      const stack: string[] = [root];
      while (stack.length) {
        const k = stack.pop()!;
        const node = next[k];
        if (node?.children?.length) stack.push(...(node.children as string[]));
        delete next[k];
      }
      return next;
    });
  }

  return { upsertNode, setChildren, removeSubtree };
}

// `run` vem do asyncRunner. Criamos um loader genérico que usa as actions de árvore.
type RunFn = <T>(opts: {
  kind?: "load" | "connect";
  task: () => Promise<T>;
  onSuccess?: (r: T) => void;
  onError?: (e: unknown) => void;
}) => Promise<any>;

export function createLoadChildren(
  run: RunFn,
  { upsertNode, setChildren }: ReturnType<typeof createTreeActions>
) {
  return async function loadChildren<T>({
    parentKey,
    fetcher,
    mapItem,
    emptyMsg,
    successMsg,
  }: {
    parentKey: NodeKey;
    fetcher: () => Promise<T[]>;
    mapItem: (item: T) => { key: NodeKey; name: string; data?: any };
    emptyMsg: string;
    successMsg: string;
  }): Promise<T[]> {
    return run<T[]>({
      task: async () => {
        const items = await fetcher();
        if (!items || !items.length) {
          toast.error(emptyMsg);
          setChildren(parentKey, []);
          return [] as T[];
        }
        const childKeys: NodeKey[] = [];
        items.forEach((it) => {
          const { key: k, name, data } = mapItem(it);
          childKeys.push(k);
          upsertNode(k, name, data);
        });
        setChildren(parentKey, childKeys);
        toast.success(successMsg);
        return items;
      },
    });
  };
}
