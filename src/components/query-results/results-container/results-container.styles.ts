import { tw } from '@/lib/utils';

export const useStyles = () => ({
  root: tw`flex flex-col h-full`,
  loadingContainer: tw`flex h-full items-center justify-center`,
  loadingContent: tw`flex flex-col items-center gap-3`,
  loadingSpinner: tw`h-8 w-8`,
  loadingText: tw`text-muted-foreground`,
  errorContainer: tw`flex h-full items-center justify-center p-4`,
  errorContent: tw`flex flex-col items-center gap-2 max-w-md text-center`,
  errorTitle: tw`text-destructive font-medium !mt-0`,
  errorMessage: tw`text-destructive/80 text-sm !mt-0`,
  emptyContainer: tw`flex h-full items-center justify-center`,
  emptyText: tw`text-muted-foreground !mt-0`,
  tableWrapper: tw`flex-1 min-h-0`,
  statusBar: tw`flex items-center justify-between px-3 py-1.5 border-t border-border bg-muted/30 text-xs text-muted-foreground shrink-0`,
  statusBarLeft: tw`flex items-center gap-3`,
  statusBarRight: tw`flex items-center gap-2`,
  statusBarSpinner: tw`h-3 w-3`,
  statusBarLoadingText: tw`text-xs text-muted-foreground`,
});
