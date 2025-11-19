import { useState, useEffect, useCallback } from "react";
import type { ServerPrimitive } from "@/shared/models/servers.types";
import type { ConfigServerModalProps } from "./config-server-modal.types";

export const useConfigServer = (props: ConfigServerModalProps) => {
  const {
    isEditMode,
    serverData,
    onClose,
    onCreateServer,
    onEditServer,
    onRemoveServer,
  } = props;
  const [openRemoveDialog, setOpenRemoveDialog] = useState<boolean>(false);
  const [localServerData, setLocalServerData] = useState<ServerPrimitive>({
    name: "",
    username: "postgres",
    host: "",
    port: 5432,
    default_database: "postgres",
    password: "",
    isConnected: false,
  });

  const handleChangeInput = useCallback(
    (key: string) => (value: string) => {
      setLocalServerData((prev) => ({
        ...prev,
        [key]: key === "port" ? Number(value) : value,
      }));
    },
    []
  );

  const handleSave = async () => {
    if (isEditMode) {
      const { serverId } = props;

      await onEditServer({ ...localServerData, id: serverId });

      onClose();
      return;
    }

    await onCreateServer(localServerData);
    onClose();
  };

  const handleRemove = async () => {
    if (isEditMode) {
      const { serverId } = props;

      await onRemoveServer(serverId);

      onClose();
    }
  };

  useEffect(() => {
    if (isEditMode) setLocalServerData(serverData);
  }, [serverData]);

  return {
    openRemoveDialog,
    localServerData,
    handleChangeInput,
    handleSave,
    handleRemove,
    setOpenRemoveDialog,
  };
};
