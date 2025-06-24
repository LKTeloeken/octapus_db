import React from "react";

import BaseServerCell from "./BaseServerCell";

import { IServer, IServerPrimitive } from "@/models/server";

interface ListServersProps {
  servers: Array<IServer>;
  onClickServer?: (server: IServer) => Promise<any>;
  onEdit?: (id: number, serverData: IServerPrimitive) => Promise<any>;
  onRemove?: (serverId: number) => Promise<any>;
  connectToServer?: (server: IServer) => Promise<boolean | undefined>;
  getDatabaseSchemas?: (
    serverId: number,
    databaseName?: string
  ) => Promise<any>;
}

export default function ListServers({
  servers,
  onEdit,
  onRemove,
  connectToServer,
  getDatabaseSchemas,
}: ListServersProps) {
  return (
    <div className="flex flex-col gap-1 p-2">
      {servers.map((server) => (
        <BaseServerCell
          key={server.id}
          server={server}
          onClick={async (s) => {
            console.log("Server clicked:", s);
            await connectToServer?.(s);
          }}
          onEdit={onEdit}
          onRemove={onRemove}
          getDatabaseSchemas={getDatabaseSchemas}
        />
      ))}
    </div>
  );
}
