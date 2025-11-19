import React from "react";
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Spinner } from "@/components/ui/spinner";
import { type GenericModalProps, modalVariants } from "./generic-modal.types";

import { cn } from "@/lib/utils";

export function GenericModal({
  open,
  setOpen,
  Trigger,
  closeButton,
  title,
  description,
  children,
  footer,
  showCloseButton = true,
  maxWidth,
  loading = false,
}: GenericModalProps) {
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {Trigger && (
        <DialogTrigger asChild>
          <Trigger />
        </DialogTrigger>
      )}
      {open && (
        <>
          <DialogContent
            className={cn(modalVariants({ maxWidth }), "w-full")}
            showCloseButton={showCloseButton}
          >
            {(title || description) && (
              <DialogHeader>
                {title && (
                  <DialogTitle className="flex items-center gap-2">
                    {title}
                    {loading && <Spinner />}
                  </DialogTitle>
                )}
                {description && (
                  <DialogDescription>{description}</DialogDescription>
                )}
              </DialogHeader>
            )}
            {children}
            {(footer || closeButton) && (
              <DialogFooter>
                <DialogClose asChild>{closeButton}</DialogClose>
                {footer}
              </DialogFooter>
            )}
          </DialogContent>
        </>
      )}
    </Dialog>
  );
}
