import React, { forwardRef } from "react";
import {
  ListItem,
  ListItemText,
  ListItemButton,
  ListItemIcon,
} from "@/components/ui/list-item";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Power, Settings } from "lucide-react";

import ConfigServerDialog from "../ConfigServerDialog";
import BaseDatabaseCell from "./BaseDatabaseCell";

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
      <ListItem
        disablePadding
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
        <ListItemButton onClick={() => onClick?.(server)}>
          <ListItemIcon>
            <Badge variant={server.isConnected ? "default" : "destructive"}>
              <Power className="size-4" />
            </Badge>
          </ListItemIcon>
          <ListItemText primary={server.name} />
        </ListItemButton>
      </ListItem>

      {server.databases && server.databases.length > 0 && (
        <>
          <Separator className="my-1" />
          <div className="flex flex-col gap-1 px-2">
            {server.databases.map((db) => (
              <BaseDatabaseCell
                database={db}
                key={db.name}
                getDatabaseSchemas={getDatabaseSchemas}
              />
            ))}
          </div>
        </>
      )}
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
