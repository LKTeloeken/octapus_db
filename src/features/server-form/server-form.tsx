import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { SimpleAlertDialog } from '@/components/ui/simple-alert-dialog';
import { SimpleDialog } from '@/components/ui/simple-dialog';
import { Switch } from '@/components/ui/switch';
import type { DatabaseType } from '@/api/types/server.types';
import { DB_TYPE_LABELS, SUPPORTED_DB_TYPES } from '@/lib/db-defaults';
import type { ServerFormProps } from './server-form.types';
import { useServerForm } from './use-server-form';

export function ServerForm(props: ServerFormProps) {
  const { open, onClose } = props;
  const {
    form,
    isEditMode,
    disableSave,
    openRemoveDialog,
    setField,
    setDbType,
    setOpenRemoveDialog,
    handleSave,
    handleRemove,
  } = useServerForm(props);

  return (
    <>
      <SimpleDialog
        open={open}
        onOpenChange={onClose}
        title={isEditMode ? 'Editar Servidor' : 'Adicionar Servidor'}
        footer={
          <>
            {isEditMode ? (
              <Button
                variant="destructive"
                size="sm"
                onClick={() => setOpenRemoveDialog(true)}
              >
                Remover
              </Button>
            ) : (
              <Button variant="ghost" size="sm" onClick={onClose}>
                Fechar
              </Button>
            )}
            <Button
              size="sm"
              variant="default"
              onClick={handleSave}
              disabled={disableSave}
            >
              Salvar
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-6">
          <Input
            type="text"
            label="Nome do servidor"
            placeholder="Digite aqui..."
            value={form.name}
            onChange={e => setField('name', e.target.value)}
          />

          <Select
            value={form.dbType}
            onValueChange={value => setDbType(value as DatabaseType)}
          >
            <SelectTrigger>
              <SelectValue placeholder="Tipo de banco" />
            </SelectTrigger>
            <SelectContent>
              {SUPPORTED_DB_TYPES.map(type => (
                <SelectItem key={type} value={type}>
                  {DB_TYPE_LABELS[type]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <div className="flex gap-2">
            <Input
              type="text"
              label="Host"
              placeholder="Digite aqui..."
              className="flex-1"
              value={form.host}
              onChange={e => setField('host', e.target.value)}
            />
            <Input
              type="number"
              label="Porta"
              placeholder="Digite aqui..."
              className="w-24"
              value={form.port || ''}
              onChange={e => setField('port', Number(e.target.value))}
            />
          </div>

          <div className="flex gap-2">
            <Input
              type="text"
              label="Usuário"
              placeholder="Digite aqui..."
              value={form.username}
              onChange={e => setField('username', e.target.value)}
            />

            <Input
              type="password"
              label={isEditMode ? 'Senha (redigite para salvar)' : 'Senha'}
              placeholder="Digite aqui..."
              value={form.password}
              onChange={e => setField('password', e.target.value)}
            />
          </div>

          <Input
            type="text"
            label="Banco de dados padrão"
            placeholder="Digite aqui..."
            value={form.defaultDatabase ?? ''}
            onChange={e => setField('defaultDatabase', e.target.value || null)}
          />

          <Input
            type="text"
            label="URI de conexão (opcional — Atlas, Redis Cloud...)"
            placeholder="Digite aqui..."
            value={form.connectionUri ?? ''}
            onChange={e => setField('connectionUri', e.target.value || null)}
          />

          <div className="flex items-center gap-2">
            <Switch
              id="ssl-enabled"
              checked={!!form.sslEnabled}
              onCheckedChange={checked => setField('sslEnabled', checked)}
            />
            <Label htmlFor="ssl-enabled" className="text-sm">
              SSL habilitado
            </Label>
          </div>
        </div>
      </SimpleDialog>

      <SimpleAlertDialog
        open={openRemoveDialog}
        onOpenChange={setOpenRemoveDialog}
        title="Confirmar remoção"
        description="Tem certeza que deseja remover este servidor? Esta ação não pode ser desfeita."
        onAccept={handleRemove}
      />
    </>
  );
}
