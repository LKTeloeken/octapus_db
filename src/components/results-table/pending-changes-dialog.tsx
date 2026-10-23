import { Cancel as AlertDialogCancelPrimitive } from '@radix-ui/react-alert-dialog';
import type { ReactNode } from 'react';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';

export interface PendingChangesDialogProps {
  open: boolean;
  title: ReactNode;
  description: ReactNode;
  /** Rótulo do botão destrutivo (descarta as pendências e segue) */
  discardLabel: string;
  onDiscard: () => void;
  onCancel: () => void;
  /** Grava antes de seguir; sem ele, só há descartar ou cancelar */
  onSave?: () => void;
  isSaving?: boolean;
}

/**
 * Pergunta antes de uma ação que perderia as alterações pendentes da grade
 * (fechar a aba, ordenar, filtrar, recarregar, reexecutar a query).
 */
export function PendingChangesDialog({
  open,
  title,
  description,
  discardLabel,
  onDiscard,
  onCancel,
  onSave,
  isSaving = false,
}: PendingChangesDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={next => !next && onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>

        <AlertDialogFooter>
          {/* O Cancel do Radix recebe o foco ao abrir: Enter não descarta por engano */}
          <AlertDialogCancelPrimitive asChild>
            <Button variant="ghost" size="sm" disabled={isSaving}>
              Cancelar
            </Button>
          </AlertDialogCancelPrimitive>
          <Button
            variant="destructive"
            size="sm"
            disabled={isSaving}
            onClick={onDiscard}
          >
            {discardLabel}
          </Button>
          {onSave && (
            <Button size="sm" disabled={isSaving} onClick={onSave}>
              {isSaving ? 'Salvando…' : 'Salvar'}
            </Button>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
