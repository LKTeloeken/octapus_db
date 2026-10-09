import {
  ArrowRight01Icon,
  DatabaseIcon,
  Folder01Icon,
  FolderLibraryIcon,
  HashtagIcon,
  Layers01Icon,
  MoreHorizontalIcon,
  ServerStack01Icon,
  TableIcon,
  ViewIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Spinner } from '@/components/ui/spinner';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { formatBytes } from '@/lib/format-bytes';
import { cn } from '@/lib/utils';
import type { CatalogNodeKind } from '@/api/types/catalog.types';
import type { NodeKind } from '@/lib/node-ref';
import type { NodeRowProps } from './node-row.types';

const KIND_ICONS: Record<NodeKind, typeof DatabaseIcon> = {
  server: ServerStack01Icon,
  database: DatabaseIcon,
  shape: FolderLibraryIcon,
  schema: Folder01Icon,
  table: TableIcon,
  column: HashtagIcon,
};

/** Views e tabelas particionadas se distinguem da tabela comum pelo ícone */
const RELATION_ICONS: Partial<Record<CatalogNodeKind, typeof DatabaseIcon>> = {
  view: ViewIcon,
  materializedView: ViewIcon,
  partitioned: Layers01Icon,
};

/**
 * Linha da árvore (28 px). O ícone é neutro, exceto o do servidor, que leva a
 * cor do banco; o chevron gira em vez de trocar de ícone; a tabela aberta na
 * aba ativa fica em `--active`; o cursor do teclado é um anel, não um fundo.
 */
export const NodeRow = memo(
  ({
    level,
    kind,
    name,
    subLabel,
    sizeBytes,
    hasChildren,
    isExpanded,
    isLoading,
    dbType,
    relationKind,
    badge,
    isOpen,
    isFocused,
    onClick,
    actions,
  }: NodeRowProps) => {
    const [isMenuOpen, setIsMenuOpen] = useState(false);
    const hasActions = !!actions?.length;
    const hasSize = sizeBytes != null;
    const isServer = kind === 'server';

    return (
      <div
        className={cn(
          'group relative flex h-7 cursor-pointer items-center gap-1.5 rounded-sm pr-2 text-body transition-colors',
          isServer ? 'font-medium text-fg' : 'text-fg-muted',
          isOpen ? 'bg-active text-fg' : 'hover:bg-hover hover:text-fg',
          isFocused && 'ring-[1.5px] ring-inset ring-ring',
          // Espaço reservado para o botão absoluto de ações
          hasActions && !hasSize && 'pr-8',
        )}
        style={{ paddingLeft: `${6 + level * 14}px` }}
        onClick={onClick}
        onContextMenu={
          hasActions
            ? event => {
                event.preventDefault();
                event.stopPropagation();
                setIsMenuOpen(true);
              }
            : undefined
        }
      >
        <span className="flex size-3.5 shrink-0 items-center justify-center">
          {hasChildren &&
            (isLoading ? (
              <Spinner className="size-3 text-fg-subtle" />
            ) : (
              <HugeiconsIcon
                icon={ArrowRight01Icon}
                className={cn(
                  'size-3.5 text-fg-subtle transition-transform duration-120 ease-standard motion-reduce:transition-none',
                  isExpanded && 'rotate-90',
                )}
              />
            ))}
        </span>

        <HugeiconsIcon
          icon={
            (relationKind && RELATION_ICONS[relationKind]) ?? KIND_ICONS[kind]
          }
          className={cn(
            'size-4 shrink-0',
            isServer && dbType
              ? DB_TYPE_TEXT_COLOR[dbType]
              : isOpen
                ? 'text-fg'
                : 'text-fg-subtle',
          )}
        />

        {/* O nome tem prioridade: o rótulo secundário (tipo, contagem) encolhe antes. */}
        <span className="min-w-0 truncate">{name}</span>
        {subLabel && (
          <span className="min-w-0 shrink-[3] truncate text-small font-normal text-fg-subtle tabular-nums">
            {subLabel}
          </span>
        )}
        {badge && (
          <Badge
            variant="warning"
            className="font-mono tabular-nums"
            title={badge.title}
          >
            {badge.label}
          </Badge>
        )}

        {hasSize && (
          <span
            className="ml-auto shrink-0 font-mono text-[11px] tabular-nums text-fg-subtle"
            title={`${sizeBytes.toLocaleString()} bytes`}
          >
            {formatBytes(sizeBytes)}
          </span>
        )}

        {hasActions && (
          <Popover open={isMenuOpen} onOpenChange={setIsMenuOpen}>
            {hasSize ? (
              // Sem botão: o menu do clique direito abre ancorado no tamanho
              <PopoverAnchor className="absolute right-2 h-6" />
            ) : (
              <div
                className={cn(
                  'absolute right-1 transition-opacity',
                  isMenuOpen
                    ? 'opacity-100'
                    : 'opacity-0 group-hover:opacity-100',
                )}
              >
                <PopoverTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`Ações de ${name}`}
                    onClick={event => event.stopPropagation()}
                  >
                    <HugeiconsIcon icon={MoreHorizontalIcon} />
                  </Button>
                </PopoverTrigger>
              </div>
            )}
            <PopoverContent className="w-48 p-1.5" align="end">
              {actions.map(action => (
                <Button
                  key={action.label}
                  variant="ghost"
                  size="sm"
                  className="w-full justify-start gap-2 px-2 font-normal text-fg hover:bg-active"
                  onClick={event => {
                    event.stopPropagation();
                    action.onSelect();
                    setIsMenuOpen(false);
                  }}
                >
                  <HugeiconsIcon
                    icon={action.icon}
                    className="size-4 text-fg-muted"
                  />
                  {action.label}
                </Button>
              ))}
            </PopoverContent>
          </Popover>
        )}
      </div>
    );
  },
);

NodeRow.displayName = 'NodeRow';
