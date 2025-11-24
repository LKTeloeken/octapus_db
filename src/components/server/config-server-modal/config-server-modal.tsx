import { SimpleAlertDialog } from "@/components/ui/simple-alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SimpleDialog } from "@/components/ui/simple-dialog";
import { useConfigServer } from "./use-config-server";
import { configs } from "./server-configs";
import type { ConfigServerModalProps } from "./config-server-modal.types";

export function ConfigServerModal(props: ConfigServerModalProps) {
  const { isOpen, isEditMode, onClose } = props;
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
        title={isEditMode ? "Editar Servidor" : "Adicionar Servidor"}
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
            <Input
              key={i}
              type={config.type}
              placeholder={config.placeholder}
              value={localServerData[config.value] as string}
              onChange={(e) => handleChangeInput(config.value)(e.target.value)}
            />
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
