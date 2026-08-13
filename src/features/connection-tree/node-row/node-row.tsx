import {
  ArrowDown01Icon,
  ArrowRight01Icon,
  DatabaseIcon,
  Folder01Icon,
  HashtagIcon,
  MoreHorizontalIcon,
  TableIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Server as ServerIcon } from 'lucide-react';
import { memo, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';
import type { NodeKind } from '@/lib/node-ref';
import type { NodeRowProps } from './node-row.types';

const ICON_CLASS = 'min-w-4 min-h-4 max-w-4 max-h-4';

const KIND_ICONS: Record<Exclude<NodeKind, 'server'>, typeof DatabaseIcon> = {
  database: DatabaseIcon,
  schema: Folder01Icon,
  table: TableIcon,
  column: HashtagIcon,
};

const NodeKindIcon = ({
  kind,
  isHighlighted,
}: {
  kind: NodeKind;
  isHighlighted?: boolean;
}) => {
  if (kind === 'server') {
    return (
      <ServerIcon className={cn(ICON_CLASS, isHighlighted && 'text-primary')} />
    );
  }

  return <HugeiconsIcon icon={KIND_ICONS[kind]} className={ICON_CLASS} />;
};

export const NodeRow = memo(
  ({
    level,
    kind,
    name,
    subLabel,
    hasChildren,
    isExpanded,
    isLoading,
    isHighlighted,
    isFocused,
    onClick,
    actions,
  }: NodeRowProps) => {
    const [isMenuOpen, setIsMenuOpen] = useState(false);

    return (
      <div
        className={cn(
          'flex items-center gap-2 py-1 px-2 cursor-pointer hover:text-foreground hover:bg-surface-light/50 rounded-md transition-color group relative pr-8',
          isFocused && 'bg-surface-light/40 ring-1 ring-inset ring-primary',
        )}
        style={{ paddingLeft: `${level * 1.25 + 0.5}rem` }}
        onClick={onClick}
        onContextMenu={
          actions?.length
            ? event => {
                event.preventDefault();
                event.stopPropagation();
                setIsMenuOpen(true);
              }
            : undefined
        }
      >
        <div className="flex items-center justify-center w-4 h-4 shrink-0">
          {hasChildren &&
            (isLoading ? (
              <Spinner className="w-3 h-3" />
            ) : (
              <HugeiconsIcon
                icon={isExpanded ? ArrowDown01Icon : ArrowRight01Icon}
                className={cn(ICON_CLASS, 'text-foreground')}
              />
            ))}
        </div>

        <div className="flex items-center gap-2 min-w-0">
          <NodeKindIcon kind={kind} isHighlighted={isHighlighted} />

          <div className="flex items-baseline gap-1.5 min-w-0">
            <div className="text-sm truncate">{name}</div>
            {subLabel && (
              <div className="text-xs text-muted-foreground truncate">
                {subLabel}
              </div>
            )}
          </div>
        </div>

        {actions && actions.length > 0 && (
          <div
            className={cn(
              'absolute right-2 transition-opacity',
              isMenuOpen ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
            )}
          >
            <Popover open={isMenuOpen} onOpenChange={setIsMenuOpen}>
              <PopoverTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6"
                  onClick={event => event.stopPropagation()}
                >
                  <HugeiconsIcon
                    icon={MoreHorizontalIcon}
                    className="h-4 w-4"
                  />
                </Button>
              </PopoverTrigger>
              <PopoverContent className="w-36 p-1" align="end">
                {actions.map(action => (
                  <Button
                    key={action.label}
                    variant="ghost"
                    size="sm"
                    className="w-full justify-start gap-2"
                    onClick={event => {
                      event.stopPropagation();
                      action.onSelect();
                      setIsMenuOpen(false);
                    }}
                  >
                    <HugeiconsIcon icon={action.icon} className="h-3 w-3" />
                    {action.label}
                  </Button>
                ))}
              </PopoverContent>
            </Popover>
          </div>
        )}
      </div>
    );
  },
);

NodeRow.displayName = 'NodeRow';
