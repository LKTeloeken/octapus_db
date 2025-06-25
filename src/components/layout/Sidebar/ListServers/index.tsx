import React from "react";

import ServerCell from "./Cells/ServerCell";

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
  getSchemaTables?: (
    serverId: number,
    schemaName: string,
    databaseName?: string
  ) => Promise<any>;
  getSchemaColumns?: (
    serverId: number,
    schemaName: string,
    tableName: string,
    databaseName?: string
  ) => Promise<any>;
}

export default function ListServers({
  servers,
  onEdit,
  onRemove,
  connectToServer,
  getDatabaseSchemas,
  getSchemaTables,
  getSchemaColumns,
}: ListServersProps) {
  return (
    <div className="flex flex-col gap-1 p-2">
      {servers.map((server) => (
        <ServerCell
          key={server.id}
          server={server}
          onClick={async (s) => {
            console.log("Server clicked:", s);
            await connectToServer?.(s);
          }}
          onEdit={onEdit}
          onRemove={onRemove}
          getDatabaseSchemas={getDatabaseSchemas}
          getSchemaTables={getSchemaTables}
          getSchemaColumns={getSchemaColumns}
        />
      ))}
    </div>
  );
}
