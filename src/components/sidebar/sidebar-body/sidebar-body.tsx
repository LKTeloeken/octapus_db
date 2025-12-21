import { memo } from "react";
import { Typography } from "@/components/ui/typography";
import { Spinner } from "@/components/ui/spinner";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { ServerTree } from "../server-tree/server-tree";
import { ConfigServerModal } from "@/components/server/config-server-modal/config-server-modal";
import { useSidebarBody } from "./use-sidebar-body";
import { Plus } from "lucide-react";
import type { SidebarBodyProps } from "./sidebar-body.types";

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
    const {
      isConfigServerModalOpen,
      editingServer,
      openConfigServerModal,
      closeConfigServerModal,
    } = useSidebarBody();

    return (
      <div className="h-full flex flex-col">
        <div className="flex items-center justify-between px-4 py-2">
          <div className="flex items-center gap-2">
            <Typography variant="p" className="font-semibold">
              Servidores
            </Typography>

            {isLoading && <Spinner />}
          </div>

          {/* <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => openConfigServerModal()}
                >
                  <Plus />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">Adicionar servidor</TooltipContent>
            </Tooltip>
          </TooltipProvider> */}

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

        <div className="p-2 flex flex-col gap-2 overflow-y-auto flex-1">
          <div
            className="w-full p-1 flex items-center justify-center gap-2 text-xs font-bold text-muted-foreground cursor-pointer hover:bg-accent hover:text-accent-foreground rounded-sm"
            onClick={() => openConfigServerModal()}
          >
            <Plus className="size-4" />
            Adicionar
          </div>
          <ServerTree
            nodes={nodes}
            childrenMap={childrenMap}
            toggleNode={toggleNode}
            openServerModal={openConfigServerModal}
            openNewTab={openTab}
          />
        </div>
      </div>
    );
  }
);
