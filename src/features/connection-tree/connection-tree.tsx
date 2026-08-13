import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useRef } from 'react';
import { Spinner } from '@/components/ui/spinner';
import { ErrorRow } from './error-row';
import { NodeRow } from './node-row/node-row';
import { useConnectionTree } from './use-connection-tree';
import { useTreeNavigation } from './use-tree-navigation';
import type { ConnectionTreeProps } from './connection-tree.types';

const ROW_HEIGHT = 32;
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
  const { rows, isLoading, isError, error, retry, isEmpty } = useConnectionTree({
    onEditServer,
  });
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
    getItemKey: index => rows[index].id,
  });

  const { focusedNodeId, setFocusedNode, onKeyDown, onFocus } =
    useTreeNavigation({
      rows,
      virtualizer,
      containerRef: parentRef,
    });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-4">
        <Spinner className="h-4 w-4" />
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
    return <p className="text-sm text-muted-foreground">No servers connected</p>;
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

          return (
            <div
              key={virtualRow.key}
              // O mousedown já foca o container (tabIndex=0); aqui só alinhamos
              // o cursor do teclado com o que foi clicado.
              onMouseDown={() => {
                if (row.variant === 'node') setFocusedNode(row.id);
              }}
              className="absolute left-0 top-0 w-full"
              style={{
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              {row.variant === 'error' ? (
                <ErrorRow
                  level={row.level}
                  message={row.message}
                  onRetry={row.onRetry}
                />
              ) : (
                <NodeRow {...row.props} isFocused={row.id === focusedNodeId} />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});

ConnectionTree.displayName = 'ConnectionTree';
