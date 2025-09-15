import { DbStructureProvider } from "@/shared/providers/db-structure-provider";
import SidebarBody from "./sidebar-body";
import { useServers } from "@/shared/providers/servers-provider";

export default function Sidebar() {
  const serverProps = useServers();

  return (
    <DbStructureProvider setServers={serverProps.setServers}>
      <SidebarBody {...serverProps} />
    </DbStructureProvider>
  );
}
