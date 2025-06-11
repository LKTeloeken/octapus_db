import React, { useEffect, useState, forwardRef } from "react";
import { SimpleDialog } from "@/components/ui/simple-dialog";
import { SimpleAlertDialog } from "@/components/ui/simple-alert-dialog";
import { Button, ButtonProps } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

import { IPostgreServer, IPostgreServerPrimitive } from "@/models/postgreDb";

interface ConfigServerProps {
  serverInfo?: IPostgreServer;
  editing?: boolean;
  DialogTrigger: React.ForwardRefExoticComponent<any>;
  onCreate?: (data: IPostgreServerPrimitive) => Promise<any>;
  onEdit?: (id: number, data: IPostgreServerPrimitive) => Promise<any>;
  onRemove?: (data: number) => Promise<any>;
}

interface ConfirmRemoveProps {
  onRemove: () => Promise<any>;
  Trigger: React.ForwardRefExoticComponent<any> | React.ComponentType<any>;
}

export default function ConfigServerDialog({
  serverInfo,
  editing = false,
  DialogTrigger,
  onCreate,
  onEdit,
  onRemove,
}: ConfigServerProps) {
  const [open, setOpen] = useState(false);
  const [serverData, setServerData] = useState<IPostgreServerPrimitive>({
    name: "",
    username: "postgres",
    host: "",
    port: 5432,
    default_database: "postgres",
    password: "",
    isConnected: false,
  });

  const handleSave = async () => {
    if (editing && serverInfo) {
      await onEdit?.(serverInfo.id, serverData);
    } else {
      await onCreate?.(serverData);
    }
    setOpen(false);
  };

  const handleRemove = async () => {
    if (serverInfo) {
      await onRemove?.(serverInfo.id);
    }
    setOpen(false);
  };

  const handleChange =
    (field: string) => (event: React.ChangeEvent<HTMLInputElement>) => {
      setServerData((prevData) => ({
        ...prevData,
        [field]: event.target.value,
      }));
    };

  useEffect(() => {
    if (!open || !serverInfo) return;

    setServerData({ ...serverInfo });
  }, [open, serverInfo]);

  return (
    <SimpleDialog
      open={open}
      onOpenChange={setOpen}
      title={editing ? "Editar servidor" : "Criar servidor"}
      description={"Preencha os dados do servidor"}
      trigger={DialogTrigger}
      footer={
        <>
          {editing && (
            <ConfirmRemove Trigger={RemoveButton} onRemove={handleRemove} />
          )}
          <Button variant={"default"} onClick={handleSave}>
            Salvar
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        {inputs.map((input, i) => (
          <Input
            key={i}
            placeholder={input.placeholder}
            value={serverData[input.value] as string}
            onChange={handleChange(input.value)}
          />
        ))}
      </div>
    </SimpleDialog>
  );
}

const RemoveButton = forwardRef<HTMLButtonElement, ButtonProps>(
  (props, ref) => {
    return (
      <Button variant={"destructive"} size={"sm"} ref={ref} {...props}>
        Remover
      </Button>
    );
  }
);

function ConfirmRemove({ onRemove, Trigger }: ConfirmRemoveProps) {
  return (
    <SimpleAlertDialog
      Trigger={Trigger}
      title={"Remover servidor"}
      description={
        "Você tem certeza que deseja remover este servidor? Esta ação não pode ser desfeita."
      }
      onAccept={onRemove}
    />
  );
}

const inputs: {
  placeholder: string;
  value:
    | "name"
    | "username"
    | "host"
    | "port"
    | "default_database"
    | "password";
  type: string;
}[] = [
  {
    placeholder: "Nome do servidor",
    value: "name",
    type: "text",
  },
  {
    placeholder: "Usuário",
    value: "username",
    type: "text",
  },
  {
    placeholder: "Host",
    value: "host",
    type: "text",
  },
  {
    placeholder: "Porta",
    value: "port",
    type: "number",
  },
  {
    placeholder: "Banco de dados padrão",
    value: "default_database",
    type: "text",
  },
  {
    placeholder: "Senha",
    value: "password",
    type: "password",
  },
];
