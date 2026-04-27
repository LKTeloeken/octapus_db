import { tw } from '@/lib/utils';

export const useStyles = () => ({
  container: tw`flex items-center gap-2 py-1 px-2 cursor-pointer hover:text-foreground hover:bg-surface-light/50 rounded-md transition-color group relative pr-8`,
  iconWrapper: tw`flex items-center justify-center w-4 h-4 shrink-0`,
  spinner: tw`w-3 h-3`,
  chevronIcon: tw`min-w-4 min-h-4 max-w-4 max-h-4 text-foreground`,
  contentWrapper: tw`flex items-center gap-2 min-w-0`,
  textWrap: tw`flex items-baseline gap-1.5 min-w-0`,
  nameText: tw`text-sm truncate`,
  dataTypeText: tw`text-xs text-muted-foreground truncate`,
  menuButtonWrapper: tw`absolute right-2 transition-opacity`,
  menuButton: tw`h-6 w-6`,
  popoverContent: tw`w-32 p-1`,
  menuItemButton: tw`w-full justify-start gap-2`,
  menuIcon: tw`h-3 w-3`,
});
