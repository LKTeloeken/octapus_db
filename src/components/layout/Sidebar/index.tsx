import { DbStructureProvider } from "@/shared/providers/dbStructureProvider";
import SidebarBody from "./sidebar-body";
import { useServers } from "@/shared/providers/serversProvider";

export default function Sidebar() {
  const serverProps = useServers();

  return (
    <DbStructureProvider setServers={serverProps.setServers}>
      <SidebarBody {...serverProps} />
    </DbStructureProvider>
  );
}
