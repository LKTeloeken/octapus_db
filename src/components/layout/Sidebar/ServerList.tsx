import React from "react";
import { useServers } from "@/providers/serversProvider";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  ListItem,
  ListItemText,
  ListItemButton,
} from "@/components/ui/list-item";
import { Badge } from "@/components/ui/badge";
import { PlusCircleIcon, ServerIcon, Edit2Icon } from "lucide-react";
import { IPostgreServer } from "@/models/postgreDb";

interface ServerListProps {
  onCreateServer: () => void;
  onEditServer: (server: IPostgreServer) => void;
}

export function ServerList({ onCreateServer, onEditServer }: ServerListProps) {
  const { servers, selectedServer, selectServer } = useServers();

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wide">
          Servers
        </h2>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              onClick={onCreateServer}
              className="h-7 w-7 p-0"
            >
              <PlusCircleIcon className="h-4 w-4" />
              <span className="sr-only">Add Server</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <p>Add new server</p>
          </TooltipContent>
        </Tooltip>
      </div>

      {servers.length > 0 ? (
        <>
          {servers.map((server) => (
            <ListItem
              key={server.id}
              className="flex items-center justify-between"
              disablePadding
              secondaryAction={
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        onEditServer(server);
                      }}
                      className="h-7 w-7 p-0"
                    >
                      <Edit2Icon className="h-3 w-3" />
                      <span className="sr-only">Edit Server</span>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>Edit server</p>
                  </TooltipContent>
                </Tooltip>
              }
            >
              <ListItemButton
                className="justify-start text-left overflow-hidden"
                onClick={() => selectServer(server.id)}
              >
                <ServerIcon className="h-4 w-4 mr-2 flex-shrink-0" />
                <ListItemText
                  primary={server.name.toString()}
                  secondary={
                    server.isConnected ? (
                      <Badge variant="default" className="text-xs">
                        Connected
                      </Badge>
                    ) : (
                      <Badge variant="destructive" className="text-xs">
                        Disconnected
                      </Badge>
                    )
                  }
                />
              </ListItemButton>
            </ListItem>

            // <li key={server.id} className="flex items-center justify-between">
            //   <Button
            //     variant={
            //       selectedServer?.id === server.id ? "secondary" : "ghost"
            //     }
            //     size="sm"
            //     className="w-[90%] justify-start text-left overflow-hidden"
            //     onClick={() => selectServer(server.id)}
            //   >
            //     <ServerIcon className="h-4 w-4 mr-2 flex-shrink-0" />
            //     <span className="truncate">{server.name.toString()}</span>
            //   </Button>
            //   <Tooltip>
            //     <TooltipTrigger asChild>
            //       <Button
            //         variant="ghost"
            //         size="sm"
            //         onClick={(e) => {
            //           e.stopPropagation();
            //           onEditServer(server);
            //         }}
            //         className="h-7 w-7 p-0"
            //       >
            //         <Edit2Icon className="h-3 w-3" />
            //       </Button>
            //     </TooltipTrigger>
            //     <TooltipContent>
            //       <p>Edit server</p>
            //     </TooltipContent>
            //   </Tooltip>
            // </li>
          ))}
        </>
      ) : (
        <div className="flex flex-col items-center justify-center py-4 text-center">
          <ServerIcon className="h-8 w-8 text-muted-foreground mb-2" />
          <p className="text-sm text-muted-foreground mb-2">No servers found</p>
          <Button size="sm" onClick={onCreateServer}>
            Add Server
          </Button>
        </div>
      )}
    </div>
  );
}
