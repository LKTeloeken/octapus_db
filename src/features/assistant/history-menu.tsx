import { Clock01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Button } from '@/components/ui/button';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Spinner } from '@/components/ui/spinner';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { useHistoryMenu } from './use-history-menu';

const PARADIGM_LABELS: Record<string, string> = {
  postgres: 'PostgreSQL',
  sqlite: 'SQLite',
  mongodb: 'MongoDB',
  redis: 'Redis',
};

const dateFormat = new Intl.DateTimeFormat('pt-BR', {
  dateStyle: 'short',
  timeStyle: 'short',
});

/** Conversas anteriores, vindas do servidor */
export function HistoryMenu() {
  const { open, setOpen, items, isLoading, error, select } = useHistoryMenu();

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip content="Conversas anteriores" position="bottom">
        <PopoverTrigger asChild>
          <Button variant="ghost" size="icon" aria-label="Conversas anteriores">
            <HugeiconsIcon icon={Clock01Icon} />
          </Button>
        </PopoverTrigger>
      </Tooltip>
      <PopoverContent align="end" className="w-80 p-1.5">
        {isLoading ? (
          <div className="flex justify-center py-4">
            <Spinner className="size-4 text-fg-subtle" />
          </div>
        ) : error ? (
          <p className="px-2 py-3 text-small text-danger">
            Não consegui carregar o histórico.
          </p>
        ) : items.length === 0 ? (
          <p className="px-2 py-3 text-small text-fg-muted">
            Nenhuma conversa ainda.
          </p>
        ) : (
          <ul className="flex max-h-80 flex-col overflow-y-auto scrollbar-thin">
            {items.map(item => (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => select(item)}
                  className="flex w-full cursor-pointer flex-col items-start gap-0.5 rounded-sm px-2 py-1.5 text-left outline-none hover:bg-active focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring"
                >
                  <span className="w-full truncate text-body text-fg">
                    {item.title ?? 'Sem título'}
                  </span>
                  <span className="text-small text-fg-subtle">
                    {PARADIGM_LABELS[item.paradigm] ?? item.paradigm} ·{' '}
                    {item.model.alias} ·{' '}
                    {dateFormat.format(new Date(item.updatedAt))}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </PopoverContent>
    </Popover>
  );
}
