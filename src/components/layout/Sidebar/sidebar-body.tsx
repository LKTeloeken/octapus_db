import { ScrollArea } from "@/components/ui/scroll-area";
import { Typography } from "@/components/ui/typography";
import { Separator } from "@/components/ui/separator";
import { CircularProgress } from "@/components/ui/circular-progress";

import AddServerButton from "@/components/common/server/add-server-button";
import ConfigServerDialog from "@/components/common/server/config-server-dialog";
import RecursiveList from "@/components/common/recursive-list";

import { renderItems } from "./render-items";

import { useDbStructure } from "@/shared/providers/db-structure-provider";
import { useTabs } from "@/shared/providers/tabs-provider";

import type { ServersProviderReturn } from "@/shared/providers/servers-provider";

export default function SidebarBody({
  servers,
  isLoading,
  addServer,
  connectToServer,
}: ServersProviderReturn) {
  const {
    getDatabaseSchemas,
    getSchemaTables,
    getTableColumns,
    getTableIndexes,
    getTableTriggers,
  } = useDbStructure();
  const { openTab } = useTabs();

  return (
    <ScrollArea className="h-full">
      <div className="flex items-center justify-between px-4 py-2">
        <div className="flex items-center gap-2">
          <Typography variant="p" className="font-semibold">
            Servidores
          </Typography>

          {isLoading && <CircularProgress size={25} />}
        </div>

        <ConfigServerDialog
          DialogTrigger={AddServerButton}
          onCreate={addServer}
        />
      </div>
      <Separator />

      <RecursiveList
        tree={servers}
        renderItem={renderItems(
          isLoading,
          connectToServer,
          getDatabaseSchemas,
          getSchemaTables,
          getTableColumns,
          getTableIndexes,
          getTableTriggers,
          openTab
        )}
        emptyMessage="Nenhum servidor encontrado."
      />
    </ScrollArea>
  );
}
