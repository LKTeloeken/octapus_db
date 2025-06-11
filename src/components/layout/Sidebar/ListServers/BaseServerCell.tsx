import React, { forwardRef } from "react";
import {
  ListItem,
  ListItemText,
  ListItemButton,
  ListItemIcon,
} from "@/components/ui/list-item";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Power, Settings } from "lucide-react";

import ConfigServerDialog from "../ConfigServerDialog";

import { IPostgreServer, IPostgreServerPrimitive } from "@/models/postgreDb";

interface BaseServerCellProps {
  server: IPostgreServer;
  onClick?: (server: IPostgreServer) => Promise<any>;
  onEdit?: (id: number, serverData: IPostgreServerPrimitive) => Promise<any>;
  onRemove?: (serverId: number) => Promise<any>;
}

export default function BaseServerCell({
  server,
  onClick,
  onEdit,
  onRemove,
}: BaseServerCellProps) {
  return (
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
          <Badge variant={"destructive"}>
            <Power className="size-4" />
          </Badge>
        </ListItemIcon>
        <ListItemText primary={server.name} />
      </ListItemButton>
    </ListItem>
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
