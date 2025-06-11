import React from "react";

import BaseServerCell from "./BaseServerCell";

import { IPostgreServer, IPostgreServerPrimitive } from "@/models/postgreDb";

interface ListServersProps {
  servers: Array<IPostgreServer>;
  onClickServer?: (server: IPostgreServer) => Promise<any>;
  onEdit?: (id: number, serverData: IPostgreServerPrimitive) => Promise<any>;
  onRemove?: (serverId: number) => Promise<any>;
}

export default function ListServers({
  servers,
  onEdit,
  onRemove,
}: ListServersProps) {
  return (
    <div className="flex flex-col gap-1 p-2">
      {servers.map((server) => (
        <BaseServerCell
          key={server.id}
          server={server}
          onClick={async (s) => {
            console.log("Server clicked:", s);
          }}
          onEdit={onEdit}
          onRemove={onRemove}
        />
      ))}
    </div>
  );
}
