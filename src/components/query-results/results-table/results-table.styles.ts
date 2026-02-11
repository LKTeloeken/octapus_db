import { tw } from '@/lib/utils';

export const useStyles = () => ({
  emptyContainer: tw`flex h-48 items-center justify-center rounded-md border`,
  emptyText: tw`text-muted-foreground`,
  tableContainer: tw`relative h-full w-full overflow-auto rounded-md border scrollbar-thin`,
  header: tw`bg-background`,
  headerCell: tw`sticky top-0 z-20 bg-background shadow-[0_1px_0_0_var(--color-border)]`,
  rowEven: tw`bg-muted/30 hover:bg-muted/50`,
  rowOdd: tw`bg-transparent hover:bg-muted/50`,
  cellText: tw`font-mono text-xs truncate inline-block w-full`,
  loadingMoreContainer: tw`flex items-center justify-center py-3`,
  loadingMoreSpinner: tw`h-4 w-4`,
  loadingMoreText: tw`text-xs text-muted-foreground ml-2`,
});
