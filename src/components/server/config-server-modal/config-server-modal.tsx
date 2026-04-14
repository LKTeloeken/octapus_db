import { SimpleAlertDialog } from '@/components/ui/simple-alert-dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { SimpleDialog } from '@/components/ui/simple-dialog';
import { useConfigServer } from './use-config-server';
import { configs } from './server-configs';
import type { ConfigServerModalProps } from './config-server-modal.types';
import { useState } from 'react';
import { Eye, EyeOff } from 'lucide-react';

export function ConfigServerModal(props: ConfigServerModalProps) {
  const { isOpen, isEditMode, onClose } = props;
  const [showPassword, setShowPassword] = useState(false);
  const {
    localServerData,
    openRemoveDialog,
    disableSave,
    handleChangeInput,
    handleRemove,
    handleSave,
    setOpenRemoveDialog,
  } = useConfigServer(props);

  return (
    <>
      <SimpleDialog
        open={isOpen}
        onOpenChange={onClose}
        title={isEditMode ? 'Editar Servidor' : 'Adicionar Servidor'}
        footer={
          <>
            {isEditMode && (
              <Button
                variant="destructive"
                size="sm"
                onClick={() => setOpenRemoveDialog(true)}
              >
                Remover
              </Button>
            )}
            {!isEditMode && (
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
          <div className="flex flex-col gap-4">
            {configs.map((config, i) => (
              <div key={i} className="space-y-1">
                <div className="relative">
                  <Input
                    type={
                      config.value === 'password'
                        ? showPassword
                          ? 'text'
                          : 'password'
                        : config.type
                    }
                    placeholder={config.placeholder}
                    value={String(localServerData[config.value] ?? '')}
                    onChange={e => handleChangeInput(config.value)(e.target.value)}
                  />
                  {config.value === 'password' ? (
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label={showPassword ? 'Hide password' : 'Show password'}
                      className="absolute right-1 top-1/2 -translate-y-1/2 h-8 w-8"
                      onClick={() => setShowPassword(current => !current)}
                    >
                      {showPassword ? (
                        <EyeOff className="size-4" />
                      ) : (
                        <Eye className="size-4" />
                      )}
                    </Button>
                  ) : null}
                </div>
                {config.value === 'password' && isEditMode ? (
                  <p className="text-xs text-muted-foreground">
                    Leave password empty to keep the existing value.
                  </p>
                ) : null}
              </div>
            ))}
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
