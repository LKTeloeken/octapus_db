import * as React from "react";
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";

export interface SimpleDialogProps {
  /** Conteúdo do gatilho (botão ou elemento que abre o diálogo) */
  trigger: React.ForwardRefExoticComponent<any> | React.ComponentType<any>;
  /** Título do diálogo */
  title: React.ReactNode;
  /** Descrição ou subtítulo */
  description?: React.ReactNode;
  /** Conteúdo principal do diálogo */
  children: React.ReactNode;
  /** Elementos de ação (botões) no rodapé */
  footer?: React.ReactNode;
  /** Controle de estado aberto/fechado */
  open?: boolean;
  /** Callback chamado quando o estado de aberto mudar */
  onOpenChange?: (open: boolean) => void;
}

export const SimpleDialog: React.FC<SimpleDialogProps> = ({
  trigger: Trigger,
  title,
  description,
  children,
  footer,
  open,
  onOpenChange,
}) => {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>{<Trigger />}</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && <DialogDescription>{description}</DialogDescription>}
        </DialogHeader>
        <div className="mt-4">{children}</div>
        {footer && <DialogFooter>{footer}</DialogFooter>}
      </DialogContent>
    </Dialog>
  );
};

SimpleDialog.displayName = "SimpleDialog";
