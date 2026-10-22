import {
  ArrowRight01Icon,
  DatabaseIcon,
  Folder01Icon,
  Folder02Icon,
  FolderCodeIcon,
  FolderDetailsIcon,
  FolderKeyIcon,
  FolderLibraryIcon,
  FolderViewIcon,
  FoldersIcon,
  FunctionSquareIcon,
  HashtagIcon,
  Layers01Icon,
  MoreHorizontalIcon,
  ServerStack01Icon,
  TableIcon,
  ViewIcon,
  ZapIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Fragment, memo, useState } from 'react';
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
import type {
  NodeRowKind,
  NodeRowProps,
  TreeFolderKind,
} from './node-row.types';

const KIND_ICONS: Record<NodeRowKind, typeof DatabaseIcon> = {
  group: Folder02Icon,
  server: ServerStack01Icon,
  database: DatabaseIcon,
  shape: FolderLibraryIcon,
  schema: Folder01Icon,
  table: TableIcon,
  folder: Folder01Icon,
  index: ZapIcon,
  function: FunctionSquareIcon,
  column: HashtagIcon,
};

/** Cada pasta com o ícone do que guarda */
const FOLDER_ICONS: Record<TreeFolderKind, typeof DatabaseIcon> = {
  tables: Folder01Icon,
  columns: FolderDetailsIcon,
  partitions: FoldersIcon,
  view: FolderViewIcon,
  materializedView: FolderViewIcon,
  index: FolderKeyIcon,
  function: FolderCodeIcon,
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
    folderKind,
    title,
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
    // Grupo e servidor são os nós de topo: nome em destaque
    const isTopLevel = isServer || kind === 'group';
    // Sem botão: o tamanho ocupa o lugar dele, e numa coluna, índice ou
    // função o nome e o detalhe precisam da largura toda — o menu abre pelo
    // clique direito
    const contextMenuOnly =
      hasSize || kind === 'column' || kind === 'index' || kind === 'function';

    return (
      <div
        className={cn(
          'group relative flex h-7 cursor-pointer items-center gap-1.5 rounded-sm pr-2 text-body transition-colors',
          isTopLevel ? 'font-medium text-fg' : 'text-fg-muted',
          isOpen ? 'bg-active text-fg' : 'hover:bg-hover hover:text-fg',
          isFocused && 'ring-[1.5px] ring-inset ring-ring',
          // Espaço reservado para o botão absoluto de ações
          hasActions && !contextMenuOnly && 'pr-8',
        )}
        style={{ paddingLeft: `${6 + level * 14}px` }}
        title={title}
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
            (folderKind && FOLDER_ICONS[folderKind]) ??
            (relationKind && RELATION_ICONS[relationKind]) ??
            KIND_ICONS[kind]
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
            {contextMenuOnly ? (
              // Sem botão: o menu do clique direito abre ancorado à direita
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
                <Fragment key={action.label}>
                  {action.separated && (
                    <div
                      role="separator"
                      className="-mx-1.5 my-1 h-px bg-line-subtle"
                    />
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    className={cn(
                      'w-full justify-start gap-2 px-2 font-normal',
                      action.danger
                        ? 'text-danger hover:bg-danger-soft hover:text-danger'
                        : 'text-fg hover:bg-active',
                    )}
                    onClick={event => {
                      event.stopPropagation();
                      action.onSelect();
                      setIsMenuOpen(false);
                    }}
                  >
                    <HugeiconsIcon
                      icon={action.icon}
                      className={cn(
                        'size-4',
                        action.danger ? 'text-danger' : 'text-fg-muted',
                      )}
                    />
                    {action.label}
                  </Button>
                </Fragment>
              ))}
            </PopoverContent>
          </Popover>
        )}
      </div>
    );
  },
);

NodeRow.displayName = 'NodeRow';
