import React, { forwardRef } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Power, Settings } from "lucide-react";

import BaseCell from "./BaseCell";
import ConfigServerDialog from "../../ConfigServerDialog";
import BaseDatabaseCell from "./DatabaseCell";

import { IServer, IServerPrimitive } from "@/models/server";

interface BaseServerCellProps {
  server: IServer;
  onClick?: (server: IServer) => Promise<any>;
  onEdit?: (id: number, serverData: IServerPrimitive) => Promise<any>;
  onRemove?: (serverId: number) => Promise<any>;
  getDatabaseSchemas?: (
    serverId: number,
    databaseName?: string
  ) => Promise<any>;
}

export default function BaseServerCell({
  server,
  onClick,
  onEdit,
  onRemove,
  getDatabaseSchemas,
}: BaseServerCellProps) {
  return (
    <>
      <BaseCell
        icon={
          <Badge variant={server.isConnected ? "default" : "destructive"}>
            <Power className="size-4" />
          </Badge>
        }
        primaryText={server.name}
        onClick={() => {
          onClick?.(server);
        }}
        secondaryAction={
          <ConfigServerDialog
            DialogTrigger={ServerEditButton}
            serverInfo={server}
            onEdit={onEdit}
            onRemove={onRemove}
            editing
          />
        }
      >
        {server && server.databases && server.databases.length > 0 && (
          <div className="flex flex-col gap-1 px-2">
            {server.databases.map((db) => (
              <BaseDatabaseCell
                database={db}
                key={db.name}
                getDatabaseSchemas={getDatabaseSchemas}
              />
            ))}
          </div>
        )}
      </BaseCell>
    </>
  );
}

const ServerEditButton = forwardRef<HTMLButtonElement, any>(
  ({ onClick }, ref) => (
    <Button variant={"ghost"} size={"sm"} onClick={onClick} ref={ref}>
      <Settings className="size-4" />
    </Button>
  )
);

ServerEditButton.displayName = "ServerEditButton";
