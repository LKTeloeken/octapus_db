import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { SimpleAlertDialog } from '@/components/ui/simple-alert-dialog';
import { SimpleDialog } from '@/components/ui/simple-dialog';
import {
  DB_TYPE_ICONS,
  DB_TYPE_LABELS,
  SUPPORTED_DB_TYPES,
} from '@/lib/db-defaults';
import type { SslMode } from '@/api/types/server.types';
import type { ServerFormProps } from './server-form.types';
import { useServerForm } from './use-server-form';
import { NO_GROUP } from '@/features/server-groups/server-groups.types';
import { cn } from '@/lib/utils';

const SCOPE_HELP =
  'Padrões separados por vírgula: * qualquer trecho, ? um caractere, ! na frente exclui. Vazio mostra todos.';

export function ServerForm(props: ServerFormProps) {
  const { open, onClose } = props;
  const {
    form,
    groups,
    isEditMode,
    disableSave,
    openRemoveDialog,
    sslOptions,
    sslVerifies,
    setField,
    setDbType,
    chooseRootCert,
    setOpenRemoveDialog,
    handleSave,
    handleRemove,
  } = useServerForm(props);

  const isFileBased = form.dbType === 'sqlite';

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
          {/* Escolha do banco: selecionado em `--active` + anel `--ring`; na
              edição o tipo não muda e os demais ficam apagados. */}
          <div role="radiogroup" aria-label="Banco" className="flex gap-2">
            {SUPPORTED_DB_TYPES.map(type => {
              const isSelected = form.dbType === type;

              return (
                <button
                  key={type}
                  type="button"
                  role="radio"
                  aria-checked={isSelected}
                  disabled={isEditMode && !isSelected}
                  className={cn(
                    'flex w-full cursor-pointer flex-col items-center gap-1.5 rounded-[10px] py-2.5 text-small font-medium transition-colors outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring',
                    isSelected
                      ? 'bg-active text-fg ring-[1.5px] ring-inset ring-ring'
                      : 'text-fg-muted ring-1 ring-inset ring-line hover:bg-hover hover:text-fg',
                    'disabled:cursor-default disabled:opacity-45 disabled:hover:bg-transparent',
                  )}
                  onClick={() => !isEditMode && setDbType(type)}
                >
                  <img
                    src={DB_TYPE_ICONS[type]}
                    alt=""
                    className="size-8"
                  />
                  {DB_TYPE_LABELS[type]}
                </button>
              );
            })}
          </div>

          {/* Foco inicial no nome, como antes de os cartões de banco virarem
              botões: sem isso o diálogo focaria o primeiro cartão. */}
          <Input
            type="text"
            label="Nome do servidor"
            placeholder="Digite aqui..."
            autoFocus
            value={form.name}
            onChange={e => setField('name', e.target.value)}
          />

          {groups.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="server-group">Grupo</Label>
              <Select
                value={form.groupId == null ? NO_GROUP : String(form.groupId)}
                onValueChange={value =>
                  setField('groupId', value === NO_GROUP ? null : Number(value))
                }
              >
                <SelectTrigger id="server-group" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NO_GROUP}>Sem grupo</SelectItem>
                  <SelectSeparator />
                  {groups.map(group => (
                    <SelectItem key={group.id} value={String(group.id)}>
                      {group.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {isFileBased ? (
            /* Banco de arquivo: não há host, porta nem credencial — o caminho
               do .db ocupa o lugar da URI de conexão. */
            <Input
              type="text"
              label="Arquivo do banco"
              placeholder="/Users/voce/dados/app.db"
              value={form.connectionUri ?? ''}
              onChange={e => setField('connectionUri', e.target.value || null)}
            />
          ) : (
            <>
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
                onChange={e =>
                  setField('defaultDatabase', e.target.value || null)
                }
              />

              {form.dbType !== 'postgres' && (
                <>
                  <Input
                    type="text"
                    label="URI de conexão (opcional — Atlas, Redis Cloud...)"
                    placeholder="Digite aqui..."
                    value={form.connectionUri ?? ''}
                    onChange={e =>
                      setField('connectionUri', e.target.value || null)
                    }
                  />
                </>
              )}

              <div className="flex flex-col gap-1.5">
                <Label htmlFor="server-ssl-mode">Criptografia (SSL/TLS)</Label>
                <Select
                  value={form.sslMode ?? 'disable'}
                  onValueChange={value => setField('sslMode', value as SslMode)}
                >
                  <SelectTrigger id="server-ssl-mode" className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {sslOptions.map(option => (
                      <SelectItem key={option.value} value={option.value}>
                        {option.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <p className="text-small text-fg-muted">
                  {
                    sslOptions.find(option => option.value === form.sslMode)
                      ?.description
                  }
                </p>
              </div>

              {sslVerifies && (
                <div className="flex items-end gap-2">
                  <Input
                    type="text"
                    label="CA própria (opcional)"
                    placeholder="CAs confiáveis do sistema"
                    className="flex-1"
                    value={form.sslRootCert ?? ''}
                    onChange={e =>
                      setField('sslRootCert', e.target.value || null)
                    }
                  />
                  <Button variant="outline" onClick={chooseRootCert}>
                    Escolher…
                  </Button>
                </div>
              )}

              {/* Escopo: quem tem milhares de tenants e só cuida de alguns */}
              <Input
                type="text"
                label="Databases visíveis (opcional)"
                placeholder="tenant_*, !tenant_teste*"
                helperText={SCOPE_HELP}
                value={form.scopeDatabases ?? ''}
                onChange={e =>
                  setField('scopeDatabases', e.target.value || null)
                }
              />
              {form.dbType === 'postgres' && (
                <Input
                  type="text"
                  label="Schemas visíveis (opcional)"
                  placeholder="public, tenant_00*"
                  helperText={SCOPE_HELP}
                  value={form.scopeSchemas ?? ''}
                  onChange={e =>
                    setField('scopeSchemas', e.target.value || null)
                  }
                />
              )}
            </>
          )}
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
