import { tw } from '@/lib/utils';

export const useStyles = () => ({
  root: tw`h-full flex flex-col`,
  header: tw`flex items-center justify-between px-4 py-2`,
  headerContent: tw`flex items-center gap-2`,
  titleText: tw`font-semibold`,
  content: tw`p-2 flex flex-col gap-2 overflow-y-auto flex-1`,
  addButton: tw`w-full p-1 flex items-center justify-center gap-2 text-xs font-bold text-muted-foreground cursor-pointer hover:bg-accent hover:text-accent-foreground rounded-sm`,
  addIcon: tw`size-4`,
});
