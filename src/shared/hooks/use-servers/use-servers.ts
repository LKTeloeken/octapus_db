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
import type { UserServersProps } from "./user-servers.types";

export const useServers = ({ addChildren }: UserServersProps) => {
  const [isLoading, setIsLoading] = useState<boolean>(false);

  const fetchServers = async () => {
    setIsLoading(true);
    try {
      const data = await getAllServers();
      const formattedServers = data.map((server) =>
        formatTreeNode(
          `server-${server.id}`,
          "server",
          server.id,
          server.name,
          null,
          { serverData: server }
        )
      );

      addChildren(formattedServers);
    } catch (error) {
      toast.error("Failed to fetch servers.");
    } finally {
      setIsLoading(false);
    }
  };

  const addServer = async (server: ServerPrimitive) => {
    setIsLoading(true);
    try {
      const newServer = await createServer(server);

      const formattedServer = formatTreeNode(
        `server-${newServer.id}`,
        "server",
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

  const editServer = async (server: Server) => {
    setIsLoading(true);
    try {
      const updatedServer = await updateServer(server);

      const formattedServer = formatTreeNode(
        `server-${updatedServer.id}`,
        "server",
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

  const removeServer = async (id: number) => {
    setIsLoading(true);
    try {
      await deleteServer(id);
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
