import { Add01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Typography } from '@/components/ui/typography';
import { ConnectionTree } from '@/features/connection-tree/connection-tree';
import { ServerForm } from '@/features/server-form/server-form';
import { useSidebar } from './use-sidebar';

export const Sidebar = memo(() => {
  const { isFormOpen, editingServer, openCreateForm, openEditForm, closeForm } =
    useSidebar();

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-4 py-2">
        <Typography variant="p" className="font-semibold">
          Servidores
        </Typography>

        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="outline" size="sm" onClick={openCreateForm}>
                <HugeiconsIcon icon={Add01Icon} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="left">Adicionar servidor</TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>

      <Separator />

      <div className="p-2 flex flex-col gap-2 overflow-y-auto flex-1">
        <ConnectionTree onEditServer={openEditForm} />
      </div>

      <ServerForm open={isFormOpen} onClose={closeForm} server={editingServer} />
    </div>
  );
});

Sidebar.displayName = 'Sidebar';
