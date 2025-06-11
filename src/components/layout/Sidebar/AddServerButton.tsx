import React, { forwardRef } from "react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Plus } from "lucide-react";

const AddServerButton = forwardRef<
  HTMLButtonElement,
  React.ComponentPropsWithoutRef<"button">
>((props, ref) => {
  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="outline" size="sm" ref={ref} {...props}>
            <Plus />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Adicionar servidor</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
});

AddServerButton.displayName = "AddServerButton";

export default AddServerButton;
