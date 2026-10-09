import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useRef } from 'react';
import { Spinner } from '@/components/ui/spinner';
import { ErrorRow } from './error-row';
import { FilterRow } from './filter-row';
import { MoreRow } from './more-row';
import { NodeRow } from './node-row/node-row';
import { useConnectionTree } from './use-connection-tree';
import { useTreeEnterAnimation } from './use-tree-enter-animation';
import { useTreeNavigation } from './use-tree-navigation';
import type { ConnectionTreeProps } from './connection-tree.types';

// Linha de 28 px + 1 px de respiro entre as linhas.
const ROW_HEIGHT = 29;
const OVERSCAN = 12;

/**
 * Virtualized sidebar tree. The visible tree is flattened in
 * useConnectionTree and only the on-screen rows are rendered, so a
 * multi-tenant database with thousands of tables stays smooth.
 *
 * O container é o único tab stop da árvore: como as linhas são virtualizadas,
 * uma ordem natural de Tab seria caótica. A navegação vem do teclado
 * (useTreeNavigation) e do clique, que convergem no mesmo cursor.
 */
export const ConnectionTree = memo(({ onEditServer }: ConnectionTreeProps) => {
  const { rows, isLoading, isError, error, retry, isEmpty } = useConnectionTree(
    {
      onEditServer,
    },
  );
  const parentRef = useRef<HTMLDivElement>(null);
  const { getEnterDelay } = useTreeEnterAnimation(rows);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
    getItemKey: index => rows[index].id,
  });

  const { focusedNodeId, setFocusedNode, onKeyDown, onFocus, leaveFilter } =
    useTreeNavigation({
      rows,
      virtualizer,
      containerRef: parentRef,
    });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-4">
        <Spinner className="size-4 text-fg-subtle" />
      </div>
    );
  }

  if (isError) {
    return (
      <ErrorRow
        level={0}
        message={error?.message ?? 'Falha ao carregar servidores'}
        onRetry={retry}
      />
    );
  }

  if (isEmpty) {
    return (
      <p className="px-2 py-1 text-small text-fg-subtle">
        Nenhum servidor ainda — use o + para adicionar.
      </p>
    );
  }

  return (
    <div
      ref={parentRef}
      tabIndex={0}
      role="tree"
      onKeyDown={onKeyDown}
      onFocus={onFocus}
      className="h-full overflow-auto scrollbar-thin outline-none"
    >
      <div
        className="relative w-full"
        style={{ height: `${virtualizer.getTotalSize()}px` }}
      >
        {virtualizer.getVirtualItems().map(virtualRow => {
          const row = rows[virtualRow.index];
          if (!row) return null;
          const enterDelay = getEnterDelay(virtualRow.index);

          return (
            <div
              key={virtualRow.key}
              // O mousedown já foca o container (tabIndex=0); aqui só alinhamos
              // o cursor do teclado com o que foi clicado.
              onMouseDown={() => {
                if (row.variant === 'node' || row.variant === 'more') {
                  setFocusedNode(row.id);
                }
              }}
              className="absolute left-0 top-0 w-full"
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              {/* A animação de entrada vai num elemento interno: o externo já usa
                  `transform` para se posicionar na lista virtual. */}
              <div
                className={
                  enterDelay === null
                    ? undefined
                    : 'animate-in fade-in-0 slide-in-from-top-1 fill-mode-both duration-180 ease-out'
                }
                style={
                  enterDelay === null
                    ? undefined
                    : { animationDelay: `${enterDelay}ms` }
                }
              >
                {row.variant === 'error' ? (
                  <ErrorRow
                    level={row.level}
                    message={row.message}
                    onRetry={row.onRetry}
                  />
                ) : row.variant === 'filter' ? (
                  <FilterRow
                    level={row.level}
                    nodeId={row.nodeId}
                    value={row.value}
                    total={row.total}
                    placeholder={row.placeholder}
                    onChange={row.onChange}
                    onLeave={() => leaveFilter(row.nodeId)}
                  />
                ) : row.variant === 'more' ? (
                  <MoreRow
                    level={row.level}
                    remaining={row.remaining}
                    isLoading={row.isLoading}
                    isFocused={row.id === focusedNodeId}
                    onMore={row.onMore}
                  />
                ) : (
                  <NodeRow
                    {...row.props}
                    isFocused={row.id === focusedNodeId}
                  />
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
});

ConnectionTree.displayName = 'ConnectionTree';
