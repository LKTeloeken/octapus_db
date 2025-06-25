import React from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Typography } from "@/components/ui/typography";
import { Separator } from "@/components/ui/separator";
import { CircularProgress } from "@/components/ui/circular-progress";

import AddServerButton from "./AddServerButton";
import ConfigServerDialog from "./ConfigServerDialog";
import ListServers from "./ListServers";

import { useServers } from "@/providers/serversProvider";

export default function Sidebar() {
  const {
    servers,
    isLoading,
    addServer,
    editServer,
    removeServer,
    connectToServer,
    getDatabaseSchemas,
    getSchemaTables,
    getSchemaColumns,
  } = useServers();

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
      <ListServers
        servers={servers}
        onEdit={editServer}
        onRemove={removeServer}
        connectToServer={connectToServer}
        getDatabaseSchemas={getDatabaseSchemas}
        getSchemaTables={getSchemaTables}
        getSchemaColumns={getSchemaColumns}
      />
    </ScrollArea>
  );
}
