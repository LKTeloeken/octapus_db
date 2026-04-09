import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { memo } from 'react';
import { Typography } from '@/components/ui/typography';
import { Spinner } from '@/components/ui/spinner';
import { Separator } from '@/components/ui/separator';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Input } from '@/components/ui/input';
import { ServerTree } from '../server-tree/server-tree';
import { ConfigServerModal } from '@/components/server/config-server-modal/config-server-modal';
import { useSidebarBody } from './use-sidebar-body';
import { useStyles } from './sidebar-body.styles';
import type { SidebarBodyProps } from './sidebar-body.types';

export const SidebarBody = memo(
  ({
    nodes,
    childrenMap,
    isLoading,
    toggleNode,
    onCreateServer,
    onEditServer,
    onDeleteServer,
    openTab,
  }: SidebarBodyProps) => {
    const styles = useStyles();
    const {
      isConfigServerModalOpen,
      editingServer,
      searchTerm,
      setSearchTerm,
      openConfigServerModal,
      closeConfigServerModal,
    } = useSidebarBody();

    return (
      <div className={styles.root}>
        <div className={styles.header}>
          <div className={styles.headerContent}>
            <Typography variant="p" className={styles.titleText}>
              Servidores
            </Typography>

            {isLoading && <Spinner />}
          </div>

          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => openConfigServerModal()}
                >
                  <HugeiconsIcon icon={Add01Icon} />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">Adicionar servidor</TooltipContent>
            </Tooltip>
          </TooltipProvider>

          {editingServer ? (
            <ConfigServerModal
              isOpen={isConfigServerModalOpen}
              onClose={closeConfigServerModal}
              isEditMode={true}
              isLoading={isLoading}
              onEditServer={onEditServer}
              onRemoveServer={onDeleteServer}
              serverData={editingServer}
              serverId={editingServer.id}
            />
          ) : (
            <ConfigServerModal
              isOpen={isConfigServerModalOpen}
              onClose={closeConfigServerModal}
              isEditMode={false}
              isLoading={isLoading}
              onCreateServer={onCreateServer}
              onRemoveServer={onDeleteServer}
              serverData={null}
            />
          )}
        </div>

        <Separator />

        <div className={styles.content}>
          <Input
            value={searchTerm}
            onChange={event => setSearchTerm(event.target.value)}
            placeholder="Filtrar schemas, tabelas e colunas..."
          />
          <ServerTree
            nodes={nodes}
            childrenMap={childrenMap}
            toggleNode={toggleNode}
            openServerModal={openConfigServerModal}
            openNewTab={openTab}
            searchTerm={searchTerm}
          />
        </div>
      </div>
    );
  },
);
