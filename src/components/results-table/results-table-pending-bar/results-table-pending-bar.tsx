import { HugeiconsIcon } from '@hugeicons/react';
import { Undo02Icon } from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Kbd } from '@/components/ui/kbd';
import { shortcut } from '@/lib/platform';
import type { ResultsTablePendingBarProps } from './results-table-pending-bar.types';

const Dot = ({ className }: { className: string }) => (
  <span aria-hidden className={`size-1.5 rounded-full ${className}`} />
);

/**
 * Barra flutuante de alterações pendentes (DESIGN.md §8): vidro sobre a grade,
 * centralizada acima da status bar. Sobe 12 px ao aparecer; só existe enquanto
 * há algo a salvar.
 */
export const ResultsTablePendingBar = memo(
  ({
    changesCount,
    addedCount,
    removedCount,
    onDiscard,
    onSave,
  }: ResultsTablePendingBarProps) => {
    const total = changesCount + addedCount + removedCount;

    return (
      <div
        role="status"
        className="glass pointer-events-auto flex h-11 items-center gap-2.5 rounded-lg pr-1.5 pl-3.5 animate-in fade-in-0 slide-in-from-bottom-3 duration-240 ease-out"
      >
        <span className="text-body font-medium whitespace-nowrap text-fg">
          {total} alteraç{total === 1 ? 'ão' : 'ões'}
        </span>
        {changesCount > 0 && (
          <Badge variant="warning">
            <Dot className="bg-warning" />
            {changesCount} editada{changesCount === 1 ? '' : 's'}
          </Badge>
        )}
        {addedCount > 0 && (
          <Badge variant="success">
            <Dot className="bg-success" />
            {addedCount} nova{addedCount === 1 ? '' : 's'}
          </Badge>
        )}
        {removedCount > 0 && (
          <Badge variant="danger">
            <Dot className="bg-danger" />
            {removedCount} removida{removedCount === 1 ? '' : 's'}
          </Badge>
        )}
        <span aria-hidden className="mx-0.5 h-4 w-px bg-line" />
        <Button variant="ghost" size="sm" onClick={onDiscard}>
          <HugeiconsIcon icon={Undo02Icon} />
          Descartar
        </Button>
        <Button size="sm" onClick={onSave}>
          Salvar
          <Kbd variant="inverse">{shortcut('S')}</Kbd>
        </Button>
      </div>
    );
  },
);

ResultsTablePendingBar.displayName = 'ResultsTablePendingBar';
