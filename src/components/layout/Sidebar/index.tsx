import React, { useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { AlertCircle } from "lucide-react";
import { useServers } from "@/providers/serversProvider";
import { ServerList } from "./ServerList";
import { ServerForm } from "./ServerForm";
import { DatabaseList } from "./DatabaseList";
import { IPostgreServer, IPostgreServerPrimitive } from "@/models/postgreDb";

export default function Sidebar() {
  const { addServer, editServer, removeServer, error } = useServers();
  const [isAddingServer, setIsAddingServer] = useState(false);
  const [editingServer, setEditingServer] = useState<IPostgreServer | null>(
    null
  );

  const handleCreateServer = () => {
    setIsAddingServer(true);
  };

  const handleEditServer = (server: IPostgreServer) => {
    setEditingServer(server);
  };

  const handleCloseAddServer = () => {
    setIsAddingServer(false);
  };

  const handleCloseEditServer = () => {
    setEditingServer(null);
  };

  const handleAddServerSubmit = async (data: IPostgreServerPrimitive) => {
    try {
      await addServer(data);
    } catch (err) {
      console.error("Failed to add server:", err);
    }
  };

  const handleEditServerSubmit = async (data: IPostgreServerPrimitive) => {
    if (!editingServer) return;
    try {
      await editServer(editingServer.id, data);
    } catch (err) {
      console.error("Failed to update server:", err);
    }
  };

  return (
    <ScrollArea className="h-full">
      <div className="p-4 space-y-4">
        {error && (
          <Alert variant="destructive" className="mb-4">
            <AlertCircle className="h-4 w-4" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <ServerList
          onCreateServer={handleCreateServer}
          onEditServer={handleEditServer}
        />

        <Separator className="my-4" />

        <DatabaseList />

        {/* Server Forms */}
        <ServerForm
          isOpen={isAddingServer}
          onClose={handleCloseAddServer}
          onSubmit={handleAddServerSubmit}
        />

        <ServerForm
          server={editingServer || undefined}
          isOpen={!!editingServer}
          onClose={handleCloseEditServer}
          onSubmit={handleEditServerSubmit}
        />
      </div>
    </ScrollArea>
  );
}
