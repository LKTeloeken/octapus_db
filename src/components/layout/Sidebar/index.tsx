import React from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Typography } from "@/components/ui/typography";
import { Separator } from "@/components/ui/separator";
import { CircularProgress } from "@/components/ui/circular-progress";

import AddServerButton from "@/components/common/server/add-server-button";
import ConfigServerDialog from "@/components/common/server/config-server-dialog";
import RecursiveList from "@/components/common/recursive-list";

import { renderItems } from "./render-items";
import { useServers } from "@/shared/providers/serversProvider";

export default function Sidebar() {
  const {
    servers,
    isLoading,
    addServer,
    connectToServer,
    getDatabaseSchemas,
    getSchemaTables,
    getTableColumns,
    getTableIndexes,
    getTableTriggers,
  } = useServers();

  console.log("Rendering Sidebar with servers:", servers);

  return (
    <ScrollArea className="h-full">
      <div className="flex items-center justify-between px-4 py-2">
        <div className="flex items-center gap-2">
          <Typography variant="p" className="font-semibold">
            Servidores
          </Typography>

          {isLoading && <CircularProgress size={25} />}
        </div>

        <ConfigServerDialog
          DialogTrigger={AddServerButton}
          onCreate={addServer}
        />
      </div>
      <Separator />

      <RecursiveList
        tree={servers}
        renderItem={renderItems(
          isLoading,
          connectToServer,
          getDatabaseSchemas,
          getSchemaTables,
          getTableColumns,
          getTableIndexes,
          getTableTriggers
        )}
        emptyMessage="Nenhum servidor encontrado."
      />
    </ScrollArea>
  );
}
