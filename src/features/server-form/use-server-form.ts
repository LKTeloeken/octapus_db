import { useEffect, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import type { DatabaseType, ServerInput } from '@/api/types/server.types';
import {
  useCreateServer,
  useDeleteServer,
  useUpdateServer,
} from '@/queries/use-servers';
import { DEFAULT_DATABASES, DEFAULT_PORTS } from '@/lib/db-defaults';
import type { ServerFormProps } from './server-form.types';

const emptyForm: ServerInput = {
  name: '',
  dbType: 'postgres',
  host: '',
  port: DEFAULT_PORTS.postgres,
  username: 'postgres',
  password: '',
  defaultDatabase: DEFAULT_DATABASES.postgres,
  sslEnabled: false,
  connectionUri: null,
};

export const useServerForm = ({ open, onClose, server }: ServerFormProps) => {
  const [form, setForm] = useState<ServerInput>(emptyForm);
  const [openRemoveDialog, setOpenRemoveDialog] = useState(false);

  const createServer = useCreateServer();
  const updateServer = useUpdateServer();
  const deleteServer = useDeleteServer();

  const isEditMode = server != null;
  const isSaving = createServer.isPending || updateServer.isPending;

  useEffect(() => {
    if (!open) return;

    if (server) {
      setForm({
        name: server.name,
        dbType: server.dbType,
        host: server.host,
        port: server.port,
        username: server.username,
        password: '', // never comes back from the backend — must be retyped
        defaultDatabase: server.defaultDatabase,
        sslEnabled: server.sslEnabled,
        connectionUri: server.connectionUri,
      });
    } else {
      setForm(emptyForm);
    }
  }, [open, server]);

  const setField = <K extends keyof ServerInput>(
    key: K,
    value: ServerInput[K],
  ) => {
    setForm(prev => ({ ...prev, [key]: value }));
  };

  const setDbType = (dbType: DatabaseType) => {
    setForm(prev => ({
      ...prev,
      dbType,
      // Only swap defaults the user hasn't customized
      port:
        prev.port === DEFAULT_PORTS[prev.dbType]
          ? DEFAULT_PORTS[dbType]
          : prev.port,
      defaultDatabase:
        prev.defaultDatabase === DEFAULT_DATABASES[prev.dbType]
          ? DEFAULT_DATABASES[dbType]
          : prev.defaultDatabase,
    }));
  };

  const disableSave = useMemo(() => {
    const hasUri = !!form.connectionUri?.trim();
    const hasHostConfig =
      !!form.host && !!form.port && !!form.username && !!form.password;
    return !form.name || (!hasUri && !hasHostConfig) || isSaving;
  }, [form, isSaving]);

  const handleSave = async () => {
    try {
      if (isEditMode) {
        await updateServer.mutateAsync({ id: server.id, input: form });
        toast.success('Servidor atualizado.');
      } else {
        await createServer.mutateAsync(form);
        toast.success('Servidor adicionado.');
      }
      onClose();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRemove = async () => {
    if (!isEditMode) return;

    try {
      await deleteServer.mutateAsync(server.id);
      toast.success('Servidor removido.');
      onClose();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return {
    form,
    isEditMode,
    isSaving,
    disableSave,
    openRemoveDialog,
    setField,
    setDbType,
    setOpenRemoveDialog,
    handleSave,
    handleRemove,
  };
};
