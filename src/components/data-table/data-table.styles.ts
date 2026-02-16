import { tw } from '@/lib/utils';

export const useStyles = () => ({
  // Layout
  root: tw`flex flex-col h-full`,
  tableWrapper: tw`flex-1 min-h-0`,
  container: tw`relative h-full w-full overflow-auto rounded-md border scrollbar-thin`,

  // Table
  table: tw`caption-bottom text-sm`,

  // Header
  header: tw`bg-background`,
  headerRow: tw`border-b`,
  headerCell: tw`bg-background shadow-[0_1px_0_0_var(--color-border)] h-9 px-2 text-left align-middle font-medium text-foreground whitespace-nowrap text-xs select-none`,

  // Body
  bodyRow: tw`border-b border-r transition-colors`,
  bodyRowEven: tw`bg-muted/30`,
  bodyRowModified: tw`bg-yellow-900/20`,
  bodyCell: tw`p-0 align-middle whitespace-nowrap`,
  bodyCellModifiedIndicator: tw`absolute left-0 top-0 bottom-0 w-0.5 bg-yellow-500`,

  // Loading
  loadingContainer: tw`flex h-full items-center justify-center`,
  loadingContent: tw`flex flex-col items-center gap-3`,
  loadingSpinner: tw`h-8 w-8`,
  loadingText: tw`text-muted-foreground`,
  loadingMoreContainer: tw`flex items-center justify-center py-3`,
  loadingMoreSpinner: tw`h-4 w-4`,
  loadingMoreText: tw`text-xs text-muted-foreground ml-2`,

  // Empty
  emptyContainer: tw`flex h-full items-center justify-center`,
  emptyText: tw`text-muted-foreground`,

  // Status bar
  statusBar: tw`flex items-center justify-between px-3 py-1.5 border-t border-border bg-muted/30 text-xs text-muted-foreground shrink-0`,
  statusBarLeft: tw`flex items-center gap-3`,
  statusBarRight: tw`flex items-center gap-2`,
  statusBarSpinner: tw`h-3 w-3`,
  statusBarChanges: tw`text-yellow-400`,
});
