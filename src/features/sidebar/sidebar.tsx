import { Add01Icon, Settings01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { Typography } from '@/components/ui/typography';
import { ConnectionTree } from '@/features/connection-tree/connection-tree';
import { ServerForm } from '@/features/server-form/server-form';
import { SettingsDialog } from '@/features/settings/settings-dialog';
import { useSidebar } from './use-sidebar';

export const Sidebar = memo(() => {
  const {
    isFormOpen,
    editingServer,
    isSettingsOpen,
    openCreateForm,
    openEditForm,
    closeForm,
    openSettings,
    closeSettings,
  } = useSidebar();

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-4 py-2">
        <Typography variant="p" className="font-semibold">
          Servidores
        </Typography>

        <div className="flex items-center gap-1">
          <Tooltip content="Adicionar servidor" position="left">
            <Button variant="outline" size="sm" onClick={openCreateForm}>
              <HugeiconsIcon icon={Add01Icon} />
            </Button>
          </Tooltip>

          <Tooltip content="Configurações" position="left">
            <Button variant="outline" size="sm" onClick={openSettings}>
              <HugeiconsIcon icon={Settings01Icon} />
            </Button>
          </Tooltip>
        </div>
      </div>

      <Separator />

      <div className="p-2 flex flex-col gap-2 overflow-y-auto flex-1">
        <ConnectionTree onEditServer={openEditForm} />
      </div>

      <ServerForm
        open={isFormOpen}
        onClose={closeForm}
        server={editingServer}
      />

      <SettingsDialog open={isSettingsOpen} onClose={closeSettings} />
    </div>
  );
});

Sidebar.displayName = 'Sidebar';
