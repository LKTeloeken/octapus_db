import { memo, type FC } from "react";
import { cn } from "@/lib/utils";
import { Spinner } from "@/components/ui/spinner";
import { Typography } from "@/components/ui/typography";
import { ResultsTable } from "@/components/query-results/results-table/results-table";

import type { ResultsContainerProps } from "./results-container.types";

export const ResultsContainer: FC<ResultsContainerProps> = memo(
  ({ columns, rows, isLoading = false, error = null, className }) => {
    if (isLoading) {
      return (
        <div
          className={cn("flex h-full items-center justify-center", className)}
        >
          <div className="flex flex-col items-center gap-3">
            <Spinner className="h-8 w-8" />
            <Typography variant="p" className="text-muted-foreground">
              Executing query...
            </Typography>
          </div>
        </div>
      );
    }

    if (error) {
      return (
        <div
          className={cn(
            "flex h-full items-center justify-center p-4",
            className
          )}
        >
          <div className="flex flex-col items-center gap-2 max-w-md text-center">
            <Typography
              variant="p"
              className="text-destructive font-medium !mt-0"
            >
              Query Error
            </Typography>
            <Typography
              variant="p"
              className="text-destructive/80 text-sm !mt-0"
            >
              {error}
            </Typography>
          </div>
        </div>
      );
    }

    if (!columns || !rows) {
      return (
        <div
          className={cn("flex h-full items-center justify-center", className)}
        >
          <Typography variant="p" className="text-muted-foreground !mt-0">
            Execute a query to see results
          </Typography>
        </div>
      );
    }

    return (
      <ResultsTable
        columns={columns}
        rows={rows}
        className={cn("h-full", className)}
      />
    );
  }
);

ResultsContainer.displayName = "ResultsContainer";
