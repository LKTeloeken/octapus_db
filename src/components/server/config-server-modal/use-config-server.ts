import { useState, useEffect, useCallback, useMemo } from 'react';
import type { ServerPrimitive } from '@/shared/models/servers.types';
import type { ConfigServerModalProps } from './config-server-modal.types';

export const useConfigServer = (props: ConfigServerModalProps) => {
  const { isEditMode, serverData, isLoading, onClose, onRemoveServer } = props;
  const [openRemoveDialog, setOpenRemoveDialog] = useState<boolean>(false);
  const [localServerData, setLocalServerData] = useState<ServerPrimitive>({
    name: '',
    username: 'postgres',
    host: '',
    port: 5432,
    default_database: 'postgres',
    password: '',
    isConnected: false,
  });
  const disableSave = useMemo(() => {
    const isMissingRequiredPassword = !isEditMode && !localServerData.password;
    return (
      !localServerData.name ||
      !localServerData.host ||
      !localServerData.port ||
      !localServerData.default_database ||
      !localServerData.username ||
      isMissingRequiredPassword ||
      isLoading
    );
  }, [isEditMode, isLoading, localServerData]);

  const handleChangeInput = useCallback(
    (key: string) => (value: string) => {
      setLocalServerData(prev => ({
        ...prev,
        [key]: key === 'port' ? Number(value) : value,
      }));
    },
    [],
  );

  const handleSave = async () => {
    if (isEditMode) {
      const { serverId, onEditServer } = props;

      await onEditServer({ ...localServerData, id: serverId });

      onClose();
      return;
    }

    const { onCreateServer } = props;

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
    if (isEditMode) {
      setLocalServerData({
        ...serverData,
        password: '',
      });
    }
  }, [isEditMode, serverData]);

  return {
    openRemoveDialog,
    localServerData,
    disableSave,
    handleChangeInput,
    handleSave,
    handleRemove,
    setOpenRemoveDialog,
  };
};
