import type { ServerGroupDialog } from '@/stores/server-group-dialog-store';

/** O diálogo de um `kind` específico */
export type GroupDialogOf<K extends ServerGroupDialog['kind']> = Extract<
  ServerGroupDialog,
  { kind: K }
>;

export interface GroupDialogProps<K extends ServerGroupDialog['kind']> {
  open: boolean;
  dialog: GroupDialogOf<K>;
}

/** Valor do select para "fora de grupo" (o Radix não aceita valor vazio) */
export const NO_GROUP = 'none';
