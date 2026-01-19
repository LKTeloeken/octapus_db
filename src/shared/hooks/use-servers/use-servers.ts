import { useState } from "react";
import toast from "react-hot-toast";
import {
  getAllServers,
  createServer,
  updateServer,
  deleteServer,
} from "@/api/server/methods";
import { formatTreeNode } from "@/lib/format-tree-node";
import type { Server, ServerPrimitive } from "@/shared/models/servers.types";
import type {
  UserServersProps,
  AddServer,
  EditServer,
  DeleteServer,
} from "./user-servers.types";
import { TreeNodeType } from "@/shared/models/database.types";

export const useServers = ({ addChildren, removeNode }: UserServersProps) => {
  const [isLoading, setIsLoading] = useState<boolean>(false);

  const fetchServers = async () => {
    setIsLoading(true);
    try {
      const data = await getAllServers();
      const formattedServers = data.map((server) =>
        formatTreeNode(
          `server-${server.id}`,
          TreeNodeType.Server,
          server.id,
          server.name,
          null,
          { serverData: server }
        )
      );

      addChildren(formattedServers);
    } catch (error) {
      console.log("error", error);

      toast.error("Failed to fetch servers.");
    } finally {
      setIsLoading(false);
    }
  };

  const addServer: AddServer = async (server: ServerPrimitive) => {
    setIsLoading(true);
    try {
      const newServer = await createServer(server);

      const formattedServer = formatTreeNode(
        `server-${newServer.id}`,
        TreeNodeType.Server,
        newServer.id,
        newServer.name,
        null,
        { serverData: newServer }
      );

      addChildren([formattedServer]);

      toast.success("Server added successfully.");
    } catch (error) {
      toast.error("Failed to add server.");
    } finally {
      setIsLoading(false);
    }
  };

  const editServer: EditServer = async (server: Server) => {
    setIsLoading(true);
    try {
      const updatedServer = await updateServer(server);

      const formattedServer = formatTreeNode(
        `server-${updatedServer.id}`,
        TreeNodeType.Server,
        updatedServer.id,
        updatedServer.name,
        null,
        { serverData: updatedServer }
      );

      addChildren([formattedServer]);

      toast.success("Server updated successfully.");
    } catch (error) {
      toast.error("Failed to update server.");
    } finally {
      setIsLoading(false);
    }
  };

  const removeServer: DeleteServer = async (id: number) => {
    setIsLoading(true);
    try {
      await deleteServer(id);
      removeNode(`server-${id}`);
      toast.success("Server deleted successfully.");
    } catch (error) {
      toast.error("Failed to delete server.");
    } finally {
      setIsLoading(false);
    }
  };

  return {
    isLoading,
    fetchServers,
    addServer,
    editServer,
    removeServer,
  };
};
