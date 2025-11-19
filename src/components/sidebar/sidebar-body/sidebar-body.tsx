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
  }: SidebarBodyProps) => {
    return (
      <div className="h-full flex flex-col">
        <div className="flex items-center justify-between px-4 py-2">
          <div className="flex items-center gap-2">
            <Typography variant="p" className="font-semibold">
              Servidores
            </Typography>

            {isLoading && <Spinner />}
          </div>

          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="outline" size="sm" onClick={onCreateServer}>
                  <Plus />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="left">Adicionar servidor</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>

        <Separator />

        <ServerTree
          nodes={nodes}
          childrenMap={childrenMap}
          toggleNode={toggleNode}
        />
      </div>
    );
  }
);
