import React from "react";
import { useServers } from "@/providers/serversProvider";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Separator } from "@/components/ui/separator";
import { TableIcon, RefreshCwIcon } from "lucide-react";

export function DatabaseList() {
  const { selectedServer } = useServers();
  // In a real implementation, this would fetch tables when a server is selected
  const [tables, setTables] = React.useState<string[]>([]);
  const [isLoading, setIsLoading] = React.useState(false);

  // Mock data for demonstration
  const mockTables = [
    "customers",
    "orders",
    "order_items",
    "products",
    "suppliers",
  ];

  // Simulate fetching tables when a server is selected
  React.useEffect(() => {
    if (selectedServer) {
      setIsLoading(true);
      // Simulate API call
      setTimeout(() => {
        setTables(mockTables);
        setIsLoading(false);
      }, 500);
    } else {
      setTables([]);
    }
  }, [selectedServer]);

  const handleRefresh = () => {
    if (selectedServer) {
      setIsLoading(true);
      // Simulate API call
      setTimeout(() => {
        setTables(mockTables);
        setIsLoading(false);
      }, 500);
    }
  };

  if (!selectedServer) {
    return (
      <div className="p-1 mt-2">
        <p className="text-xs text-muted-foreground text-center">
          Select a server to view tables
        </p>
      </div>
    );
  }

  return (
    <div className="mt-4 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wide">
          Tables
        </h2>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleRefresh}
              disabled={isLoading}
              className="h-7 w-7 p-0"
            >
              <RefreshCwIcon
                className={`h-4 w-4 ${isLoading ? "animate-spin" : ""}`}
              />
              <span className="sr-only">Refresh</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <p>Refresh tables</p>
          </TooltipContent>
        </Tooltip>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8">
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"></div>
        </div>
      ) : tables.length > 0 ? (
        <ul className="space-y-1">
          {tables.map((table) => (
            <li key={table}>
              <Button
                variant="ghost"
                size="sm"
                className="w-full justify-start text-left"
              >
                <TableIcon className="h-4 w-4 mr-2" />
                {table}
              </Button>
            </li>
          ))}
        </ul>
      ) : (
        <div className="flex flex-col items-center justify-center py-8 text-center">
          <TableIcon className="h-8 w-8 text-muted-foreground mb-2" />
          <p className="text-sm text-muted-foreground">No tables found</p>
        </div>
      )}

      <Separator className="my-4" />

      <p className="text-xs text-muted-foreground text-center">
        Click on a table to view details or run queries
      </p>
    </div>
  );
}
