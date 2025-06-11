import React from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";

interface SimpleAlertDialogProps {
  Trigger: React.ForwardRefExoticComponent<any> | React.ComponentType<any>;
  title: React.ReactNode;
  description?: React.ReactNode;
  children?: React.ReactNode;
  acceptLabel?: string;
  cancelLabel?: string;
  onAccept?: () => any;
  onCancel?: () => any;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function SimpleAlertDialog({
  Trigger,
  title,
  description,
  acceptLabel = "Sim",
  cancelLabel = "Não",
  onAccept,
  open,
  onOpenChange,
}: SimpleAlertDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogTrigger asChild>{<Trigger />}</AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{cancelLabel}</AlertDialogCancel>
          <AlertDialogAction onClick={onAccept}>
            {acceptLabel}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
SimpleAlertDialog.displayName = "AlertDialogDemo";
