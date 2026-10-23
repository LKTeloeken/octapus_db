import { PendingChangesDialog } from '@/components/results-table/pending-changes-dialog';
import { pendingCountLabel } from '@/components/results-table/pending-changes';
import type { ClosingTab } from './query-tabs.types';

interface CloseTabDialogProps {
  closing: ClosingTab | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/** Confirma fechar uma aba cuja grade tem alterações pendentes (não salvas) */
export function CloseTabDialog({
  closing,
  onConfirm,
  onCancel,
}: CloseTabDialogProps) {
  const count = closing?.pendingCount ?? 0;

  return (
    <PendingChangesDialog
      open={closing?.open ?? false}
      title="Descartar as alterações pendentes?"
      description={
        <>
          A aba <span className="font-medium text-fg">{closing?.title}</span>{' '}
          tem {pendingCountLabel(count)} que não{' '}
          {count === 1 ? 'foi salva' : 'foram salvas'} no banco. Fechar a aba
          descarta {count === 1 ? 'essa alteração' : 'essas alterações'}.
        </>
      }
      discardLabel="Descartar e fechar"
      onDiscard={onConfirm}
      onCancel={onCancel}
    />
  );
}
